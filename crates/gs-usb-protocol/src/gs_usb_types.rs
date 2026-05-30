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

pub const GS_CAN_ID_FLAG_EFF: u32 = 1 << 31;
pub const GS_CAN_ID_FLAG_RTR: u32 = 1 << 30;
pub const GS_CAN_ID_FLAG_ERR: u32 = 1 << 29;

pub const GS_CAN_ID_MASK_SFF: u32 = 0x0000_07FF;
pub const GS_CAN_ID_MASK_EFF: u32 = 0x1FFF_FFFF;

pub const GS_CAN_FLAG_OVERFLOW: u8 = 1 << 0;
pub const GS_CAN_FLAG_FD: u8 = 1 << 1;
pub const GS_CAN_FLAG_BRS: u8 = 1 << 2;
pub const GS_CAN_FLAG_ESI: u8 = 1 << 3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default)]
pub struct GsHostFrame {
    pub echo_id: u32,
    pub can_id: u32,
    pub can_dlc: u8,
    pub channel: u8,
    pub flags: u8,
    pub reserved: u8,
    pub data: [u8; 8],
}

impl GsHostFrame {
    pub fn from_can_frame(id: u32, is_extended: bool, dlc: u8, data: &[u8; 8]) -> Self {
        let can_id = if is_extended {
            (id & GS_CAN_ID_MASK_EFF) | GS_CAN_ID_FLAG_EFF
        } else {
            id & GS_CAN_ID_MASK_SFF
        };

        Self {
            echo_id: 0,
            can_id,
            can_dlc: dlc,
            channel: 0,
            flags: 0,
            reserved: 0,
            data: *data,
        }
    }

    pub fn from_tx_msg_echo(echo_id: u32, id: u32, is_extended: bool, dlc: u8, data: &[u8; 8]) -> Self {
        let can_id = if is_extended {
            (id & GS_CAN_ID_MASK_EFF) | GS_CAN_ID_FLAG_EFF
        } else {
            id & GS_CAN_ID_MASK_SFF
        };

        Self {
            echo_id,
            can_id,
            can_dlc: dlc,
            channel: 0,
            flags: GS_USB_FLAG_TX_ECHO,
            reserved: 0,
            data: *data,
        }
    }
}

pub const GS_USB_FLAG_TX_ECHO: u8 = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default)]
pub struct GsTxMsg {
    pub echo_id: u32,
    pub can_id: u32,
    pub can_dlc: u8,
    pub channel: u8,
    pub flags: u8,
    pub reserved: u8,
    pub data: [u8; 8],
}

impl GsTxMsg {
    pub fn id(&self) -> u32 {
        if self.is_extended() {
            self.can_id & GS_CAN_ID_MASK_EFF
        } else {
            self.can_id & GS_CAN_ID_MASK_SFF
        }
    }

    pub fn is_extended(&self) -> bool {
        (self.can_id & GS_CAN_ID_FLAG_EFF) != 0
    }

    pub fn is_rtr(&self) -> bool {
        (self.can_id & GS_CAN_ID_FLAG_RTR) != 0
    }

    pub fn dlc(&self) -> u8 {
        self.can_dlc.min(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gs_host_frame_tamaño_es_20_bytes() {
        assert_eq!(core::mem::size_of::<GsHostFrame>(), 20);
    }

    #[test]
    fn test_from_can_frame_standard() {
        let data = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let frame = GsHostFrame::from_can_frame(0x123, false, 8, &data);

        assert_eq!(frame.echo_id, 0);
        assert_eq!(frame.can_id, 0x123);
        assert_eq!(frame.can_dlc, 8);
        assert_eq!(frame.channel, 0);
        assert_eq!(frame.flags, 0);
        assert_eq!(frame.data, data);
    }

    #[test]
    fn test_from_can_frame_extended() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00];
        let frame = GsHostFrame::from_can_frame(0x1ABCDEF, true, 4, &data);

        assert_eq!(frame.echo_id, 0);
        assert_eq!(frame.can_id, 0x8000_0000 | 0x1ABCDEF);
        assert_eq!(frame.can_dlc, 4);
        assert_eq!(frame.channel, 0);
        assert_eq!(frame.flags, 0);
    }

    #[test]
    fn test_from_can_frame_mascara_sff() {
        let data = [0; 8];
        let frame = GsHostFrame::from_can_frame(0xFFFF_FFFF, false, 8, &data);
        assert_eq!(frame.can_id, GS_CAN_ID_MASK_SFF);
    }

    #[test]
    fn test_from_can_frame_mascara_eff() {
        let data = [0; 8];
        let frame = GsHostFrame::from_can_frame(0xFFFF_FFFF, true, 8, &data);
        assert_eq!(frame.can_id, GS_CAN_ID_FLAG_EFF | GS_CAN_ID_MASK_EFF);
    }

    #[test]
    fn test_serializacion_bytemuck() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let frame = GsHostFrame::from_can_frame(0x7DF, false, 8, &data);
        let bytes = bytemuck::bytes_of(&frame);

