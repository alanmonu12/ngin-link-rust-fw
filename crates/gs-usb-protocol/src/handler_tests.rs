// Los tests configuran el handler campo por campo; ignorar el lint
// de "field_reassign_with_default" mejora la legibilidad del setup.
#![allow(clippy::field_reassign_with_default)]

use core::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use crate::handler::GsUsbControlHandler;
use crate::gs_usb_types::*;
use embassy_usb::control::{InResponse, OutResponse, Recipient, Request, RequestType};
use embassy_usb::driver::Direction;
use embassy_usb::Handler;

// ---------------------------------------------------------------------------
// Helpers y fixtures
// ---------------------------------------------------------------------------

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

/// Timestamp controlado por un `AtomicU32` para que el callback `now_ms`
/// configurado por el test pueda inyectar valores deterministas.
static FAKE_NOW_MS: AtomicU32 = AtomicU32::new(0);

fn fake_now_ms() -> u32 {
    FAKE_NOW_MS.load(Ordering::SeqCst)
}

/// Estado capturado por el callback `on_identify` para verificar que se
/// invocó con el argumento correcto. Usamos un `Mutex<Vec<bool>>` porque los
/// tests en host corren en hilos separados cuando se usa `cargo test` con
/// paralelismo.
static IDENTIFY_CALLS: Mutex<Vec<bool>> = Mutex::new(Vec::new());

fn record_identify(on: bool) {
    IDENTIFY_CALLS.lock().unwrap().push(on);
}

fn clear_identify_calls() {
    IDENTIFY_CALLS.lock().unwrap().clear();
}

fn identify_calls() -> Vec<bool> {
    IDENTIFY_CALLS.lock().unwrap().clone()
}

/// Construye un handler con `now_ms` y `on_identify` instrumentados.
fn build_handler() -> GsUsbControlHandler {
    let mut h = GsUsbControlHandler::default();
    h.now_ms = fake_now_ms;
    h.on_identify = Some(record_identify);
    // Habilitamos la feature IDENTIFY para que el callback se invoque.
    h.capabilities = GsDeviceCapabilities::default()
        .with_feature(GS_CAN_FEATURE_IDENTIFY);
    h
}

// ---------------------------------------------------------------------------
// Tests existentes (control_in/out originales)
// ---------------------------------------------------------------------------

#[test]
fn test_ignoramos_peticiones_no_vendor() {
    let mut handler = GsUsbControlHandler::default();
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
    let mut handler = GsUsbControlHandler::default();
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
    let mut handler = GsUsbControlHandler::default();
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
fn test_bt_const_feature_incluye_listen_only_y_loop_back() {
    let mut handler = GsUsbControlHandler::default();
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_BT_CONST, 40);
    let mut buf = [0u8; 40];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        let consts: GsDeviceBtConst = bytemuck::pod_read_unaligned(data);
        assert_eq!(consts.feature & GS_CAN_FEATURE_LISTEN_ONLY, GS_CAN_FEATURE_LISTEN_ONLY);
        assert_eq!(consts.feature & GS_CAN_FEATURE_LOOP_BACK, GS_CAN_FEATURE_LOOP_BACK);
    } else {
        panic!("BT_CONST no fue aceptado");
    }
}

#[test]
fn test_out_mode_start_es_aceptado() {
    let mut handler = GsUsbControlHandler::default();
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_MODE, 4);
    let buf = [1u8, 0, 0, 0];

    assert!(matches!(handler.control_out(req, &buf), Some(OutResponse::Accepted)));
}

// ---------------------------------------------------------------------------
// Fase 3: GS_USB_BREQ_TIMESTAMP (control_in)
// ---------------------------------------------------------------------------

#[test]
fn test_timestamp_devuelve_valor_de_now_ms() {
    FAKE_NOW_MS.store(123_456, Ordering::SeqCst);
    let mut handler = build_handler();
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_TIMESTAMP, 4);
    let mut buf = [0u8; 4];

    let response = handler.control_in(req, &mut buf);
    assert!(matches!(response, Some(InResponse::Accepted(_))));

    if let Some(InResponse::Accepted(data)) = response {
        let ts = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(ts, 123_456, "Debe devolver el valor de fake_now_ms()");
    }
}

