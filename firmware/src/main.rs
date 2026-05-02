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

    info!("¡Sistema configurado y listo!");

    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}
