use embassy_usb::UsbDevice;

#[embassy_executor::task]
pub async fn usb_run(mut usb: UsbDevice<'static, bsp_f446::usb::BspUsbDriver>) {
    usb.run().await
}