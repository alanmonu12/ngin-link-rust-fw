use embassy_usb::{Builder, UsbDevice};
use embassy_usb::driver::Driver;
use gs_usb_protocol::handler::GsUsbControlHandler;
use gs_usb_protocol::gs_usb_types::{GsDeviceCapabilities, GS_CAN_FEATURE_IDENTIFY, GS_CAN_FEATURE_LISTEN_ONLY, GS_CAN_FEATURE_LOOP_BACK, GS_CAN_FEATURE_USER_ID};
use gs_usb_protocol::gs_usb_types::GsDeviceBitTiming;
use static_cell::StaticCell;

static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static CONTROL_HANDLER: StaticCell<GsUsbControlHandler> = StaticCell::new();

pub type UsbEpIn<D> = <D as Driver<'static>>::EndpointIn;
pub type UsbEpOut<D> = <D as Driver<'static>>::EndpointOut;

pub struct UsbStack<D: Driver<'static>> {
    pub device: UsbDevice<'static, D>,
    pub ep_in: UsbEpIn<D>,
    pub ep_out: UsbEpOut<D>,
}

pub fn build_usb_stack<D: Driver<'static>>(
    driver: D,
    config_usb: embassy_usb::Config<'static>,
    on_start: fn(u32),
    on_stop: fn(),
    on_bit_timing: fn(GsDeviceBitTiming),
    on_identify: fn(bool),
    now_ms: fn() -> u32,
) -> UsbStack<D> {
    let mut builder = Builder::new(
        driver,
        config_usb,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        MSOS_DESC.init([0; 256]),
        CONTROL_BUF.init([0; 64]),
    );

    let control_handler = CONTROL_HANDLER.init(GsUsbControlHandler {
        on_start: Some(on_start),
        on_stop: Some(on_stop),
        on_bit_timing: Some(on_bit_timing),
        on_identify: Some(on_identify),
        now_ms,
        user_id: 0,
        capabilities: GsDeviceCapabilities::default()
            .with_feature(GS_CAN_FEATURE_LISTEN_ONLY)
            .with_feature(GS_CAN_FEATURE_LOOP_BACK)
            .with_feature(GS_CAN_FEATURE_IDENTIFY)
            .with_feature(GS_CAN_FEATURE_USER_ID),
    });
    builder.handler(control_handler);

    let (ep_in, ep_out) = {
        let mut function = builder.function(0xFF, 0xFF, 0xFF);
        let mut interface = function.interface();
        let mut alt_setting = interface.alt_setting(0xFF, 0xFF, 0xFF, None);
        let ep_in = alt_setting.endpoint_bulk_in(None, 64);
        let ep_out = alt_setting.endpoint_bulk_out(None, 64);
        (ep_in, ep_out)
    };

    let device = builder.build();

    UsbStack { device, ep_in, ep_out }
}