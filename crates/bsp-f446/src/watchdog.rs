use embassy_stm32::wdg::IndependentWatchdog;
use embassy_stm32::peripherals::IWDG;

pub struct BspWatchdog {
    wdg: IndependentWatchdog<'static, IWDG>,
}

impl BspWatchdog {
    pub fn new(iwdg: embassy_stm32::Peri<'static, IWDG>, timeout_ms: u32) -> Self {
        let wdg = IndependentWatchdog::new(iwdg, timeout_ms * 1000);
        Self { wdg }
    }

    pub fn unleash(&mut self) {
        self.wdg.unleash();
    }

    pub fn pet(&mut self) {
        self.wdg.pet();
    }
}