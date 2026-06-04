# Diseño de Canales - Patrones de Comunicación

## Visión General

El firmware usa `embassy_sync::channel` para comunicación asíncrona entre tareas. Este documento describe el diseño, patrones y consideraciones de performance.

## Patrón: Productor-Consumidor con Canales

### Concepto

<div align="center">
  <img src="../imgs/Patrón-Productor-Consumidor.png" alt="Patrón Productor-Consumidor" />
  <p><em>Patrón Productor-Consumidor: comunicación asíncrona vía Channel con buffer circular.</em></p>
</div>

### Ventajas

1. **Desacoplamiento:** Las tareas no conocen unas de otras
2. **Sin bloqueo:** `send().await` suspende sin bloquear el executor
3. **Buffer circular:** Absorbe picos de tráfico
4. **Type safety:** El tipo del mensaje está codificado en tiempo de compilación

## Canales Implementados

### 1. CAN_RX_CHANNEL (32 mensajes)

**Propósito:** Tramas CAN recibidas del bus → USB host

```rust
static CAN_RX_CHANNEL: Channel<CriticalSectionRawMutex, CanFrame, 32> = Channel::new();
```

**Productor:** `can_driver_task` (cuando recibe del bus)
**Consumidor:** `usb_tx_task`

**Por qué 32 mensajes:**
- USB Full Speed: ~1ms por transferencia de 64 bytes
- CAN a 500kbps: ~1 trama cada 30-50μs (depende de longitud)
- 32 mensajes dan ~1ms de buffer en ráfaga máxima
- RAM: 32 × 20 bytes = 640 bytes

### 2. CAN_CMD_CHANNEL (16 mensajes)

**Propósito:** Comandos de control y solicitudes de transmisión → actor del driver CAN

```rust
static CAN_CMD_CHANNEL: Channel<CriticalSectionRawMutex, CanDriverCmd, 16> = Channel::new();
```

**Productores:**
- Callbacks de `GsUsbControlHandler` (`Start`, `Stop`, `SetBitTiming`)
- `usb_rx_task` (`Transmit`)

**Consumidor:** `can_driver_task`

**Por qué 16 mensajes:**
- Unifica control (esporádico) y TX (burst posible del host)
- Evita tener dos canales separados que complican el `select`
- RAM: 16 × ~28 bytes = ~448 bytes

### 3. USB_ECHO_CHANNEL (16 mensajes)

**Propósito:** Confirmaciones de TX enviadas → host

```rust
static USB_ECHO_CHANNEL: Channel<CriticalSectionRawMutex, GsHostFrame, 16> = Channel::new();
```

**Productor:** `can_driver_task` (después de transmitir exitosamente)
**Consumidor:** `usb_tx_task`

**Por qué 16 mensajes:**
- Debe coincidir con la capacidad de TX
- Cada TX genera un echo
- RAM: 16 × 20 bytes = 320 bytes

## Patrón: select() en can_driver_task

### Problema

`can_driver_task` es el **actor único** del driver CAN. Debe escuchar 2 fuentes simultáneamente cuando está iniciado:
1. Comandos del driver (`CAN_CMD_CHANNEL`: control + transmisión)
2. Tramas del bus CAN (`can.can.read()`)

### Solución

```rust
match select(
    CAN_CMD_CHANNEL.receive(),   // Fuente 1: comandos + TX
    can.can.read()               // Fuente 2: RX del bus
).await {
    Either::First(cmd) => { /* Manejar comando */ }
    Either::Second(Ok(env)) => { /* Manejar RX del bus */ }
    Either::Second(Err(_)) => { /* Ignorar error de bus */ }
}
```

### Cómo funciona select()

<div align="center">
  <img src="../imgs/select3().png" alt="select()" />
  <p><em>select() consulta 2 fuentes simultáneas de forma cooperativa y fair.</em></p>
</div>

