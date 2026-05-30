# USB TX/RX - Implementación Fase 2

## Resumen

Implementación completa del envío y recepción de tramas CAN vía USB Bulk endpoints, compatible con el driver `gs_usb` de Linux.

## Arquitectura

```
┌─────────────────────────────────────────────────────────────────────┐
│                          HOST (Linux)                              │
│                                                                     │
│  SocketCAN (candump/cansend)                                       │
│       │                                                             │
│       ▼                                                             │
│  gs_usb driver ─────────────────────────────────────────────────── │
└──────────────┬─────────────────────────────────────────────┬────────┘
               │ Bulk IN (EP1)                               │ Bulk OUT (EP1)
               │ 20 bytes/trama                              │ 20 bytes/trama
               ▼                                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      DEVICE (STM32F446)                            │
│                                                                     │
│  ┌─────────────┐   ┌──────────────┐   ┌─────────────┐            │
│  │ usb_tx_task │◄──│ CAN_RX_CHAN  │   │ usb_rx_task │            │
│  │             │   │    (32)      │   │             │            │
│  │             │◄──│ ECHO_CHAN    │   │             │            │
│  │             │   │    (16)      │   │             │            │
│  └──────┬──────┘   └──────────────┘   └──────┬──────┘            │
│         │                                     │                    │
│         │                                     ▼                    │
│         │                            ┌──────────────┐             │
│         │                            │ CAN_TX_CHAN  │             │
│         │                            │    (16)      │             │
│         │                            └──────┬──────┘             │
│         │                                   │                     │
│         │                                   ▼                     │
│         │                          ┌────────────────┐            │
│         │                          │  can_rx_task   │            │
│         │                          │  (select3)     │            │
│         │                          └───────┬────────┘            │
│         │                                  │                      │
│         │                                  ▼                      │
│         │                         ┌─────────────────┐            │
│         │                         │    bxCAN HW      │            │
│         │                         │  (CAN1 PB8/PB9) │            │
│         │                         └─────────────────┘            │
└─────────┴─────────────────────────────────────────────────────────┘
```

## Componentes Implementados

### 1. GsTxMsg (host → device)

