use embassy_stm32::usb::{Driver, InterruptHandler};
use embassy_stm32::{bind_interrupts, peripherals};
use static_cell::StaticCell;

static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();

// Enlazamos la interrupción aquí, ocultándola del main
bind_interrupts!(pub struct Irqs {
    OTG_FS => InterruptHandler<peripherals::USB_OTG_FS>;
});

// Alias de tipos para exportar al exterior sin exponer los detalles genéricos del STM32
pub type BspUsbDriver = Driver<'static, peripherals::USB_OTG_FS>;

// Exportamos el buffer para inicializar el Driver desde lib.rs
pub fn get_ep_out_buffer() -> &'static mut [u8; 256] {
    EP_OUT_BUFFER.init([0; 256])
}