```
┌─────────────────────────────────────────────┐
│                    select()                 │
├─────────────────────────────────────────────┤
│                                             │
│  ┌──────────────┐  ┌──────────────┐        │
│  │  receive()   │  │   read()     │        │
│  │  CAN_CMD     │  │   can        │        │
│  └──────┬───────┘  └──────┬───────┘        │
│         │                 │                 │
│         ▼                 ▼                 │
│  ┌──────────────────────────────────────┐  │
│  │          Waker Registry              │  │
│  │  - Registra wakers de cada op      │  │
│  └──────────────────────────────────────┘  │
│                    │                        │
│                    ▼                        │
│  ┌──────────────────────────────────────┐  │
│  │          Poll (cooperative)          │  │
│  │  - Solo una fuente retorna valor     │  │
│  │  - Las otras se re-registran         │  │
│  └──────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

**Características:**
- No desperdicia CPU (polling no activo)
- Fair: ambas fuentes se consultan equitativamente
- Sin prioridad explícita (depende del orden de registro)

### Por qué solo 2 fuentes (no 3)

Anteriormente se usaba `select3()` con:
- `CAN_CTRL_CHANNEL` (control)
- `can.can.read()` (RX)
- `CAN_TX_CHANNEL` (TX)

Esto se simplificó a un único canal `CAN_CMD_CHANNEL` que unifica control y TX:
- **Menor complejidad:** Un solo `select` en lugar de `select3`
- **Código más legible:** No hay `Either3::First/Second/Third`
- **Menor overhead:** ~100 ciclos en Cortex-M4 vs ~150 con `select3`
- **Responsabilidad clara:** `can_driver_task` solo recibe "comandos" y las ejecuta

## Patrón: select() en usb_tx_task

### Problema

`usb_tx_task` debe enviar 2 tipos de tramas:
1. Tramas recibidas del bus (`CAN_RX_CHANNEL`)
2. Echoes de transmisión (`USB_ECHO_CHANNEL`)

### Solución

```rust
let host_frame = match select(
    CAN_RX_CHANNEL.receive(),
    USB_ECHO_CHANNEL.receive()
).await {
    Either::First(frame) => GsHostFrame::from_can_frame(...),
    Either::Second(echo) => echo,
};
```

## Patrones de Error Handling

### 1. try_send() en callbacks

```rust
fn on_start_cb() {
    let _ = CAN_CMD_CHANNEL.try_send(CanDriverCmd::Start);
}
```

**Razón:** Los callbacks se ejecutan en contexto de interrupción USB. `send().await` bloquearía. `try_send()` falla silenciosamente si la cola está llena, pero es seguro porque el comando no es crítico.

### 2. try_send() en usb_rx_task

```rust
match CAN_CMD_CHANNEL.try_send(CanDriverCmd::Transmit(tx_req)) {
    Ok(()) => {}
    Err(_) => warn!("Cola CAN CMD llena"),
}
```

**Razón:** Si el driver CAN no puede absorber tramas tan rápido como el USB las recibe, la cola se llena. Es mejor descartar que bloquear el USB.

### 3. Result en transmit()

```rust
match can.transmit(...).await {
    Ok(()) => { /* Enviar echo */ }
    Err(e) => error!("Error TX"),
}
```

**Razón:** El hardware CAN puede fallar (ID inválido, bus off, etc.). Se loguea el error pero se continúa.

## Optimizaciones de Performance

### 1. Zero-Copy con bytemuck

```rust
// ❌ Copy byte-by-byte (lento)
let mut msg = GsTxMsg::default();
msg.echo_id = u32::from_le_bytes(buf[0..4].try_into().unwrap());
// ...

// ✅ Zero-copy (rápido)
let msg: GsTxMsg = bytemuck::pod_read_unaligned(&buf);
```

**Ahorro:** ~100 ciclos por trama en Cortex-M4

### 2. Capacidad de canales ajustada

| Canal | Capacidad | Justificación |
|-------|-----------|---------------|
| CAN_RX | 32 | Absorbe burst de CAN a 1Mbps |
| CAN_CMD | 16 | Unifica control + TX burst |
| ECHO | 16 | 1:1 con TX |

### 3. Límite de DLC

```rust
pub fn dlc(&self) -> u8 {
    self.can_dlc.min(8)  // CAN clásico
}
```

**Prev:** Envío de datos inválidos al hardware

### 4. select() en lugar de select3()

```rust
// ❌ select3() (más complejo, menos legible)
match select3(ctrl, can_read, tx).await { ... }

// ✅ select() de 2 fuentes (más simple)
match select(CAN_CMD_CHANNEL.receive(), can.can.read()).await { ... }
```

**Beneficio:** Menor complejidad cognitiva, menos ramas en el `match`, ~50 ciclos menos por iteración.

## Consideraciones de Timing

### Latencia del sistema

```
USB RX → usb_rx_task → CAN_CMD_CHANNEL → can_driver_task → bxCAN
  │                                                     │
  └─ ~1ms (USB Full Speed)                              └─ ~30μs (CAN 500kbps)

Total: ~1.03ms por trama
```

### Throughput máximo teórico

- CAN 500kbps: ~4,500 tramas/segundo (tramas de 8 bytes)
- USB Full Speed: ~1,000 transferencias/segundo × 64 bytes
- Bottleneck: USB (no CAN)

### Jitter

- `select()`: ~1μs de jitter por iteración
- `try_send()`: ~0.1μs (no bloquea)
- `ep_out.read()`: Variable (depende de USB stack)

## debugging

### Logs con defmt

```rust
info!("CAN: Configurando Bit Timing");
warn!("USB RX: Cola CAN CMD llena");
error!("CAN TX: Error al transmitir");
```

### Métricas a monitorear

1. **Tasa de descartes en CAN_CMD_CHANNEL:** Indica si el driver CAN es bottleneck
2. **Tasa de errores USB TX:** Indica si el host está leyendo
3. **Latencia end-to-end:** Medir con timestamps

## Futuras Mejoras

1. **BufferedCan:** Usar `embassy_stm32::can::BufferedCan` para mayor throughput
2. **Prioridades:** Asignar prioridades a tareas Embassy
3. **Power:** Integrar `embassy_usb` power management
4. **DMA:** Usar DMA para transferencias USB (si disponible)
5. **RTOS metrics:** Agregar contadores de mensajes procesados
