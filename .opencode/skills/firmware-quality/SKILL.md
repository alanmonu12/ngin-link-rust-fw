---
name: firmware-quality
description: Use when reviewing, refactoring, or writing production firmware code. Covers safety-critical patterns, error handling, resource management, no-panic guarantees, and defensive programming for automotive/embedded systems. Use ONLY when the task involves code quality, safety, robustness, or architectural decisions in firmware .rs files.
---

# Firmware Quality — Ngin-Link Firmware

## Principios de Robustez para Firmware Automotriz

Este firmware se conecta a buses CAN de vehículos. Un bug puede causar:
- Corrupción de datos ECU (escritura accidental).
- Pérdida de comunicación crítica.
- Comportamiento impredecible en el bus CAN.

**Las siguientes reglas son obligatorias en código de producción.**

## 1. Prohibido Panic en Producción

| Patrón | Prohibido | Alternativa |
|--------|-----------|-------------|
| Acceso a array | `arr[idx]` | `arr.get(idx).copied().unwrap_or_default()` |
| Opción presente | `opt.unwrap()` | `match opt { Some(v) => ..., None => { warn!("..."); } }` |
| Resultado esperado | `res.expect("msg")` | `match res { Ok(v) => ..., Err(e) => { error!("..."); } }` |
| Indexación de slice | `&slice[a..b]` | Verificar bounds antes: `if b <= slice.len()` |

**Excepción:** `unwrap()` es aceptable **solo** en `#[test]` o en código con `#[cfg(test)]`.

## 2. Manejo de Errores

### Patrón Correcto: Match Exhaustivo
```rust
match can.can.read().await {
    Ok(envelope) => { /* procesar */ }
    Err(e) => {
        error!("CAN RX: {}", defmt::Debug2Format(&e));
        // No panic, no unwrap — continuar el loop
    }
}
```

### Patrón Correcto: Canales con try_send
```rust
match CAN_TX_CHANNEL.try_send(tx_req) {
    Ok(()) => {}
    Err(_) => {
        warn!("CAN TX: Cola llena, descartando trama");
        // Políticas posibles: descartar (actual), esperar (cambiar a send), contar estadísticas
    }
}
```

### Antipatrón: Ignorar Silenciosamente
```rust
// MAL — tramas perdidas sin log
let _ = CAN_TX_CHANNEL.try_send(tx_req);

// BIEN — loggear descarte
if CAN_TX_CHANNEL.try_send(tx_req).is_err() {
    warn!("Cola llena, trama descartada");
}
```

## 3. Gestión de Recursos

### Memoria
- Capacidad de canales fija en `static`: 32 (CAN_RX), 16 (CAN_TX), 16 (USB_ECHO).
- **Nunca** crecer buffers dinámicamente. Si el throughput excede la capacidad, las tramas se descartan con warning.
- Buffer USB: 64 bytes (max packet size para Full Speed). **No modificar sin entender el USB spec.**

### Stack
- Cada tarea Embassy tiene su propio stack. Código embebido = stack limitado.
- Evitar funciones recursivas o con mucho stack (ej: grandes arrays locales).
- Preferir `static` para buffers grandes (patrón `StaticCell`).

### Concurrencia
- Solo **un productor** y **un consumidor** por canal (patrón SPSC con `Channel`).
- `select`/`select3` para multiplexar sin busy-waiting.
- No compartir `&mut` entre tareas sin protección.

## 4. Defensa del Bus CAN

### Validación de Entrada
```rust
// SIEMPRE validar tramas recibidas del host antes de transmitirlas
fn validate_tx_msg(msg: &GsTxMsg) -> bool {
    let dlc = msg.dlc(); // Ya limita a 8
    if dlc > 8 { return false; } // Redundante pero explícito
    if msg.channel > 0 { return false; } // Solo canal 0 soportado
    true
}
```

### Rate Limiting
- No enviar tramas CAN más rápido de lo que el bus puede absorber.
- El hardware bxCAN tiene un TX mailbox de 3 posiciones, pero esto puede llenarse.
- Considerar contar tramas descartadas para estadísticas.

### Protección contra Tramas Malformadas
- Siempre verificar `slice.len() >= size_of::<Struct>()` antes de deserializar.
- Verificar DLC dentro de rango (0-8 para CAN clásico).
- Verificar que IDs estándar no excedan 0x7FF sin bit EFF.
- Ignorar tramas con flags desconocidos.

