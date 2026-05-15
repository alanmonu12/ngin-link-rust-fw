#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_usb::{Builder, UsbDevice};
use static_cell::StaticCell;
use gs_usb_protocol::{default_gs_usb_config, handler::GsUsbControlHandler};
use embassy_futures::select::{select, Either};

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
    
    // Declaramos la interfaz de gs_usb
    {
        let mut function = builder.function(0xFF, 0xFF, 0xFF);
        let mut interface = function.interface();
        let mut alt_setting = interface.alt_setting(0xFF, 0xFF, 0xFF, None);
        let _ep_in = alt_setting.endpoint_bulk_in(None, 64);
        let _ep_out = alt_setting.endpoint_bulk_out(None, 64);
    }

    let usb_device = builder.build();

    // 2. Lanzamos la tarea de fondo del USB
    spawner.spawn(usb_task(usb_device).unwrap());
    
    spawner.spawn(can_rx_task(board.can_driver).unwrap());
    spawner.spawn(usb_tx_task().unwrap());

    info!("¡Sistema configurado y listo!");

    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}


#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, bsp_f446::usb::BspUsbDriver>) -> ! {
    usb.run().await
}


// Tarea 1: Productor (Solo lee del hardware CAN lo más rápido posible)
#[embassy_executor::task]
async fn can_rx_task(mut can: bsp_f446::can::BspCan) {
    // El dispositivo arranca detenido esperando configuración del host (gs_usb)
    let mut is_started = false;

    loop {
        if is_started {
            // Escuchamos comandos de control Y tramas del bus de forma simultánea
            match select(CAN_CTRL_CHANNEL.receive(), can.inner.read()).await {
                Either::First(cmd) => {
                    match cmd {
                        CanCommand::Stop => {
                            info!("CAN: Apagando controlador por comando USB...");
                            is_started = false;
                            // El BSP se encarga de los detalles de hardware
                            can.stop();
                        }
                        CanCommand::Start => info!("CAN: Ya estaba iniciado"),
                        CanCommand::SetBitTiming(_) => info!("CAN: Debes hacer STOP antes de cambiar la velocidad"),
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

                    let generic_frame = can_protocol::CanFrame { id, is_extended, data, dlc: dlc as u8 };

                    let decoded = can_protocol::analyze_frame(&generic_frame);
                    match decoded {
                        can_protocol::DecodedProtocol::Obd2Request(cmd) => defmt::info!("OBD2: {:?}", defmt::Debug2Format(&cmd)),
                        can_protocol::DecodedProtocol::UdsMessage(msg) => defmt::info!("UDS: {:?}", defmt::Debug2Format(&msg)),
                        can_protocol::DecodedProtocol::Raw => {} 
                    }

                    CAN_RX_CHANNEL.send(generic_frame).await;
                }
                Either::Second(Err(_)) => {} // Manejo opcional de errores del bus
            }
        } else {
            // Si estamos detenidos, solo esperamos comandos de control (bloqueante)
            let cmd = CAN_CTRL_CHANNEL.receive().await;
            match cmd {
                CanCommand::Start => {
                    info!("CAN: Iniciando controlador...");
                    is_started = true;
                    // El BSP encapsula cómo encender el periférico
                    can.start();
                }
                CanCommand::Stop => info!("CAN: Ya estaba detenido"),
                CanCommand::SetBitTiming(timing) => {
                    info!("CAN: Configurando Bit Timing: brp={}, prop={}, phase1={}, phase2={}, sjw={}", 
                        timing.brp, timing.prop_seg, timing.phase_seg1, timing.phase_seg2, timing.sjw);
                    
                    // Pasamos la configuración al BSP para que él manipule los registros bxCAN
                    can.set_bit_timing(&timing);
                }
            }
        }
    }
}

// Tarea 2: Consumidor (Saca de la cola y en el futuro escribirá al USB)
#[embassy_executor::task]
async fn usb_tx_task(/* mut usb_in_ep: BulkInEndpoint ... */) {
    loop {
        // Esperamos dormidos a que la tarea del CAN RX meta algo en la cola
        let _frame = CAN_RX_CHANNEL.receive().await;
        
        // (Por ahora no hacemos nada más. En el futuro armaremos GsHostFrame y mandaremos a la PC)
    }
}

// firmware/src/main.rs
