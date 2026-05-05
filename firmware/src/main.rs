#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

use {defmt_rtt as _, panic_probe as _};

// Cola (Channel) para comunicar la tarea del CAN (Productor) con la del USB (Consumidor).
// Usamos CriticalSectionRawMutex y una capacidad de 32 mensajes (ajustable según RAM/necesidad).
static CAN_RX_CHANNEL: Channel<CriticalSectionRawMutex, can_protocol::CanFrame, 32> = Channel::new();

// La macro #[embassy_executor::main] configura el entorno asíncrono por ti
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // 1. Inicializamos TODO el hardware a través del BSP (Relojes, pines, y construcción del USB)
    // Ahora `main.rs` no sabe si es un STM32, un RP2040, o un ESP32.
    let bsp = bsp_f446::init();
    info!("Hardware y relojes configurados. Iniciando driver USB...");

    // 2. Lanzamos la tarea de fondo del USB
    spawner.spawn(usb_task(bsp.usb_device).unwrap());
    
    spawner.spawn(can_rx_task(bsp.can_driver).unwrap());
    spawner.spawn(usb_tx_task().unwrap());

    info!("¡Sistema configurado y listo!");

    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}


#[embassy_executor::task]
async fn usb_task(mut usb: bsp_f446::usb::BspUsbDevice) -> ! {
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
