use embassy_stm32::usb::Driver;
use embassy_stm32::peripherals::USB_OTG_FS;
use embassy_usb::{Builder, UsbDevice};
use static_cell::StaticCell;

// Importamos la lógica pura desde nuestro nuevo crate en el workspace
use ngin_usb_protocol::default_gs_usb_config;
use ngin_usb_protocol::handler::GsUsbControlHandler;

// Buffers de memoria estática que necesita el USB
static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static CONTROL_HANDLER: StaticCell<GsUsbControlHandler> = StaticCell::new();

// Función pública para construir el dispositivo USB
pub fn build_usb_device(driver: Driver<'static, USB_OTG_FS>) -> UsbDevice<'static, Driver<'static, USB_OTG_FS>> {
    let config_usb = default_gs_usb_config();

    let mut builder = Builder::new(
        driver,
        config_usb,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        MSOS_DESC.init([0; 256]),
        CONTROL_BUF.init([0; 64]),
    );

    let control_handler = CONTROL_HANDLER.init(GsUsbControlHandler);

    builder.handler(control_handler);
    
    // Declaramos la interfaz de gs_usb
    {
        let mut function = builder.function(0xFF, 0xFF, 0xFF);
        let mut interface = function.interface();
        let mut alt_setting = interface.alt_setting(0xFF, 0xFF, 0xFF, None);
        let _ep_in = alt_setting.endpoint_bulk_in(None, 64);
        let _ep_out = alt_setting.endpoint_bulk_out(None, 64);
    }

    builder.build()
}

// La tarea de ejecución continua (Loop)
#[embassy_executor::task]
pub async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB_OTG_FS>>) -> ! {
    usb.run().await
}
