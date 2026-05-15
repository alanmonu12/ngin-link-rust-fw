use embassy_stm32::bind_interrupts;
use embassy_stm32::can::{Can, Rx0InterruptHandler, Rx1InterruptHandler, SceInterruptHandler, TxInterruptHandler};
use embassy_stm32::peripherals;

// Enlazamos las 4 interrupciones requeridas por el bxCAN
bind_interrupts!(pub struct Irqs {
    CAN1_RX0 => Rx0InterruptHandler<peripherals::CAN1>;
    CAN1_RX1 => Rx1InterruptHandler<peripherals::CAN1>;
    CAN1_SCE => SceInterruptHandler<peripherals::CAN1>;
    CAN1_TX => TxInterruptHandler<peripherals::CAN1>;
});

pub struct BspCan {
    // Hacemos el campo público para que lib.rs pueda inicializarlo al instanciar el Board.
    pub inner: Can<'static>,
}

impl BspCan {

    pub fn start(&mut self) {
        // Para "encender", devolvemos el bus a su modo de operación normal.
        // En la API bxCAN de Embassy, la función modify_config automáticamente entra en modo 
        // de inicialización (Init) temporalmente y vuelve al modo normal al terminar el closure.
        self.inner.modify_config()
            .set_silent(false)
            .set_loopback(false);
    }

    pub fn stop(&mut self) {
        // Para simular que el bus está "detenido" sin desconfigurar el hardware completo,
        // lo ponemos en modo silencioso (Listen-only/Silent). De este modo no acusa recibo (ACK)
        // ni interfiere físicamente con otros dispositivos en la red CAN.
        self.inner.modify_config()
            .set_silent(true);
    }

    pub fn set_bit_timing(&mut self, timing: &gs_usb_protocol::gs_usb_types::GsDeviceBitTiming) {
        // En la versión 0.6.0 de embassy-stm32, ya no manipulamos el registro BTR directamente.
        // Usamos NominalBitTiming, que recibe los valores reales (sin restar 1).
        // En la arquitectura CAN del STM32, prop_seg y phase_seg1 están combinados.
        // Usamos .max(1) para garantizar que los valores NonZero nunca sean 0 y evitar pánicos.
        let bt = embassy_stm32::can::util::NominalBitTiming {
            sync_jump_width: core::num::NonZeroU8::new((timing.sjw as u8).max(1)).unwrap(),
            seg1: core::num::NonZeroU8::new((timing.prop_seg + timing.phase_seg1) as u8).unwrap(),
            seg2: core::num::NonZeroU8::new(timing.phase_seg2 as u8).unwrap(),
            prescaler: core::num::NonZeroU16::new((timing.brp as u16).max(1)).unwrap(),
        };

        self.inner.modify_config()
            .set_bit_timing(bt);
    }
}
