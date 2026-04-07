#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;


// Declaramos nuestro nuevo módulo
mod usb;

//use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::rcc::{Hse, HseMode, Pll, APBPrescaler};
use embassy_stm32::usb::Driver;
use embassy_stm32::usb::Config as UsbConfig;
use embassy_stm32::{Config, bind_interrupts, peripherals};

use {defmt_rtt as _, panic_probe as _};


// 1. Enlazamos la interrupción de hardware del USB al driver de Embassy
bind_interrupts!(struct Irqs {
    OTG_FS => embassy_stm32::usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

// La macro #[embassy_executor::main] configura el entorno asíncrono por ti
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // 2. Configuramos el microcontrolador
    let mut config = Config::default();

    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::Hertz(8_000_000), // Frecuencia del cristal externo
        mode: HseMode::Bypass, // Usamos el cristal externo en modo bypass
    });

    config.rcc.pll_src = embassy_stm32::rcc::PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: embassy_stm32::rcc::PllPreDiv::DIV4, // 8MHz / 4 = 2MHz
        mul: embassy_stm32::rcc::PllMul::MUL168, // 2MHz * 336 = 672MHz
        divp: Some(embassy_stm32::rcc::PllPDiv::DIV4), // 336MHz / 4 = 84MHz (Sysclk máximo del F401)
        divq: Some(embassy_stm32::rcc::PllQDiv::DIV7), // 336MHz / 7 = 48MHz (¡Reloj exacto para el USB!)
        divr: None,
    });

    config.rcc.sys = embassy_stm32::rcc::Sysclk::PLL1_P;

    // Límites del F446: APB1 = max 45MHz, APB2 = max 90MHz
    config.rcc.apb1_pre = APBPrescaler::DIV4; // 168 MHz / 4 = 42 MHz (Seguro, menor a 45)
    config.rcc.apb2_pre = APBPrescaler::DIV2; // 168 MHz / 2 = 84 MHz (Seguro, menor a 90)

    let p = embassy_stm32::init(config);
    info!("Relojes configurados. Iniciando driver USB...");

    // 3. Reservamos memoria estática para el FIFO RX (El contenedor general de recepción)
    static EP_OUT_BUFFER: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    let ep_out_buffer = EP_OUT_BUFFER.init([0; 256]);

    // 4. Inicializamos el periférico. Le pasamos la interrupción y los pines PA11 y PA12.
    let mut usb_config = UsbConfig::default();
    usb_config.vbus_detection = false; // La Nucleo no enruta VBUS por defecto

    let driver = Driver::new_fs(p.USB_OTG_FS, Irqs, p.PA12, p.PA11, ep_out_buffer, usb_config);

    // Llamamos a nuestro módulo para construir el USB
    let usb_device = usb::build_usb_device(driver);

    // 5. Lanzamos la tarea de fondo del USB
    spawner.spawn(usb::usb_task(usb_device).unwrap());

    info!("¡Sistema configurado y listo!");

    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}
