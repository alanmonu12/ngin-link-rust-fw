use embassy_usb::UsbDevice;
use crate::app_context::USB_DEVICE_READY;

#[embassy_executor::task]
pub async fn usb_run(mut usb: UsbDevice<'static, bsp_f446::usb::BspUsbDriver>) {
    USB_DEVICE_READY.signal(());
    usb.run().await
}