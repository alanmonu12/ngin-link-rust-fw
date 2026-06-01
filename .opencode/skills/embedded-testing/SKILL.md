---
name: embedded-testing
description: Use when writing or modifying tests for embedded Rust code. Covers host-based unit tests, #[cfg_attr] patterns, bytemuck serialization tests, protocol validation, and test strategies for no_std crates. Use when editing test files, adding #[test] functions, or running cargo test.
---

# Embedded Testing — Ngin-Link Firmware

## Estrategia de Testing

### Dos Niveles de Tests

| Nivel | Dónde | Comando | Notas |
|-------|-------|---------|-------|
| **Host Unit Tests** | `crates/gs-usb-protocol/`, `crates/can-protocol/` | `cargo test-mac` o `cargo test-linux` | Corren en x86_64/aarch64, sin hardware |
| **_integration tests_** | En el MCU con probe-rs | `DEFMT_LOG=trace cargo run --release` | Pendiente de implementar |

### Principio Fundamental
**Probar la lógica, no el hardware.** El BSP (`bsp-f446`) es el único crate que toca hardware y no se testea unitariamente — se valida en integración. Los crates `gs-usb-protocol` y `can-protocol` son 100% lógica pura y deben tener tests exhaustivos.

## Patrones de Testing

### 1. Crate Testeable en Host

El patrón actual para crates `#![no_std]` que necesitan tests en host:

```rust
// lib.rs
#![cfg_attr(target_os = "none", no_std)]

pub mod handler;
pub mod gs_usb_types;

#[cfg(all(test, not(target_os = "none")))]
mod handler_tests;
```

**Regla:** `#![no_std]` se aplica **solo** cuando target es `none` (MCU). En host se usa `std` para poder correr `#[test]`.

### 2. Tests de Structs C-repr (Wire Format)

Estos tests son **críticos** porque garantizan compatibilidad binaria con el driver gs_usb del kernel Linux:

```rust
#[test]
fn test_gs_host_frame_tamaño_es_20_bytes() {
    assert_eq!(core::mem::size_of::<GsHostFrame>(), 20);
}

#[test]
fn test_serializacion_bytemuck() {
    let frame = GsHostFrame::from_can_frame(0x7DF, false, 8, &data);
    let bytes = bytemuck::bytes_of(&frame);
    assert_eq!(bytes.len(), 20);
    assert_eq!(bytes[4..8], [0xDF, 0x07, 0x00, 0x00]); // can_id little-endian
}
```

**Siempre verificar:**
- Tamaño exacto del struct (20 bytes para frames gs_usb).
- Endianness (little-endian en ARM/USB).
- Offsets de campos específicos.
- Round-trip: serializar → deserializar → comparar.

### 3. Tests de Protocolo

```rust
#[test]
fn test_obd2_request_mode_01() {
    let frame = CanFrame { id: 0x7DF, is_extended: false, data: [0x02, 0x01, 0x0C, 0, 0, 0, 0, 0], dlc: 8 };
    let result = analyze_frame(&frame);
    assert!(matches!(result, DecodedProtocol::Obd2Request(_)));
}
```

**Regla:** Para cada service ID de OBD2/UDS que se soporte, agregar un test con una trama realista.

### 4. Tests de Deserialización desde Bytes Raw

```rust
#[test]
fn test_gs_tx_msg_deserializacion_bytemuck() {
    let bytes: [u8; 20] = [
        0x2A, 0x00, 0x00, 0x00,  // echo_id = 42
        0xDF, 0x07, 0x00, 0x00,  // can_id = 0x7DF
        0x08,                      // can_dlc = 8
        0x00,                      // channel = 0
        0x00,                      // flags = 0
        0x00,                      // reserved = 0
        0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE,
    ];
    let msg: GsTxMsg = bytemuck::pod_read_unaligned(&bytes);
    assert_eq!(msg.echo_id, 42);
    assert_eq!(msg.can_id, 0x7DF);
}
```

### 5. Tests de Límites y Edge Cases

```rust
#[test]
fn test_dlc_maximo_8() {
    let msg = GsTxMsg { can_dlc: 15, ..Default::default() };
    assert_eq!(msg.dlc(), 8); // Se limita a 8 en CAN clásico
}

#[test]
fn test_mascara_sff() {
    let frame = GsHostFrame::from_can_frame(0xFFFF_FFFF, false, 8, &data);
    assert_eq!(frame.can_id, GS_CAN_ID_MASK_SFF); // 0x7FF
}
```

## Comandos de Testing

```bash
# Alias del proyecto (definidos en .cargo/config.toml)
cargo test-mac      # macOS: cargo test --target aarch64-apple-darwin
cargo test-linux     # Linux: cargo test --target x86_64-unknown-linux-gnu

# Crate específico
cargo test-mac -p gs-usb-protocol
cargo test-mac -p can-protocol

# Test por nombre
cargo test-mac test_gs_tx_msg

# Con output verbose (ver prints de defmt)
cargo test-mac -- --nocapture

# Tests del firmware (compilación para ARM, no ejecutan tests)
cargo build --release
```

## Convenciones

1. **Nombre de tests en español:** `test_tamaño_es_20_bytes`, `test_deserializacion_bytemuck`.
2. **Patrón Arrange-Act-Assert:** Siempre estructurar tests en tres secciones claras.
3. **Tests deterministas:** No usar randomness ni dependencias de timing.
4. **Un assertion por concepto:** Si un test verifica múltiples cosas, separar en asserts claros.
5. **Tests de regresión:** Al fixear un bug, escribir un test que hubiera fallado antes del fix.

## Qué NO Testear

- **Hardware directo:** No testear BSP con mocks de registros (los drivers Embassy ya están testeados).
- **Timing exacto:** Los timing dependen del clock del MCU y no son reproducibles en host.
- **Concurrencia real:** Los canales Embassy usan `CriticalSectionRawMutex` que no funciona en host — los tests de lógica de canales deben ser secuenciales.

## Checklist al Agregar Código Nuevo

- [ ] Si es lógica pura (protocolo, parsing, transformación de datos): agregar `#[test]` en el mismo archivo o en `*_tests.rs`.
- [ ] Si es un struct `#[repr(C)]`: testear `size_of` y serialización/deserialización con bytemuck.
- [ ] Si es un handler de USB: testear los casos de `control_in` y `control_out` con callbacks mock.
- [ ] Si modifica `analyze_frame()`: agregar tests para cada nuevo protocolo o variante.
- [ ] Correr `cargo test-mac` antes de commit.

## TypeScript de Tests Pendientes (Prioridad)

1. **USB TX/RX mock tests:** Mock de endpoints para verificar framing (alta).
2. **CAN transmit tests:** Mock de `BspCan` para verificar llamadas (alta).
3. **Channel capacity tests:** Test de `try_send` cuando la cola está llena (media).
4. **Error handling tests:** Test de logs de error y recuperación (media).
5. **Performance benchmark:** Throughput y latencia end-to-end (baja).