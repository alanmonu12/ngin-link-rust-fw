#![no_std]

pub mod can;
pub mod usb;
pub mod watchdog;

use defmt::*;
use embassy_stm32::rcc::{Hse, HseMode, Pll, APBPrescaler, PllSource, PllPreDiv, PllMul, PllPDiv, PllQDiv, Sysclk};
use embassy_stm32::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum ResetReason {
    PinReset,
    Por,
    Software,
    Iwdg,
    Wwdg,
    LowPower,
    Unknown,
}

pub struct Board {
    pub usb_driver: usb::BspUsbDriver,
    pub can_driver: can::BspCan,
    pub iwdg: embassy_stm32::Peri<'static, embassy_stm32::peripherals::IWDG>,
    pub reset_reason: ResetReason,
}

const RCC_CSR_ADDR: usize = 0x4002_1050;

fn read_reset_reason() -> ResetReason {
    let csr = unsafe { core::ptr::read_volatile(RCC_CSR_ADDR as *const u32) };

    if csr & (1 << 29) != 0 {
        ResetReason::Iwdg
    } else if csr & (1 << 30) != 0 {
        ResetReason::Wwdg
    } else if csr & (1 << 28) != 0 {
        ResetReason::Software
    } else if csr & (1 << 26) != 0 {
        ResetReason::LowPower
    } else if csr & (1 << 27) != 0 {
        ResetReason::PinReset
    } else if csr & (1 << 25) != 0 {
        ResetReason::Por
    } else {
        ResetReason::Unknown
    }
}

fn clear_reset_flags() {
    let csr = unsafe { core::ptr::read_volatile(RCC_CSR_ADDR as *const u32) };
    unsafe {
        core::ptr::write_volatile(RCC_CSR_ADDR as *mut u32, csr | (1 << 24));
    }
}

pub fn init() -> Result<Board, ()> {
    let reset_reason = read_reset_reason();
    info!("BSP: Reset = {:?}", reset_reason);
    clear_reset_flags();

    let mut config = Config::default();

    trace!("BSP: Configurando relojes (HSE=8MHz, PLL→84MHz, USB=48MHz)...");

    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::Hertz(8_000_000),
        mode: HseMode::Bypass,
    });

    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL168,
        divp: Some(PllPDiv::DIV4),
        divq: Some(PllQDiv::DIV7),
        divr: None,
    });

    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.apb1_pre = APBPrescaler::DIV2;
    config.rcc.apb2_pre = APBPrescaler::DIV1;

    let p = embassy_stm32::init(config);
    trace!("BSP: Relojes OK (SYS=84MHz, USB=48MHz)");

    let mut usb_config = embassy_stm32::usb::Config::default();
    usb_config.vbus_detection = false;

    trace!("BSP: Inicializando USB OTG FS...");
    let driver = embassy_stm32::usb::Driver::new_fs(
        p.USB_OTG_FS,
        usb::Irqs,
        p.PA12,
        p.PA11,
        usb::get_ep_out_buffer(),
        usb_config,
    );
    trace!("BSP: USB OTG FS OK");

    trace!("BSP: Inicializando CAN1 (PB8=RX, PB9=TX)...");
    let can_p = embassy_stm32::can::Can::new(p.CAN1, p.PB8, p.PB9, can::Irqs);
    trace!("BSP: CAN1 OK (en init mode)");

    Ok(Board {
        usb_driver: driver,
        can_driver: can::BspCan { can: can_p },
        iwdg: p.IWDG,
        reset_reason,
    })
}