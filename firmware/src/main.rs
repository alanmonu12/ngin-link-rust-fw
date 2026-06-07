#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_usb::{Builder, UsbDevice};
use embassy_usb::driver::{Endpoint, EndpointIn, EndpointOut};
use static_cell::StaticCell;
use gs_usb_protocol::{default_gs_usb_config, handler::GsUsbControlHandler};
use gs_usb_protocol::gs_usb_types::{
    GsDeviceCapabilities, GsHostFrame, GsTxMsg, GS_CAN_FEATURE_IDENTIFY, GS_CAN_FEATURE_LISTEN_ONLY,
    GS_CAN_FEATURE_LOOP_BACK, GS_CAN_FEATURE_USER_ID, CanBusError,
};
use embassy_futures::select::{select, Either};

use {defmt_rtt as _, panic_probe as _};

// Cola (Channel) para comunicar la tarea del CAN (Productor) con la del USB (Consumidor).
// Usamos CriticalSectionRawMutex y una capacidad de 32 mensajes (ajustable según RAM/necesidad).
static CAN_RX_CHANNEL: Channel<CriticalSectionRawMutex, can_protocol::CanFrame, 32> = Channel::new();

/// Comandos que otras tareas pueden enviar al único actor que maneja el driver CAN.
/// Unificamos control (Start/Stop/SetBitTiming) y transmisión en un solo canal
/// para simplificar el bucle del driver y evitar un `select3` ilegible.
enum CanDriverCmd {
    Start { loopback: bool, listen_only: bool, one_shot: bool },
    Stop,
    SetBitTiming(gs_usb_protocol::gs_usb_types::GsDeviceBitTiming),
    Transmit(CanTxRequest),
}
static CAN_CMD_CHANNEL: Channel<CriticalSectionRawMutex, CanDriverCmd, 16> = Channel::new();

/// Petición de transmisión CAN generada por `usb_rx_task`.
struct CanTxRequest {
    echo_id: u32,
    id: u32,
    is_extended: bool,
    is_rtr: bool,
    data: [u8; 8],
    dlc: u8,
}

static USB_ECHO_CHANNEL: Channel<CriticalSectionRawMutex, GsHostFrame, 16> = Channel::new();

fn on_start_cb(flags: u32) {
    let loopback = (flags & gs_usb_protocol::gs_usb_types::GS_CAN_FLAG_LOOP_BACK) != 0;
    let listen_only = (flags & gs_usb_protocol::gs_usb_types::GS_CAN_FLAG_LISTEN_ONLY) != 0;
    let one_shot = (flags & gs_usb_protocol::gs_usb_types::GS_CAN_FLAG_ONE_SHOT) != 0;
    let _ = CAN_CMD_CHANNEL.try_send(CanDriverCmd::Start { loopback, listen_only, one_shot });
}
fn on_stop_cb() {
    let _ = CAN_CMD_CHANNEL.try_send(CanDriverCmd::Stop);
}
fn on_bit_timing_cb(timing: gs_usb_protocol::gs_usb_types::GsDeviceBitTiming) {
    let _ = CAN_CMD_CHANNEL.try_send(CanDriverCmd::SetBitTiming(timing));
}

/// Callback para `GS_USB_BREQ_IDENTIFY`: enciende/apaga el LED de
/// identificación. Por ahora solo loggeamos porque el hardware actual
/// no tiene un LED cableado; cuando se agregue el GPIO, esta función
/// debe llamar a `led.set_state(on)`.
fn on_identify_cb(on: bool) {
    if on {
        info!("IDENTIFY: LED encendido");
    } else {
        info!("IDENTIFY: LED apagado");
    }
}

/// Fuente de tiempo para `GS_USB_BREQ_TIMESTAMP`.
/// Devuelve milisegundos desde el boot, truncados a u32 (~49.7 días).
fn now_ms() -> u32 {
    embassy_time::Instant::now().as_millis() as u32
}

// Buffers de memoria estática que necesita el USB Builder
static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static CONTROL_HANDLER: StaticCell<GsUsbControlHandler> = StaticCell::new();

