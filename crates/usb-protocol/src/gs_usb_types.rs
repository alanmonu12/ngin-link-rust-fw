use bytemuck::{Pod, Zeroable};

/// The gs_usb driver will send this request to get the device's capabilities.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default)]
pub struct GsDeviceConfig {
    pub reserved1: u8,
    pub reserved2: u8,
    pub reserved3: u8,
    /// Number of CAN interfaces
    pub interface_count: u8,
    pub sw_version: u32,
    pub hw_version: u32,
}

/// The gs_usb driver will send this request to get the bittiming capabilities
/// of the CAN peripheral.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GsDeviceBtConst {
    /// Supported features: GS_CAN_FEATURE_LISTEN_ONLY, GS_CAN_FEATURE_LOOP_BACK, etc.
    pub feature: u32,
    /// The clock frequency of the CAN peripheral in Hz.
    pub fclk_can: u32,
    pub tseg1_min: u32,
    pub tseg1_max: u32,
    pub tseg2_min: u32,
    pub tseg2_max: u32,
    pub sjw_max: u32,
    pub brp_min: u32,
    pub brp_max: u32,
    pub brp_inc: u32,
}

/// Data structure for GS_USB_BREQ_BITTIMING.
/// Received from the host to configure the CAN bus speed.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GsDeviceBitTiming {
    pub prop_seg: u32,
    pub phase_seg1: u32,
    pub phase_seg2: u32,
    pub sjw: u32,
    pub brp: u32,
}

// Supported features for GsDeviceBtConst.feature
pub const GS_CAN_FEATURE_LISTEN_ONLY: u32 = 1 << 1;
pub const GS_CAN_FEATURE_LOOP_BACK: u32 = 1 << 2;

// Vendor requests (BREQ) from host to device
pub const GS_USB_BREQ_HOST_FORMAT: u8 = 0;
pub const GS_USB_BREQ_BITTIMING: u8 = 1;
pub const GS_USB_BREQ_MODE: u8 = 2;
pub const GS_USB_BREQ_BERR: u8 = 3;
pub const GS_USB_BREQ_BT_CONST: u8 = 4;
pub const GS_USB_BREQ_DEVICE_CONFIG: u8 = 5;
pub const GS_USB_BREQ_TIMESTAMP: u8 = 6;
pub const GS_USB_BREQ_IDENTIFY: u8 = 7;
pub const GS_USB_BREQ_GET_USER_ID: u8 = 8;
pub const GS_USB_BREQ_SET_USER_ID: u8 = 9;
pub const GS_USB_BREQ_DATA_BITTIMING: u8 = 10;
pub const GS_USB_BREQ_DEV_CAPABILITIES: u8 = 11;
pub const GS_USB_BREQ_SET_TERMINATION: u8 = 12;
pub const GS_USB_BREQ_GET_TERMINATION: u8 = 13;
pub const GS_USB_BREQ_SET_FD_MODE: u8 = 14;