#[test]
fn test_timestamp_actualiza_con_now_ms_dinamico() {
    let mut handler = build_handler();
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_TIMESTAMP, 4);
    let mut buf = [0u8; 4];

    FAKE_NOW_MS.store(1, Ordering::SeqCst);
    handler.control_in(req, &mut buf).unwrap();
    assert_eq!(u32::from_le_bytes(buf), 1);

    FAKE_NOW_MS.store(u32::MAX, Ordering::SeqCst);
    handler.control_in(req, &mut buf).unwrap();
    assert_eq!(u32::from_le_bytes(buf), u32::MAX);
}

#[test]
fn test_timestamp_acepta_buffer_mayor_a_4() {
    // Algunos hosts piden más bytes de los necesarios; el handler
    // debe devolver solo los 4 bytes del u32.
    FAKE_NOW_MS.store(0xCAFE_BABE, Ordering::SeqCst);
    let mut handler = build_handler();
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_TIMESTAMP, 16);
    let mut buf = [0u8; 16];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        assert_eq!(data.len(), 4);
        assert_eq!(data, &[0xBE, 0xBA, 0xFE, 0xCA]); // little-endian
    } else {
        panic!("TIMESTAMP no fue aceptado");
    }
}

#[test]
fn test_timestamp_buffer_pequeno_copia_parcial() {
    // Si el host pide 2 bytes, el handler debe devolver lo que cabe
    // en vez de un STALL (compatibilidad con hosts mal portados).
    FAKE_NOW_MS.store(0x1234_5678, Ordering::SeqCst);
    let mut handler = build_handler();
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_TIMESTAMP, 2);
    let mut buf = [0u8; 2];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        assert_eq!(data.len(), 2);
        assert_eq!(data, &[0x78, 0x56]);
    } else {
        panic!("TIMESTAMP con buffer chico no fue aceptado");
    }
}

#[test]
fn test_timestamp_default_handler_devuelve_cero() {
    // Con el handler sin instrumentar, now_ms por defecto devuelve 0.
    let mut handler = GsUsbControlHandler::default();
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_TIMESTAMP, 4);
    let mut buf = [0u8; 4];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        assert_eq!(u32::from_le_bytes([data[0], data[1], data[2], data[3]]), 0);
    } else {
        panic!("TIMESTAMP default no fue aceptado");
    }
}

// ---------------------------------------------------------------------------
// Fase 3: GS_USB_BREQ_IDENTIFY (control_out)
// ---------------------------------------------------------------------------

#[test]
fn test_identify_on_invoca_callback_true() {
    clear_identify_calls();
    let mut handler = build_handler();
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_IDENTIFY, 4);
    let buf = [1u8, 0, 0, 0];

    assert!(matches!(handler.control_out(req, &buf), Some(OutResponse::Accepted)));
    assert_eq!(identify_calls(), vec![true]);
}

#[test]
fn test_identify_off_invoca_callback_false() {
    clear_identify_calls();
    let mut handler = build_handler();
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_IDENTIFY, 4);
    let buf = [0u8, 0, 0, 0];

    assert!(matches!(handler.control_out(req, &buf), Some(OutResponse::Accepted)));
    assert_eq!(identify_calls(), vec![false]);
}

#[test]
fn test_identify_acepta_request_aceptado_aunque_valor_sea_invalido() {
    // Valores fuera de 0/1 son aceptados silenciosamente; el firmware
    // decide qué hacer. Verificamos que el callback recibe el estado
    // derivado de is_on().
    clear_identify_calls();
    let mut handler = build_handler();
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_IDENTIFY, 4);
    let buf = [0xFFu8, 0xFF, 0xFF, 0xFF];

    assert!(matches!(handler.control_out(req, &buf), Some(OutResponse::Accepted)));
    // 0xFFFFFFFF no es ni 0 ni 1, is_on() devuelve false.
    assert_eq!(identify_calls(), vec![false]);
}

#[test]
fn test_identify_sin_feature_identify_no_invoca_callback() {
    // Sin la feature IDENTIFY activa, el handler responde OK pero NO
    // notifica al callback (política de seguridad: no enciende un LED
    // que no existe).
    clear_identify_calls();
    let mut handler = GsUsbControlHandler::default();
    handler.on_identify = Some(record_identify);
    // capabilities queda vacía -> no soporta IDENTIFY
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_IDENTIFY, 4);
    let buf = [1u8, 0, 0, 0];

    assert!(matches!(handler.control_out(req, &buf), Some(OutResponse::Accepted)));
    assert!(identify_calls().is_empty(), "No debe invocar callback sin feature IDENTIFY");
}

