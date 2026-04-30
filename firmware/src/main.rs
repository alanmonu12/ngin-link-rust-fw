#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;

//use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::usb::{Driver, Config as UsbConfig};
use embassy_stm32::{bind_interrupts, peripherals};

use {defmt_rtt as _, panic_probe as _};


// 1. Enlazamos la interrupción de hardware del USB al driver de Embassy
bind_interrupts!(struct Irqs {
    OTG_FS => embassy_stm32::usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

// La macro #[embassy_executor::main] configura el entorno asíncrono por ti
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // 2. Inicializamos el microcontrolador y los relojes desde el BSP
    let p = bsp_f446::init();
    info!("Relojes configurados. Iniciando driver USB...");

    // 3. Reservamos memoria estática para el FIFO RX (El contenedor general de recepción)
    static EP_OUT_BUFFER: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    let ep_out_buffer = EP_OUT_BUFFER.init([0; 256]);

    // 4. Inicializamos el periférico. Le pasamos la interrupción y los pines PA11 y PA12.
    let mut usb_config = UsbConfig::default();
    usb_config.vbus_detection = false; // La Nucleo no enruta VBUS por defecto

    let driver = Driver::new_fs(p.USB_OTG_FS, Irqs, p.PA12, p.PA11, ep_out_buffer, usb_config);

    // Llamamos al BSP para construir el USB
    let usb_device = bsp_f446::usb::build_usb_device(driver);

    // 5. Lanzamos la tarea de fondo del USB
    spawner.spawn(bsp_f446::usb::usb_task(usb_device).unwrap());

    info!("¡Sistema configurado y listo!");

    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}