// La macro #[embassy_executor::main] configura el entorno asíncrono por ti
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // 1. Inicializamos TODO el hardware a través del BSP (Relojes, pines, y construcción del USB)
    // Ahora `main.rs` no sabe si es un STM32, un RP2040, o un ESP32.
    let board = bsp_f446::init();
    info!("Hardware y relojes configurados. Iniciando driver USB...");

    // 1.5. Construimos el dispositivo USB uniendo el driver del BSP con el protocolo
    let config_usb = default_gs_usb_config();
    let mut builder = Builder::new(
        board.usb_driver,
        config_usb,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        MSOS_DESC.init([0; 256]),
        CONTROL_BUF.init([0; 64]),
    );

    let control_handler = CONTROL_HANDLER.init(GsUsbControlHandler {
        on_start: Some(on_start_cb),
        on_stop: Some(on_stop_cb),
        on_bit_timing: Some(on_bit_timing_cb),
        on_identify: Some(on_identify_cb),
        now_ms,
        user_id: 0,
        capabilities: GsDeviceCapabilities::default()
            .with_feature(GS_CAN_FEATURE_LISTEN_ONLY)
            .with_feature(GS_CAN_FEATURE_LOOP_BACK)
            .with_feature(GS_CAN_FEATURE_IDENTIFY)
            .with_feature(GS_CAN_FEATURE_USER_ID),
    });
    builder.handler(control_handler);
    
    // Declaramos la interfaz de gs_usb y obtenemos los endpoints
    let (ep_in, ep_out) = {
        let mut function = builder.function(0xFF, 0xFF, 0xFF);
        let mut interface = function.interface();
        let mut alt_setting = interface.alt_setting(0xFF, 0xFF, 0xFF, None);
        let ep_in = alt_setting.endpoint_bulk_in(None, 64);
        let ep_out = alt_setting.endpoint_bulk_out(None, 64);
        (ep_in, ep_out)
    };

    let usb_device = builder.build();

    // 2. Lanzamos la tarea de fondo del USB
    spawner.spawn(usb_task(usb_device).unwrap());
    
    spawner.spawn(can_driver_task(board.can_driver).unwrap());
    spawner.spawn(usb_tx_task(ep_in).unwrap());
    spawner.spawn(usb_rx_task(ep_out).unwrap());

    info!("¡Sistema configurado y listo!");

    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}


#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, bsp_f446::usb::BspUsbDriver>) -> ! {
    usb.run().await
}