#[test]
fn test_identify_sin_callback_no_paniquea() {
    // Es válido no configurar on_identify; el handler debe responder OK.
    let mut handler = GsUsbControlHandler::default();
    handler.capabilities = GsDeviceCapabilities::default()
        .with_feature(GS_CAN_FEATURE_IDENTIFY);
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_IDENTIFY, 4);
    let buf = [1u8, 0, 0, 0];

    assert!(matches!(handler.control_out(req, &buf), Some(OutResponse::Accepted)));
}

#[test]
fn test_identify_buffer_insuficiente_no_invoca_callback() {
    clear_identify_calls();
    let mut handler = build_handler();
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_IDENTIFY, 4);
    let buf = [1u8, 0]; // solo 2 bytes

    assert!(matches!(handler.control_out(req, &buf), Some(OutResponse::Accepted)));
    assert!(identify_calls().is_empty(), "No debe invocar con buffer < 4 bytes");
}

// ---------------------------------------------------------------------------
// Fase 3: GS_USB_BREQ_GET_USER_ID / SET_USER_ID
// ---------------------------------------------------------------------------

#[test]
fn test_get_user_id_devuelve_cero_por_defecto() {
    let mut handler = GsUsbControlHandler::default();
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_GET_USER_ID, 4);
    let mut buf = [0u8; 4];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        assert_eq!(u32::from_le_bytes([data[0], data[1], data[2], data[3]]), 0);
    } else {
        panic!("GET_USER_ID no fue aceptado");
    }
}

#[test]
fn test_set_user_id_actualiza_y_get_lo_refleja() {
    let mut handler = GsUsbControlHandler::default();
    let out_req = create_vendor_request(Direction::Out, GS_USB_BREQ_SET_USER_ID, 4);
    let out_buf = 0xDEAD_BEEFu32.to_le_bytes();
    assert!(matches!(handler.control_out(out_req, &out_buf), Some(OutResponse::Accepted)));
    assert_eq!(handler.user_id, 0xDEAD_BEEF);

    let in_req = create_vendor_request(Direction::In, GS_USB_BREQ_GET_USER_ID, 4);
    let mut in_buf = [0u8; 4];
    if let Some(InResponse::Accepted(data)) = handler.control_in(in_req, &mut in_buf) {
        assert_eq!(u32::from_le_bytes([data[0], data[1], data[2], data[3]]), 0xDEAD_BEEF);
    } else {
        panic!("GET_USER_ID no fue aceptado después de SET");
    }
}

#[test]
fn test_set_user_id_persiste_entre_multiples_sets() {
    let mut handler = GsUsbControlHandler::default();
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_SET_USER_ID, 4);

    handler.control_out(req, &1u32.to_le_bytes()).unwrap();
    assert_eq!(handler.user_id, 1);

    handler.control_out(req, &0xFFFF_FFFFu32.to_le_bytes()).unwrap();
    assert_eq!(handler.user_id, 0xFFFF_FFFF);
}

#[test]
fn test_set_user_id_con_buffer_insuficiente_no_modifica() {
    let mut handler = GsUsbControlHandler::default();
    handler.user_id = 0x1234_5678;
    let req = create_vendor_request(Direction::Out, GS_USB_BREQ_SET_USER_ID, 4);
    let buf = [0xFFu8, 0xFF]; // 2 bytes es insuficiente

    assert!(matches!(handler.control_out(req, &buf), Some(OutResponse::Accepted)));
    assert_eq!(handler.user_id, 0x1234_5678, "No debe modificar con buffer corto");
}

#[test]
fn test_get_user_id_acepta_buffer_mayor() {
    let mut handler = GsUsbControlHandler::default();
    handler.user_id = 0xCAFE_BABE;
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_GET_USER_ID, 16);
    let mut buf = [0u8; 16];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        assert_eq!(data.len(), 4);
        assert_eq!(u32::from_le_bytes([data[0], data[1], data[2], data[3]]), 0xCAFE_BABE);
    } else {
        panic!("GET_USER_ID con buffer grande no fue aceptado");
    }
}

