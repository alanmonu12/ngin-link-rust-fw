#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;

use {defmt_rtt as _, panic_probe as _};

// La macro #[embassy_executor::main] configura el entorno asíncrono por ti
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // 1. Inicializamos TODO el hardware a través del BSP (Relojes, pines, y construcción del USB)
    // Ahora `main.rs` no sabe si es un STM32, un RP2040, o un ESP32.
    let bsp = bsp_f446::init();
    info!("Hardware y relojes configurados. Iniciando driver USB...");

    // 2. Lanzamos la tarea de fondo del USB
    spawner.spawn(bsp_f446::usb::usb_task(bsp.usb_device).unwrap());
    spawner.spawn(can_to_usb_router_task(bsp.can_driver).unwrap());

    info!("¡Sistema configurado y listo!");

    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}


#[embassy_executor::task]
async fn can_to_usb_router_task(mut can: bsp_f446::can::BspCan /*, mut usb_in_ep: BulkInEndpoint ... */) {
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

            // 3. Usamos nuestro analizador
            let decoded = can_protocol::analyze_frame(&generic_frame);
            match decoded {
                can_protocol::DecodedProtocol::Obd2Request(cmd) => defmt::info!("OBD2: {:?}", defmt::Debug2Format(&cmd)),
                can_protocol::DecodedProtocol::UdsMessage(msg) => defmt::info!("UDS: {:?}", defmt::Debug2Format(&msg)),
                can_protocol::DecodedProtocol::Raw => {} // No loggeamos los miles de Raw
            }

            // 4. Empaquetar el frame al formato `gs_usb` (GsHostFrame) y mandar por USB...
        }
    }
}

// firmware/src/main.rs