/// Tarea actor que es la **única dueña** del driver CAN (`BspCan`).
///
/// El bxCAN de Embassy requiere `&mut self` para `read()`, `write()` y
/// `modify_config()`, por lo que no se puede compartir libremente entre tareas.
/// Esta tarea centraliza todo el acceso al hardware CAN:
/// - Recibe comandos (`Start`, `Stop`, `SetBitTiming`, `Transmit`) por `CAN_CMD_CHANNEL`.
/// - Cuando está iniciada, lee tramas del bus y las mete en `CAN_RX_CHANNEL`.
#[embassy_executor::task]
async fn can_driver_task(mut can: bsp_f446::can::BspCan) {
    let mut is_started = false;
    let mut can_error_count: u32 = 0;

    loop {
        if is_started {
            match select(CAN_CMD_CHANNEL.receive(), can.can.read()).await {
                Either::First(cmd) => {
                    match cmd {
                        CanDriverCmd::Stop => {
                            info!("CAN: Apagando controlador");
                            is_started = false;
                            can.stop().await;
                        }
                        CanDriverCmd::Start { loopback, listen_only, one_shot } => {
                            // El host re-envía START, posiblemente con modo diferente.
                            // Re-aplicamos la configuración para cambiar de Normal a Listen-Only etc.
                            info!("CAN: Re-aplicando modo (loopback={}, listen_only={}, one_shot={})", loopback, listen_only, one_shot);
                            can.start(loopback, listen_only, one_shot).await;
                        }
                        CanDriverCmd::SetBitTiming(timing) => {
                            info!("CAN: Reconfigurando Bit Timing en caliente: brp={}, prop={}, phase1={}, phase2={}, sjw={}",
                                timing.brp, timing.prop_seg, timing.phase_seg1, timing.phase_seg2, timing.sjw);
                            can.set_bit_timing(&timing);
                            // modify_config() pone el CAN en init mode y luego en sleep.
                            // Hay que sacar del sleep y re-habilitar.
                            can.reenable().await;
                        }
                        CanDriverCmd::Transmit(tx_req) => {
                            info!("CAN TX: Enviando id=0x{:03X} ext={} dlc={}", tx_req.id, tx_req.is_extended, tx_req.dlc);
                            let data_slice = &tx_req.data[..tx_req.dlc as usize];
                            match can.transmit(tx_req.id, tx_req.is_extended, tx_req.is_rtr, data_slice).await {
                                Ok(()) => {
                                    info!("CAN TX: Enviado OK");
                                    let echo_frame = GsHostFrame::from_tx_msg_echo(
                                        tx_req.echo_id,
                                        tx_req.id,
                                        tx_req.is_extended,
                                        tx_req.dlc,
                                        &tx_req.data,
                                    );
                                    let _ = USB_ECHO_CHANNEL.try_send(echo_frame);
                                }
                                Err(bsp_f446::can::CanTxError::Timeout) => {
                                    warn!("CAN TX: Timeout — sin ACK del bus, mailboxes abortados");
                                }
                                Err(e) => {
                                    error!("CAN TX: Error al transmitir: {:?}", defmt::Debug2Format(&e));
                                }
                            }
                        }
                    }
                }
                Either::Second(Ok(env)) => {
                    let rx_frame = env.frame;
                    
                    let id: u32 = match rx_frame.id() {
                        embassy_stm32::can::Id::Standard(std) => std.as_raw() as u32,
                        embassy_stm32::can::Id::Extended(ext) => ext.as_raw() as u32,
                    };
                    let is_extended = matches!(rx_frame.id(), embassy_stm32::can::Id::Extended(_));
                    
                    let mut data = [0u8; 8];
                    let payload = rx_frame.data();
                    let dlc = payload.len();
                    if dlc <= 8 {
                        data[..dlc].copy_from_slice(payload);
                    }

                    info!("CAN RX: id=0x{:03X} ext={} dlc={} data={=[u8]:#X}", id, is_extended, dlc, &data[..dlc.min(8)]);

                    let generic_frame = can_protocol::CanFrame { id, is_extended, data, dlc: dlc as u8 };
                    CAN_RX_CHANNEL.send(generic_frame).await;
                }
                Either::Second(Err(e)) => {
                    can_error_count += 1;
                    let err_frame = match &e {
                        embassy_stm32::can::enums::BusError::Stuff => {
                            warn!("CAN RX error #{}, tipo: Stuff", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Stuff)
                        }
                        embassy_stm32::can::enums::BusError::Form => {
                            warn!("CAN RX error #{}, tipo: Form", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Form)
                        }
                        embassy_stm32::can::enums::BusError::Acknowledge => {
                            warn!("CAN RX error #{}, tipo: Acknowledge", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Acknowledge)
                        }
                        embassy_stm32::can::enums::BusError::BitRecessive => {
                            warn!("CAN RX error #{}, tipo: BitRecessive", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::BitRecessive)
                        }
                        embassy_stm32::can::enums::BusError::BitDominant => {
                            warn!("CAN RX error #{}, tipo: BitDominant", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::BitDominant)
                        }
                        embassy_stm32::can::enums::BusError::Crc => {
                            warn!("CAN RX error #{}, tipo: Crc", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Crc)
                        }
                        embassy_stm32::can::enums::BusError::Software => {
                            warn!("CAN RX error #{}, tipo: Software", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Software)
                        }
                    };
                    let _ = USB_ECHO_CHANNEL.try_send(err_frame);
                    if can.can.is_sleeping() {
                        info!("CAN: Recuperando de bus-off...");
                        can.can.enable().await;
                        can_error_count = 0;
                    }
                }
            }
        } else {
            let cmd = CAN_CMD_CHANNEL.receive().await;
            match cmd {
                CanDriverCmd::Start { loopback, listen_only, one_shot } => {
                    info!("CAN: Iniciando controlador (loopback={}, listen_only={}, one_shot={})...", loopback, listen_only, one_shot);
                    is_started = true;
                    can.start(loopback, listen_only, one_shot).await;
                }
                CanDriverCmd::Stop => {
                    info!("CAN: Ya estaba detenido");
                }
                CanDriverCmd::SetBitTiming(timing) => {
                    info!("CAN: Configurando Bit Timing: brp={}, prop={}, phase1={}, phase2={}, sjw={}", 
                        timing.brp, timing.prop_seg, timing.phase_seg1, timing.phase_seg2, timing.sjw);
                    can.set_bit_timing(&timing);
                }
                CanDriverCmd::Transmit(_) => {
                    warn!("CAN: Ignorando transmisión, controlador detenido");
                }
            }
        }
    }
}