#[test]
fn test_get_user_id_buffer_pequeno_copia_parcial() {
    let mut handler = GsUsbControlHandler::default();
    handler.user_id = 0x1234_5678;
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_GET_USER_ID, 2);
    let mut buf = [0u8; 2];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        assert_eq!(data.len(), 2);
        assert_eq!(data, &[0x78, 0x56]);
    } else {
        panic!("GET_USER_ID con buffer chico no fue aceptado");
    }
}

// ---------------------------------------------------------------------------
// Fase 3: GS_USB_BREQ_DEV_CAPABILITIES (control_in)
// ---------------------------------------------------------------------------

#[test]
fn test_dev_capabilities_devuelve_cero_por_defecto() {
    let mut handler = GsUsbControlHandler::default();
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_DEV_CAPABILITIES, 4);
    let mut buf = [0u8; 4];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        let caps: GsDeviceCapabilities = bytemuck::pod_read_unaligned(data);
        assert_eq!(caps.feature, 0);
    } else {
        panic!("DEV_CAPABILITIES no fue aceptado");
    }
}

#[test]
fn test_dev_capabilities_refleja_capabilities_configuradas() {
    let mut handler = GsUsbControlHandler::default();
    handler.capabilities = GsDeviceCapabilities::default()
        .with_feature(GS_CAN_FEATURE_LOOP_BACK)
        .with_feature(GS_CAN_FEATURE_FD)
        .with_feature(GS_CAN_FEATURE_USER_ID);
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_DEV_CAPABILITIES, 4);
    let mut buf = [0u8; 4];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        let caps: GsDeviceCapabilities = bytemuck::pod_read_unaligned(data);
        assert!(caps.supports(GS_CAN_FEATURE_LOOP_BACK));
        assert!(caps.supports(GS_CAN_FEATURE_FD));
        assert!(caps.supports(GS_CAN_FEATURE_USER_ID));
        assert!(!caps.supports(GS_CAN_FEATURE_LISTEN_ONLY));
        assert!(!caps.supports(GS_CAN_FEATURE_IDENTIFY));
    } else {
        panic!("DEV_CAPABILITIES no fue aceptado");
    }
}

#[test]
fn test_dev_capabilities_serializacion_little_endian() {
    let mut handler = GsUsbControlHandler::default();
    handler.capabilities = GsDeviceCapabilities {
        feature: 0xCAFE_BABE,
    };
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_DEV_CAPABILITIES, 4);
    let mut buf = [0u8; 4];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        assert_eq!(data, &[0xBE, 0xBA, 0xFE, 0xCA]);
    } else {
        panic!("DEV_CAPABILITIES no fue aceptado");
    }
}

#[test]
fn test_dev_capabilities_buffer_pequeno_copia_parcial() {
    let mut handler = GsUsbControlHandler::default();
    handler.capabilities = GsDeviceCapabilities {
        feature: 0x1234_5678,
    };
    let req = create_vendor_request(Direction::In, GS_USB_BREQ_DEV_CAPABILITIES, 2);
    let mut buf = [0u8; 2];

    if let Some(InResponse::Accepted(data)) = handler.control_in(req, &mut buf) {
        assert_eq!(data.len(), 2);
        assert_eq!(data, &[0x78, 0x56]);
    } else {
        panic!("DEV_CAPABILITIES con buffer chico no fue aceptado");
    }
}

// ---------------------------------------------------------------------------
// Tests de regresión: comandos no soportados deben seguir devolviendo STALL
// ---------------------------------------------------------------------------

#[test]
fn test_in_comando_desconocido_devuelve_stall() {
    let mut handler = GsUsbControlHandler::default();
    let req = create_vendor_request(Direction::In, 0xFF, 4);
    let mut buf = [0u8; 4];
    assert!(handler.control_in(req, &mut buf).is_none());
}

#[test]
fn test_out_comando_desconocido_devuelve_stall() {
    let mut handler = GsUsbControlHandler::default();
    let req = create_vendor_request(Direction::Out, 0xFF, 4);
    let buf = [0u8; 4];
    assert!(handler.control_out(req, &buf).is_none());
}