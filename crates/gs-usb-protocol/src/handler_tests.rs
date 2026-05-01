use crate::handler::GsUsbControlHandler;
use crate::gs_usb_types::*;
use embassy_usb::control::{Recipient, Request, RequestType, InResponse, OutResponse};
use embassy_usb::driver::Direction;
use embassy_usb::Handler;

// Función auxiliar para crear Requests falsos en los tests
fn create_vendor_request(direction: Direction, request: u8, length: u16) -> Request {
    Request {
        direction,
        request_type: RequestType::Vendor,
        recipient: Recipient::Device,
        request,
        value: 0,
        index: 0,
        length,
    }
}

#[test]
fn test_ignoramos_peticiones_no_vendor() {
    let mut handler = GsUsbControlHandler;
    let req = Request {
        direction: Direction::In,
        request_type: RequestType::Standard, // No es Vendor
        recipient: Recipient::Device,
        request: GS_USB_BREQ_DEVICE_CONFIG,
        value: 0,
        index: 0,
        length: 12,
    };
    let mut buf = [0u8; 12];

    // Debería devolver None (STALL)
    assert!(handler.control_in(req, &mut buf).is_none());
}

#[test]
fn test_device_config_devuelve_valores_correctos() {
    let mut handler = GsUsbControlHandler;
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_DEVICE_CONFIG, 12);
    let mut buf = [0u8; 12];

    let response = handler.control_in(req, &mut buf);

    // Comprobamos que el Request fue aceptado
    assert!(matches!(response, Some(InResponse::Accepted(_))));
    
    if let Some(InResponse::Accepted(data)) = response {
        let config: GsDeviceConfig = bytemuck::pod_read_unaligned(data);
        assert_eq!(config.interface_count, 1, "Debería reportar 1 interfaz CAN");
        assert_eq!(config.sw_version, 1, "La versión de software debería ser 1");
        assert_eq!(config.hw_version, 1, "La versión de hardware debería ser 1");
    }
}

#[test]
fn test_bt_const_devuelve_limites_correctos() {
    let mut handler = GsUsbControlHandler;
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_BT_CONST, 40);
    let mut buf = [0u8; 40];

    let response = handler.control_in(req, &mut buf);

    assert!(matches!(response, Some(InResponse::Accepted(_))));
    
    if let Some(InResponse::Accepted(data)) = response {
        let consts: GsDeviceBtConst = bytemuck::pod_read_unaligned(data);
        assert_eq!(consts.fclk_can, 42_000_000, "El reloj debería ser 42 MHz");
    }
}

#[test]
fn test_out_mode_start_es_aceptado() {
    let mut handler = GsUsbControlHandler;
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_MODE, 4);
    let buf = [1u8, 0, 0, 0];

    assert!(matches!(handler.control_out(req, &buf), Some(OutResponse::Accepted)));
}