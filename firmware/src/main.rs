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
use gs_usb_protocol::gs_usb_types::{GsHostFrame, GsTxMsg};
use embassy_futures::select::{select, select3, Either, Either3};

use {defmt_rtt as _, panic_probe as _};

// Cola (Channel) para comunicar la tarea del CAN (Productor) con la del USB (Consumidor).
// Usamos CriticalSectionRawMutex y una capacidad de 32 mensajes (ajustable según RAM/necesidad).
static CAN_RX_CHANNEL: Channel<CriticalSectionRawMutex, can_protocol::CanFrame, 32> = Channel::new();

// Canal de control para mandar los comandos de configuración del USB al CAN
enum CanCommand {
    Start,
    Stop,
    SetBitTiming(gs_usb_protocol::gs_usb_types::GsDeviceBitTiming),
}
static CAN_CTRL_CHANNEL: Channel<CriticalSectionRawMutex, CanCommand, 4> = Channel::new();

struct CanTxRequest {
    echo_id: u32,
    id: u32,
    is_extended: bool,
    is_rtr: bool,
    data: [u8; 8],
    dlc: u8,
}
static CAN_TX_CHANNEL: Channel<CriticalSectionRawMutex, CanTxRequest, 16> = Channel::new();

static USB_ECHO_CHANNEL: Channel<CriticalSectionRawMutex, GsHostFrame, 16> = Channel::new();

fn on_start_cb() {
    let _ = CAN_CTRL_CHANNEL.try_send(CanCommand::Start);
}
fn on_stop_cb() {
    let _ = CAN_CTRL_CHANNEL.try_send(CanCommand::Stop);
}
fn on_bit_timing_cb(timing: gs_usb_protocol::gs_usb_types::GsDeviceBitTiming) {
    let _ = CAN_CTRL_CHANNEL.try_send(CanCommand::SetBitTiming(timing));
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
    
    spawner.spawn(can_rx_task(board.can_driver).unwrap());
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


// Tarea 1: Productor (Lee del hardware CAN y transmite tramas desde el host)
#[embassy_executor::task]
async fn can_rx_task(mut can: bsp_f446::can::BspCan) {
    let mut is_started = false;

    loop {
        if is_started {
            match select3(CAN_CTRL_CHANNEL.receive(), can.can.read(), CAN_TX_CHANNEL.receive()).await {
                Either3::First(cmd) => {
                    match cmd {
                        CanCommand::Stop => {
                            info!("CAN: Apagando controlador por comando USB...");
                            is_started = false;
                            can.stop();
                        }
                        CanCommand::Start => info!("CAN: Ya estaba iniciado"),
                        CanCommand::SetBitTiming(_) => info!("CAN: Debes hacer STOP antes de cambiar la velocidad"),
                    }
                }
                Either3::Second(Ok(env)) => {
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

                    let generic_frame = can_protocol::CanFrame { id, is_extended, data, dlc: dlc as u8 };

                    let decoded = can_protocol::analyze_frame(&generic_frame);
                    match decoded {
                        can_protocol::DecodedProtocol::Obd2Request(cmd) => defmt::info!("OBD2: {:?}", defmt::Debug2Format(&cmd)),
                        can_protocol::DecodedProtocol::UdsMessage(msg) => defmt::info!("UDS: {:?}", defmt::Debug2Format(&msg)),
                        can_protocol::DecodedProtocol::Raw => {} 
                    }

                    CAN_RX_CHANNEL.send(generic_frame).await;
                }
                Either3::Second(Err(_)) => {}
                Either3::Third(tx_req) => {
                    let data_slice = &tx_req.data[..tx_req.dlc as usize];
                    match can.transmit(tx_req.id, tx_req.is_extended, tx_req.is_rtr, data_slice).await {
                        Ok(()) => {
                            let echo_frame = GsHostFrame::from_tx_msg_echo(
                                tx_req.echo_id,
                                tx_req.id,
                                tx_req.is_extended,
                                tx_req.dlc,
                                &tx_req.data,
                            );
                            let _ = USB_ECHO_CHANNEL.try_send(echo_frame);
                        }
                        Err(e) => {
                            error!("CAN TX: Error al transmitir: {:?}", defmt::Debug2Format(&e));
                        }
                    }
                }
            }
        } else {
            let cmd = CAN_CTRL_CHANNEL.receive().await;
            match cmd {
                CanCommand::Start => {
                    info!("CAN: Iniciando controlador...");
                    is_started = true;
                    can.start();
                }
                CanCommand::Stop => info!("CAN: Ya estaba detenido"),
                CanCommand::SetBitTiming(timing) => {
                    info!("CAN: Configurando Bit Timing: brp={}, prop={}, phase1={}, phase2={}, sjw={}", 
                        timing.brp, timing.prop_seg, timing.phase_seg1, timing.phase_seg2, timing.sjw);
                    can.set_bit_timing(&timing);
                }
            }
        }
    }
}

// Tarea 2: Consumidor USB TX — Envía tramas CAN al host (RX del bus + echoes de TX)
#[embassy_executor::task]
async fn usb_tx_task(mut ep_in: bsp_f446::usb::BspUsbEndpointIn) {
    ep_in.wait_enabled().await;
    info!("USB TX: Endpoint IN habilitado, listo para enviar tramas al host");

    loop {
        let host_frame = match select(CAN_RX_CHANNEL.receive(), USB_ECHO_CHANNEL.receive()).await {
            Either::First(frame) => {
                GsHostFrame::from_can_frame(frame.id, frame.is_extended, frame.dlc, &frame.data)
            }
            Either::Second(echo_frame) => echo_frame,
        };

        let bytes = bytemuck::bytes_of(&host_frame);

        match ep_in.write(bytes).await {
            Ok(()) => {}
            Err(e) => {
                error!("USB TX: Error al escribir al endpoint: {:?}", defmt::Debug2Format(&e));
            }
        }
    }
}

// Tarea 3: USB RX — Lee tramas del host por Bulk OUT y las envía al bus CAN
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
                let tx_req = CanTxRequest {
                    echo_id: tx_msg.echo_id,
                    id: tx_msg.id(),
                    is_extended: tx_msg.is_extended(),
                    is_rtr: tx_msg.is_rtr(),
                    data: tx_msg.data,
                    dlc,
                };

                match CAN_TX_CHANNEL.try_send(tx_req) {
                    Ok(()) => {}
                    Err(_) => {
                        warn!("USB RX: Cola CAN TX llena, descartando trama");
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