## 5. Lifecycle de Estados del CAN

El CAN tiene **dos estados**: STOPPED y STARTED. Las reglas son:

```
STOPPED → STARTED: comando GS_USB_BREQ_MODE(1) = Start
STARTED → STOPPED: comando GS_USB_BREQ_MODE(0) = Stop
STOPPED → STOPPED: SetBitTiming (solo se puede configurar en STOPPED)
```

**Regla:** Bit timing **solo** se puede cambiar cuando el CAN está en STOP. Si se recibe SetBitTiming en STARTED, se debe loggear y rechazar.

**Implementar como máquina de estados explícita:**
```rust
enum CanState {
    Stopped,
    Started,
}
```

## 6. USB — Consideraciones Específicas

### Control Transfers
- `GsUsbControlHandler` procesa vendor requests (bRequest).
- Solo aceptar `bmRequestType == 0xC0` (vendor, device-to-host) o `0x40` (vendor, host-to-device).
- Ignorar y responder `None` para tipos no-vendor.

### Bulk Transfers
- Bulk IN (Device→Host): enviar `GsHostFrame` de 20 bytes por cada trama CAN recibida + echoes de TX.
- Bulk OUT (Host→Device): recibir `GsTxMsg` de 20 bytes, deserializar con `pod_read_unaligned`.
- **Validar tamaño mínimo** antes de deserializar: `n >= size_of::<GsTxMsg>()`.
- El endpoint bulk puede ser disabled/re-enabled por el host — siempre llamar `wait_enabled().await` antes del loop.

### Desconexión USB
- El host puede desconectar en cualquier momento. Embassy USB maneja esto con `wait_enabled()`.
- No hay mecanismo de recuperación explícito — el loop principal debe ser resiliente.

## 7. Patrones de Logging

```rust
// Niveles y cuándo usarlos
defmt::trace!("CAN RX: ID=0x{:03X}, DLC={}", frame.id, frame.dlc); // Detalle excesivo
defmt::debug!("CAN: Trama OBD2 en ID 0x{:03X}", frame.id);         // Debug发育
defmt::info!("CAN: Controlador iniciado");                           // Eventos normales
defmt::warn!("CAN TX: Cola llena, descartando trama");               // Problema recuperable
defmt::error!("USB: Error en endpoint: {}", e);                      // Problema grave
```

**Regla:** En producción (release), `DEFMT_LOG=warn`. En development, `DEFMT_LOG=trace`.

## 8. Checklist de Code Review

- [ ] Sin `unwrap()` o `expect()` en paths de producción.
- [ ] Todos los `match` son exhaustivos (sin `_` catch-all silencioso).
- [ ] Errores de `try_send()` son manejados (no ignorados).
- [ ] Validación de tamaño antes de deserializar bytes del USB.
- [ ] DLC limitado a 8 (CAN clásico).
- [ ] CAN ID mask aplicada correctamente (SFF vs EFF).
- [ ] Bit timing solo configurable en STOP.
- [ ] Logs en español con contexto suficiente.
- [ ] Comentarios explican "por qué", no "qué".
- [ ] No hay `alloc` implícito (no `String`, `Vec`, `format!`).
- [ ] `#[repr(C)]` y `Pod` en structs de protocolo.

## 9. Arquitectura — Qué Va Dónde

| Crate | Responsabilidad | Depende de | Tiene tests host |
|-------|----------------|------------|------------------|
| `firmware` | Orquestación de tareas, main loop | `bsp-f446`, `gs-usb-protocol`, `can-protocol` | No |
| `bsp-f446` | Hardware específico (pins, clocks, drivers) | `embassy-stm32` | No |
| `gs-usb-protocol` | Tipos y lógica del protocolo gs_usb | `embassy-usb`, `bytemuck` | Sí |
| `can-protocol` | Decodificación CAN (OBD2, UDS) | Ninguna dependency hw | Sí |

**Regla:** Si es lógica de protocolo o transformación de datos, va en `gs-usb-protocol` o `can-protocol`. Si toca registros o periféricos, va en `bsp-f446`. Si orquesta, va en `firmware/src/main.rs`.