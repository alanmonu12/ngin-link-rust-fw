use embassy_stm32::usb::{Driver, InterruptHandler};
use embassy_stm32::{bind_interrupts, peripherals};
use embassy_usb::{Builder, UsbDevice};
use static_cell::StaticCell;

// Importamos la lógica pura desde nuestro nuevo crate en el workspace
use gs_usb_protocol::default_gs_usb_config;
use gs_usb_protocol::handler::GsUsbControlHandler;

// Buffers de memoria estática que necesita el USB
static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static CONTROL_HANDLER: StaticCell<GsUsbControlHandler> = StaticCell::new();
static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();

// Enlazamos la interrupción aquí, ocultándola del main
bind_interrupts!(pub struct Irqs {
    OTG_FS => InterruptHandler<peripherals::USB_OTG_FS>;
});

// Alias de tipos para exportar al exterior sin exponer los detalles genéricos del STM32
pub type BspUsbDriver = Driver<'static, peripherals::USB_OTG_FS>;
pub type BspUsbDevice = UsbDevice<'static, BspUsbDriver>;

// Exportamos el buffer para inicializar el Driver desde lib.rs
pub fn get_ep_out_buffer() -> &'static mut [u8; 256] {
    EP_OUT_BUFFER.init([0; 256])
}

// Función pública para construir el dispositivo USB
pub fn init_usb(driver: BspUsbDriver) -> BspUsbDevice {
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
pub async fn usb_task(mut usb: BspUsbDevice) -> ! {
    usb.run().await
}
