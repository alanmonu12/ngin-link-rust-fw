---
name: embedded-rust
description: Use when writing or modifying Rust code for embedded/bare-metal targets (#![no_std], Embassy, STM32, cortex-m). Covers async patterns, memory constraints, hardware abstractions, and idiomatic embedded Rust. Use ONLY when editing .rs files that target thumbv7em-none-eabihf or use #![no_std].
---

# Embedded Rust — Ngin-Link Firmware

## Reglas Obligatorias

### 1. `#![no_std]` Compatibility
- Todos los crates del workspace deben ser `#![no_std]` compatibles excepto cuando corren tests en host.
- Usar `#![cfg_attr(target_os = "none", no_std)]` en crates que necesitan ser testeables en host (patrón actual en `gs-usb-protocol`).
- **Prohibido** usar `alloc`, `String`, `Vec`, `Box`, `format!`, `println!` en código de producción.
- Reemplazar colecciones dinámicas con arrays fijos, `heapless::Vec`, `heapless::String`, o `Channel` de Embassy.

### 2. Embassy Async Patterns
- **Nunca** usar `std::thread` o bloqueos activos (`loop { ... }` sin `.await`).
- Usar `embassy_sync::channel::Channel` para comunicación entre tareas (ya declarados como `static CAN_RX_CHANNEL`, etc.).
- Usar `embassy_futures::select::{select, select3}` para multiplexar múltiples fuentes asíncronas.
- Las tareas Embassy se declaran con `#[embassy_executor::task]` y deben ser `async fn`.
- La tarea principal usa `#[embassy_executor::main]`.

### 3. Memoria Estática
- Usar `static_cell::StaticCell` para buffers que necesita Embassy USB Builder (patrón actual en `main.rs`).
- Los canales (`Channel`) son `static` con `CriticalSectionRawMutex`.
- **Prohibido** asignaciones dinámicas en runtime (no `Box::new`, no `Vec::new`).
- Tamaño máximo de stack: 2KB por tarea por defecto. Preferir tipos fijos.

### 4. Tipos de Hardware — BSP F446
- **SIEMPRE** usar el BSP (`bsp_f446::init()`) para inicializar hardware. Nunca acceder directamente a los registros del STM32 desde `main.rs`.
- El BSP encapsula: PLL (84MHz sys, 48MHz USB), configuración de pines, drivers CAN y USB.
- Tipos exportados del BSP: `BspCan`, `BspUsbDriver`, `BspUsbEndpointIn`, `BspUsbEndpointOut`.
- Los drivers CAN retornan `Result` con errores propios — siempre manejar con `match`, nunca `unwrap()`.

### 5. Manejo de Errores
- **Prohibido** `unwrap()` y `expect()` en paths de producción.
- Usar `match` exhaustivo o `if let` con manejo de errores explícito.
- Propagar errores con `?` solo cuando el contexto lo permite (no en `#[embassy_executor::main]`).
- Loggear errores con `defmt::error!` o `defmt::warn!`.
- Usar `try_send()` para canales y manejar `Err` (cola llena = descartar o loggear).

### 6. Logging con defmt
- Usar **exclusivamente** `defmt` para logs: `defmt::info!`, `defmt::warn!`, `defmt::error!`, `defmt::debug!`, `defmt::trace!`.
- **Prohibido** `println!`, `log::info!`, o cualquier crate de logging que no sea `defmt`.
- Para tipos que no implementan `defmt::Format`, usar `defmt::Debug2Format(&tipo)`.
- Niveles de log se controlan con `DEFMT_LOG=trace` en tiempo de ejecución.

### 7. Recursos Compartidos y Concurrencia
- `CriticalSectionRawMutex` para canales estáticos (es el patrón correcto para Cortex-M single-core).
- No usar `Mutex` de `std` o `parking_lot`.
- No usar `RefCell` + `PX_STACK` — preferir canales de Embassy para comunicación entre tareas.
- El patrón productor-consumidor con `Channel` es la forma canónica de pasar datos entre tareas.

### 8. Patrones Idiomáticos del Proyecto

#### Tarea de Recepción CAN (Productor)
```rust
#[embassy_executor::task]
async fn can_rx_task(mut can: BspCan) {
    loop {
        match can.can.read().await {
            Ok(envelope) => {
                // Procesar trama y enviar por canal
                CAN_RX_CHANNEL.send(frame).await;
            }
            Err(e) => { /* loggear, nunca panic */ }
        }
    }
}
```

#### Tarea de Transmisión USB (Consumidor)
```rust
#[embassy_executor::task]
async fn usb_tx_task(mut ep_in: BspUsbEndpointIn) {
    loop {
        let frame = CAN_RX_CHANNEL.receive().await;
        let bytes = bytemuck::bytes_of(&frame);
        match ep_in.write(bytes).await {
            Ok(()) => {}
            Err(e) => { /* loggear */ }
        }
    }
}
```

#### Serialización con bytemuck
- Todos los structs de protocolo usan `#[repr(C)]` y derivan `Pod` de bytemuck.
- Serializar: `bytemuck::bytes_of(&struct)` → `&[u8]`.
- Deserializar: `bytemuck::pod_read_unaligned(&slice)` → `Struct`.
- Siempre verificar `slice.len() >= core::mem::size_of::<Struct>()` antes de deserializar.

### 9. Convenciones de Nombres
- Variables y funciones en `snake_case` en español cuando sea descriptivo (ej: `on_start_cb`, `can_rx_task`).
- Tipos y structs en `PascalCase` (ej: `GsHostFrame`, `CanCommand`).
- Constantes y estáticas en `SCREAMING_SNAKE_CASE` (ej: `CAN_RX_CHANNEL`, `GS_CAN_ID_FLAG_EFF`).
- Comentarios en español dentro del código.
- Commits: `tipo(alcance): descripción` (ej: `feat(usb): agregar bulk endpoint RX task`).

### 10. Cross-Compilation
- Target de producción: `thumbv7em-none-eabihf`.
- Tests en host: `x86_64-unknown-linux-gnu` (Linux) o `aarch64-apple-darwin` (macOS).
- Usar `#[cfg(all(test, not(target_os = "none")))]` para tests que corren en host.
- Los tests del crate `gs-usb-protocol` y `can-protocol` corren en host, no en el MCU.