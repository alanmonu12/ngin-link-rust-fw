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

// Alias de tipo para ocultar los detalles de los pines al main.rs
pub type BspCan = Can<'static>;