#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_usb::{Builder, UsbDevice};
use static_cell::StaticCell;
use gs_usb_protocol::{default_gs_usb_config, handler::GsUsbControlHandler};

use {defmt_rtt as _, panic_probe as _};

// Cola (Channel) para comunicar la tarea del CAN (Productor) con la del USB (Consumidor).
// Usamos CriticalSectionRawMutex y una capacidad de 32 mensajes (ajustable según RAM/necesidad).
static CAN_RX_CHANNEL: Channel<CriticalSectionRawMutex, can_protocol::CanFrame, 32> = Channel::new();

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

    let control_handler = CONTROL_HANDLER.init(GsUsbControlHandler);
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
    loop {
        // 1. Esperamos asíncronamente a que llegue un mensaje del CAN
        if let Ok(env) = can.read().await {
            let rx_frame = env.frame;
            
            // Adaptamos usando la API de embassy_stm32
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

            // 2. Usamos nuestro analizador al vuelo para logs
            let decoded = can_protocol::analyze_frame(&generic_frame);
            match decoded {
                can_protocol::DecodedProtocol::Obd2Request(cmd) => defmt::info!("OBD2: {:?}", defmt::Debug2Format(&cmd)),
                can_protocol::DecodedProtocol::UdsMessage(msg) => defmt::info!("UDS: {:?}", defmt::Debug2Format(&msg)),
                can_protocol::DecodedProtocol::Raw => {} // No loggeamos los miles de Raw
            }

            // 3. Enviamos a la cola para que la tarea del USB lo procese luego.
            CAN_RX_CHANNEL.send(generic_frame).await;
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