**Archivo:** `crates/gs-usb-protocol/src/gs_usb_types.rs`

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default)]
pub struct GsTxMsg {
    pub echo_id: u32,      // ID para eco (el device lo retorna en GsHostFrame)
    pub can_id: u32,       // ID + flags (EFF, RTR, ERR)
    pub can_dlc: u8,       // DLC (0-8)
    pub channel: u8,       // Canal (0)
    pub flags: u8,         // Flags reservados
    pub reserved: u8,      // Reservado
    pub data: [u8; 8],     // Payload CAN
}
// Tamaño total: 20 bytes (coincide con gs_usb)
```

**Métodos:**
- `id() → u32`: Extrae el ID aplicando la máscara correcta (SFF o EFF)
- `is_extended() → bool`: Verifica si tiene flag EFF
- `is_rtr() → bool`: Verifica si tiene flag RTR
- `dlc() → u8`: Retorna DLC con límite de 8 bytes

### 2. GsHostFrame echo (device → host)

**Método:** `GsHostFrame::from_tx_msg_echo()`

Crea un frame de echo con:
- `echo_id`: El mismo ID que envió el host (para confirmación)
- `flags`: `GS_USB_FLAG_TX_ECHO` (bit 0)
- Datos originales del mensaje

### 3. BSP CAN transmit()

**Archivo:** `crates/bsp-f446/src/can.rs`

```rust
pub async fn transmit(
    &mut self,
    id: u32,
    is_extended: bool,
    is_rtr: bool,
    data: &[u8]
) -> Result<(), embassy_stm32::can::enums::FrameCreateError>
```

**Características:**
- Soporta IDs standard (11-bit) y extended (29-bit)
- Soporta tramas de datos y RTR
- Retorna error si el ID es inválido
- Usa `Frame::new_data()` o `Frame::new_remote()` de Embassy

### 4. usb_rx_task (parseo USB → CAN)

```rust
#[embassy_executor::task]
async fn usb_rx_task(mut ep_out: BspUsbEndpointOut) {
    let mut buf = [0u8; 64];
    loop {
        match ep_out.read(&mut buf).await {
            Ok(n) if n >= 20 => {
                let tx_msg: GsTxMsg = bytemuck::pod_read_unaligned(&buf[..20]);
                // Convertir a CanTxRequest y enviar a CAN_TX_CHANNEL
            }
            Ok(n) => warn!("Trama incompleta"),
            Err(e) => error!("Error USB RX"),
        }
    }
}
```

**Optimizaciones de performance:**
1. **Deserialización zero-copy:** `bytemuck::pod_read_unaligned()` convierte bytes directamente a struct
2. **Canal sin bloqueo:** `try_send()` evita bloqueo si la cola está llena
3. **Lectura directa:** Lee solo 20 bytes (tamaño de GsTxMsg) del buffer de 64

### 5. can_rx_task (con TX integrado)

```rust
async fn can_rx_task(mut can: BspCan) {
    loop {
        if is_started {
            match select3(
                CAN_CTRL_CHANNEL.receive(),  // Comandos USB
                can.can.read(),              // Tramas RX del bus
                CAN_TX_CHANNEL.receive()     // Solicitudes TX del host
            ).await {
                Either3::First(cmd) => { /* Control */ }
                Either3::Second(Ok(env)) => { /* RX → CAN_RX_CHANNEL */ }
                Either3::Third(tx_req) => {
                    // TX → can.transmit() → USB_ECHO_CHANNEL
                }
            }
        }
    }
}
```

### 6. usb_tx_task (multiplexor TX)

```rust
async fn usb_tx_task(mut ep_in: BspUsbEndpointIn) {
    loop {
        let host_frame = match select(
            CAN_RX_CHANNEL.receive(),    // Tramas del bus
            USB_ECHO_CHANNEL.receive()   // Echoes de TX
        ).await {
            Either::First(frame) => GsHostFrame::from_can_frame(...),
            Either::Second(echo) => echo,
        };
        ep_in.write(bytemuck::bytes_of(&host_frame)).await;
    }
}
```

## Flujo de Datos

### Recepción (Host → Device → Bus CAN)

```
1. Host envía GsTxMsg (20 bytes) por Bulk OUT
2. usb_rx_task lee del EP OUT
3. Parsea con bytemuck::pod_read_unaligned()
4. Convierte a CanTxRequest
5. Envía a CAN_TX_CHANNEL via try_send()
6. can_rx_task recibe de CAN_TX_CHANNEL
7. Llama a can.transmit()
8. can.write() al hardware bxCAN
9. Envía echo a USB_ECHO_CHANNEL
10. usb_tx_task envía echo al host
```

### Transmisión (Bus CAN → Host)

```
1. bxCAN recibe trama del bus
2. can.can.read() retorna Envelope
3. can_rx_task extrae id, data, dlc
4. Convierte a can_protocol::CanFrame
5. Decodifica (OBD2/UDS/RAW)
6. Envía a CAN_RX_CHANNEL
7. usb_tx_task recibe de CAN_RX_CHANNEL
8. Convierte a GsHostFrame
9. Escribe por Bulk IN (20 bytes)
10. Host recibe trama en SocketCAN
```

## Tamaños y Capacity

| Canal | Tipo | Capacidad | Uso |
|-------|------|-----------|-----|
| `CAN_RX_CHANNEL` | `CanFrame` | 32 | Tramas RX del bus → USB |
| `CAN_TX_CHANNEL` | `CanTxRequest` | 16 | Solicitudes TX del host → CAN |
| `USB_ECHO_CHANNEL` | `GsHostFrame` | 16 | Echoes de TX → USB |
| `CAN_CTRL_CHANNEL` | `CanCommand` | 4 | Comandos start/stop/timing |

**Consideraciones de RAM:**
- Cada `CanFrame` ocupa ~20 bytes
- Total para canales: ~1,5 KB (32×20 + 16×24 + 16×20 + 4×~12)
- Ajustable según necesidades de buffering

## Tests Unitarios

Todos los tests corren en host (no requieren MCU):

```bash
# Ejecutar todos los tests
cargo test -p gs-usb-protocol --target aarch64-apple-darwin

# Tests específicos de GsTxMsg
cargo test test_gs_tx_msg --target aarch64-apple-darwin

# Tests de GsHostFrame
cargo test test_from_tx_msg_echo --target aarch64-apple-darwin
```

**Cobertura de tests:**
- Tamaño de structs (20 bytes)
- Deserialización desde bytes
- Extracción de IDs (SFF/EFF)
- Detección de RTR
- Límites de DLC
- Serialización con bytemuck

## Errores Conocidos y Limitaciones

1. **DLC máximo 8:** CAN clásico, no CAN FD
2. **Un solo canal:** `channel` siempre es 0
3. **Sin timestamps:** El campo `flags` podría usarse para timestamps
4. **Sin filtros:** Se reciben todas las tramas del bus
5. **Echo inmediato:** Se envía antes de confirmar TX al硬件

## Próximos Pasos

1. Agregar timestamps en `GsHostFrame.flags`
2. Soporte CAN FD (DLC > 8, BRS, ESI)
3. Filtros de recepción configurables por el host
4. BufferedCan para mayor throughput
5. Manejo de errores con retry en USB TX/RX

## Referencias

- [gs_usb kernel driver](https://github.com/torvalds/linux/blob/master/drivers/net/can/usb/gs_usb.c)
- [SocketCAN documentation](https://www.kernel.org/doc/html/latest/networking/can.html)
- [Embassy STM32 CAN](https://docs.embassy.dev/embassy-stm32/latest/can/)
