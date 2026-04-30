#![no_std]
pub mod usb;

use embassy_stm32::rcc::{Hse, HseMode, Pll, APBPrescaler, PllSource, PllPreDiv, PllMul, PllPDiv, PllQDiv, Sysclk};
use embassy_stm32::{Config, Peripherals};

/// Inicializa el microcontrolador y configura los relojes (RCC y PLL)
pub fn init() -> Peripherals {
    let mut config = Config::default();

    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::Hertz(8_000_000), // Frecuencia del cristal externo
        mode: HseMode::Bypass, // Usamos el cristal externo en modo bypass
    });

    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV4, // 8MHz / 4 = 2MHz
        mul: PllMul::MUL168, // 2MHz * 168 = 336MHz
        divp: Some(PllPDiv::DIV4), // 336MHz / 4 = 84MHz (Sysclk máximo del F446)
        divq: Some(PllQDiv::DIV7), // 336MHz / 7 = 48MHz (¡Reloj exacto para el USB!)
        divr: None,
    });

    config.rcc.sys = Sysclk::PLL1_P; // Usamos el PLL como fuente del sistema

    // Límites del F446: APB1 = max 45MHz, APB2 = max 90MHz
    config.rcc.apb1_pre = APBPrescaler::DIV2; // 168 MHz / 2 = 42 MHz (Seguro, menor a 45)
    config.rcc.apb2_pre = APBPrescaler::DIV1; // 168 MHz / 1 = 84 MHz (Seguro, menor a 90)

    embassy_stm32::init(config)
}