        assert_eq!(bytes.len(), 20);
        assert_eq!(bytes[0..4], [0x00, 0x00, 0x00, 0x00]);
        assert_eq!(bytes[4..8], [0xDF, 0x07, 0x00, 0x00]);
        assert_eq!(bytes[8], 8);
        assert_eq!(bytes[9], 0);
        assert_eq!(bytes[10], 0);
        assert_eq!(bytes[11], 0);
        assert_eq!(bytes[12..20], data);
    }

    #[test]
    fn test_from_tx_msg_echo() {
        let data = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22];
        let frame = GsHostFrame::from_tx_msg_echo(42, 0x123, false, 8, &data);

        assert_eq!(frame.echo_id, 42);
        assert_eq!(frame.can_id, 0x123);
        assert_eq!(frame.can_dlc, 8);
        assert_eq!(frame.flags, GS_USB_FLAG_TX_ECHO);
        assert_eq!(frame.data, data);
    }

    #[test]
    fn test_from_tx_msg_echo_extended() {
        let data = [0; 8];
        let frame = GsHostFrame::from_tx_msg_echo(99, 0x1ABCDEF, true, 4, &data);

        assert_eq!(frame.echo_id, 99);
        assert_eq!(frame.can_id, GS_CAN_ID_FLAG_EFF | 0x1ABCDEF);
        assert_eq!(frame.can_dlc, 4);
        assert_eq!(frame.flags, GS_USB_FLAG_TX_ECHO);
    }

    #[test]
    fn test_gs_tx_msg_tamaño_es_20_bytes() {
        assert_eq!(core::mem::size_of::<GsTxMsg>(), 20);
    }

    #[test]
    fn test_gs_tx_msg_deserializacion_bytemuck() {
        let bytes: [u8; 20] = [
            0x2A, 0x00, 0x00, 0x00,
            0xDF, 0x07, 0x00, 0x00,
            0x08, 0x00, 0x00, 0x00,
            0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE,
        ];
        let msg: GsTxMsg = bytemuck::pod_read_unaligned(&bytes);

        assert_eq!(msg.echo_id, 42);
        assert_eq!(msg.can_id, 0x7DF);
        assert_eq!(msg.can_dlc, 8);
        assert_eq!(msg.channel, 0);
        assert_eq!(msg.flags, 0);
        assert_eq!(msg.data, [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE]);
    }

    #[test]
    fn test_gs_tx_msg_id_standard() {
        let msg = GsTxMsg {
            echo_id: 1,
            can_id: 0x123,
            can_dlc: 8,
            channel: 0,
            flags: 0,
            reserved: 0,
            data: [0; 8],
        };

        assert_eq!(msg.id(), 0x123);
        assert!(!msg.is_extended());
    }

    #[test]
    fn test_gs_tx_msg_id_extended() {
        let msg = GsTxMsg {
            echo_id: 2,
            can_id: GS_CAN_ID_FLAG_EFF | 0x1ABCDEF,
            can_dlc: 4,
            channel: 0,
            flags: 0,
            reserved: 0,
            data: [0; 8],
        };

        assert_eq!(msg.id(), 0x1ABCDEF);
        assert!(msg.is_extended());
    }

    #[test]
    fn test_gs_tx_msg_rtr() {
        let msg = GsTxMsg {
            echo_id: 3,
            can_id: GS_CAN_ID_FLAG_RTR | 0x456,
            can_dlc: 0,
            channel: 0,
            flags: 0,
            reserved: 0,
            data: [0; 8],
        };

        assert!(msg.is_rtr());
        assert_eq!(msg.id(), 0x456);
    }

    #[test]
    fn test_gs_tx_msg_dlc_maximo_8() {
        let msg = GsTxMsg {
            echo_id: 4,
            can_id: 0x100,
            can_dlc: 15,
            channel: 0,
            flags: 0,
            reserved: 0,
            data: [0; 8],
        };

        assert_eq!(msg.dlc(), 8);
    }

    #[test]
    fn test_gs_tx_msg_dlc_normal() {
        let msg = GsTxMsg {
            echo_id: 5,
            can_id: 0x200,
            can_dlc: 4,
            channel: 0,
            flags: 0,
            reserved: 0,
            data: [0; 8],
        };

        assert_eq!(msg.dlc(), 4);
    }

    #[test]
    fn test_gs_tx_msg_mascara_sff() {
        let msg = GsTxMsg {
            echo_id: 6,
            can_id: 0x0000_FFFF,
            can_dlc: 8,
            channel: 0,
            flags: 0,
            reserved: 0,
            data: [0; 8],
        };

        assert_eq!(msg.id(), GS_CAN_ID_MASK_SFF);
        assert!(!msg.is_extended());
    }

    #[test]
    fn test_gs_tx_msg_mascara_eff() {
        let msg = GsTxMsg {
            echo_id: 7,
            can_id: GS_CAN_ID_FLAG_EFF | 0xFFFF_FFFF,
            can_dlc: 8,
            channel: 0,
            flags: 0,
            reserved: 0,
            data: [0; 8],
        };

        assert_eq!(msg.id(), GS_CAN_ID_MASK_EFF);
        assert!(msg.is_extended());
    }
}