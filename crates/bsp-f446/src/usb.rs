use embassy_stm32::usb::{Driver, InterruptHandler};
use embassy_stm32::{bind_interrupts, peripherals};
use static_cell::StaticCell;

static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();

bind_interrupts!(pub struct Irqs {
    OTG_FS => InterruptHandler<peripherals::USB_OTG_FS>;
});

pub type BspUsbDriver = Driver<'static, peripherals::USB_OTG_FS>;

pub type BspUsbEndpointIn = <BspUsbDriver as embassy_usb::driver::Driver<'static>>::EndpointIn;
pub type BspUsbEndpointOut = <BspUsbDriver as embassy_usb::driver::Driver<'static>>::EndpointOut;

pub fn get_ep_out_buffer() -> &'static mut [u8; 256] {
    EP_OUT_BUFFER.init([0; 256])
}