/// Tarea de consumidor USB TX — Envía tramas CAN al host (RX del bus + echoes de TX).
///
/// También decodifica OBD2/UDS para logging, ya que es el único lugar que consume
/// `CAN_RX_CHANNEL`. De este modo `can_driver_task` no se ensucia con lógica de
/// protocolo y se mantiene enfocada en el hardware.
#[embassy_executor::task]
async fn usb_tx_task(mut ep_in: bsp_f446::usb::BspUsbEndpointIn) {
    ep_in.wait_enabled().await;
    info!("USB TX: Endpoint IN habilitado, listo para enviar tramas al host");

    loop {
        let host_frame = match select(CAN_RX_CHANNEL.receive(), USB_ECHO_CHANNEL.receive()).await {
            Either::First(frame) => {
                // Decodificación OBD2/UDS solo para logging (no altera el frame enviado al host).
                match can_protocol::analyze_frame(&frame) {
                    can_protocol::DecodedProtocol::Obd2Request(cmd) => {
                        defmt::info!("OBD2: {:?}", defmt::Debug2Format(&cmd));
                    }
                    can_protocol::DecodedProtocol::UdsMessage(msg) => {
                        defmt::info!("UDS: {:?}", defmt::Debug2Format(&msg));
                    }
                    can_protocol::DecodedProtocol::Raw => {}
                }

                GsHostFrame::from_can_frame(frame.id, frame.is_extended, frame.dlc, &frame.data)
            }
            Either::Second(echo_frame) => echo_frame,
        };

        let bytes = bytemuck::bytes_of(&host_frame);
        info!("USB TX: can_id=0x{:08X} dlc={} flags=0x{:02X} echo_id={}", host_frame.can_id, host_frame.can_dlc, host_frame.flags, host_frame.echo_id);

        match ep_in.write(bytes).await {
            Ok(n) => info!("USB TX OK: {} bytes enviados al host", n),
            Err(e) => error!("USB TX: Error al escribir al endpoint: {:?}", defmt::Debug2Format(&e)),
        }
    }
}

/// Tarea USB RX — Lee tramas del host por Bulk OUT y las encola para transmisión CAN.
#[embassy_executor::task]
async fn usb_rx_task(mut ep_out: bsp_f446::usb::BspUsbEndpointOut) {
    ep_out.wait_enabled().await;
    info!("USB RX: Endpoint OUT habilitado, listo para recibir tramas del host");

    let mut buf = [0u8; 64];
    loop {
        match ep_out.read(&mut buf).await {
            Ok(n) if n >= core::mem::size_of::<GsTxMsg>() => {
                let tx_msg: GsTxMsg = bytemuck::pod_read_unaligned(&buf[..core::mem::size_of::<GsTxMsg>()]);
                
                let dlc = tx_msg.dlc();
                info!("USB RX: TX request id=0x{:03X}, ext={}, dlc={}, echo_id={}", 
                    tx_msg.id(), tx_msg.is_extended(), dlc, tx_msg.echo_id);
                let tx_req = CanTxRequest {
                    echo_id: tx_msg.echo_id,
                    id: tx_msg.id(),
                    is_extended: tx_msg.is_extended(),
                    is_rtr: tx_msg.is_rtr(),
                    data: tx_msg.data,
                    dlc,
                };

                match CAN_CMD_CHANNEL.try_send(CanDriverCmd::Transmit(tx_req)) {
                    Ok(()) => {}
                    Err(_) => {
                        warn!("USB RX: Cola CAN CMD llena, descartando trama");
                    }
                }
            }
            Ok(n) => {
                warn!("USB RX: Trama incompleta ({} bytes, esperados {})", n, core::mem::size_of::<GsTxMsg>());
            }
            Err(e) => {
                error!("USB RX: Error al leer del endpoint: {:?}", defmt::Debug2Format(&e));
            }
        }
    }
}
