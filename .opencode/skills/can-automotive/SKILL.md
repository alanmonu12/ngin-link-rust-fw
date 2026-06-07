---
name: can-automotive
description: Use when working with CAN bus protocol, gs_usb, OBD2, UDS, SocketCAN, or automotive ECU communication. Covers frame structures, IDs, bit timing, protocol decoding, and USB-CAN bridge patterns. Use when editing can-protocol, gs-usb-protocol, or any file related to CAN/automotive comms.
---

# CAN & Automotive Protocol — Ngin-Link Firmware

## Arquitectura del Protocolo

Este firmware implementa un puente USB↔CAN usando el protocolo `gs_usb` (compatible con SocketCAN en Linux). El flujo de datos es:

```
Bus CAN físico → bxCAN HW → Embassy CAN Driver → can_rx_task → Channel → usb_tx_task → USB Bulk IN → Host
Host → USB Bulk OUT → usb_rx_task → Channel → can_rx_task (TX) → bxCAN HW → Bus CAN físico
```

## Especificaciones CAN Clásico (CAN 2.0A/B)

### IDs y Máscaras
- **Standard Frame (SFF):** ID de 11 bits, máscara `0x7FF` (`GS_CAN_ID_MASK_SFF`).
- **Extended Frame (EFF):** ID de 29 bits, máscara `0x1FFFFFFF` (`GS_CAN_ID_MASK_EFF`).
- **Bit EFF (bit 31):** `GS_CAN_ID_FLAG_EFF = 0x80000000` — indica ID extendido.
- **Bit RTR (bit 30):** `GS_CAN_ID_FLAG_RTR = 0x40000000` — Remote Transmission Request.
- **Bit ERR (bit 29):** `GS_CAN_ID_FLAG_ERR = 0x20000000` — Error frame.

### DLC (Data Length Code)
- CAN clásico: DLC de 0 a 8.
- Un DLC > 8 se limita a 8 (método `dlc()` de `GsTxMsg`).
- CAN FD soporta DLC hasta 15 (64 bytes) — **no implementado aún**.

### Bit Timing
El bit timing se configura via USB control transfer (`GS_USB_BREQ_BITTIMING`):
- Estructura: `GsDeviceBitTiming { prop_seg, phase_seg1, phase_seg2, sjw, brp }`.
- Los límites válidos se reportan en `GsDeviceBtConst`.
- **Regla crítica:** El CAN debe estar en STOP antes de cambiar bit timing.

## Protocolo gs_usb

### USB Vendor Requests (BREQ)

| BREQ | Nombre | Dirección | Payload | Wire format |
|------|--------|-----------|---------|-------------|
| 0 | `HOST_FORMAT` | OUT | — | ACK |
| 1 | `BITTIMING` | OUT | `GsDeviceBitTiming` | 20 B |
| 2 | `MODE` | OUT | `u32` (bit 0 = start) | 4 B |
| 3 | `BERR` | OUT | — | — |
| 4 | `BT_CONST` | IN | `GsDeviceBtConst` | 40 B |
| 5 | `DEVICE_CONFIG` | IN | `GsDeviceConfig` | 12 B |
| 6 | `TIMESTAMP` | IN | `u32` (mseg desde boot) | 4 B |
| 7 | `IDENTIFY` | OUT | `GsIdentifyMode` (u32) | 4 B |
| 8 | `GET_USER_ID` | IN | `u32` | 4 B |
| 9 | `SET_USER_ID` | OUT | `u32` | 4 B |
| 11 | `DEV_CAPABILITIES` | IN | `GsDeviceCapabilities` | 4 B |

BREQ 10/12/13/14 están reservados por el kernel driver pero no se exponen en el handler actual.

### Estructuras Wire Format (20 bytes cada una)
- `GsHostFrame`: Device → Host (RX y TX echo). Campos: `echo_id`, `can_id`, `can_dlc`, `channel`, `flags`, `data[8]`.
- `GsTxMsg`: Host → Device (TX request). Mismo layout, `echo_id` > 0 indica que se espera echo.
- **Ambos structs son `#[repr(C)]` de exactamente 20 bytes** — verificado con tests de `size_of`.

### Feature Flags (`GsDeviceBtConst::feature` / `GsDeviceCapabilities::feature`)
Las posiciones de bit **deben coincidir exactamente** con `include/uapi/linux/can/gs_usb.h` del kernel Linux:

| Bit | Constante | Significado |
|-----|-----------|------------|
| 0 | `GS_CAN_FEATURE_LISTEN_ONLY` | Modo solo escucha (no ACK) |
| 1 | `GS_CAN_FEATURE_LOOP_BACK` | Loopback interno |
| 2 | `GS_CAN_FEATURE_TRIPLE_SAMPLE` | Triple sampling |
| 3 | `GS_CAN_FEATURE_ONE_SHOT` | Single-shot TX |
| 4 | `GS_CAN_FEATURE_HW_TIMESTAMP` | Timestamps en HW |
| 5 | `GS_CAN_FEATURE_IDENTIFY` | Soporta comando IDENTIFY |
| 6 | `GS_CAN_FEATURE_USER_ID` | Soporta GET/SET_USER_ID |
| 7 | `GS_CAN_FEATURE_PAD_PKTS_TO_MAX_PKT_SIZE` | Padding a 64 B |
| 8 | `GS_CAN_FEATURE_FD` | CAN FD |

### Flujo de Control USB
1. Host envía `GS_USB_BREQ_MODE(START)` → callback `on_start_cb` → `can.start()`.
2. Host envía `GS_USB_BREQ_BITTIMING` → callback `on_bit_timing_cb` → `can.set_bit_timing()`.
3. Host envía `GS_USB_BREQ_MODE(STOP)` → callback `on_stop_cb` → `can.stop()`.
4. Host envía `GS_USB_BREQ_TIMESTAMP` → handler lee `now_ms()` (inyectado) → 4 B LE.
5. Host envía `GS_USB_BREQ_IDENTIFY` → si la feature IDENTIFY está activa en `capabilities`, invoca `on_identify(true|false)`.
6. Host envía `GsTxMsg` por Bulk OUT → `usb_rx_task` → `can.transmit()`.
7. Device envía `GsHostFrame` por Bulk IN para RX y TX echoes.

### Callbacks del Handler
```rust
GsUsbControlHandler {
    on_start: Option<fn(u32)>,        // flags de modo (LISTEN_ONLY, LOOP_BACK, etc.)
    on_stop: Option<fn()>,
    on_bit_timing: Option<fn(GsDeviceBitTiming)>,
    on_identify: Option<fn(bool)>,  // true = LED ON
    now_ms: fn() -> u32,            // fuente de tiempo para TIMESTAMP
    user_id: u32,                   // estado para GET/SET_USER_ID
    capabilities: GsDeviceCapabilities, // bitfield de features reportadas
}
```
Los callbacks envían comandos por `CAN_CMD_CHANNEL` — **nunca llaman directamente al hardware CAN**. El callback `on_identify` y la fuente `now_ms` son `fn` puros (no `FnMut`/`FnOnce`) porque el handler no tiene `&mut` a estado async: el firmware debe delegar a un `Channel` Embassy si necesita notificar a una tarea.

## ⚠️ Lecciones Críticas de Interoperabilidad gs_usb

### echo_id en frames RX
El driver gs_usb del kernel usa `echo_id == 0xFFFFFFFF` (`GS_HOST_FRAME_ECHO_ID_RX`) para distinguir frames recibidos del bus de ecos de transmisión. **NUNCA** usar `echo_id = 0` para frames RX — el kernel lo descarta como "Unexpected unused echo id 0".

### interface_count en GsDeviceConfig
El kernel hace `icount = dconf.icount + 1`. Para **1 interfaz CAN**, usar `icount = 0`. Usar `icount = 1` crea incorrectamente 2 interfaces (can0 y can1).

### sw_version y GS_CAN_FEATURE_IDENTIFY
El kernel solo habilita `GS_CAN_FEATURE_IDENTIFY` si `sw_version > 1`. Usar `sw_version = 1` desactiva IDENTIFY aunque el feature esté en capabilities.

### bt_const_feature debe reflejar capabilities
`GsDeviceBtConst.feature` debe ser idéntico a `GsDeviceCapabilities.feature`. El kernel consulta BT_CONST para determinar capacidades como listen-only.

## Protocolos Automotrices (Decodificación)

### OBD2 (On-Board Diagnostics)
- **Request IDs:** `0x7DF` (broadcast), `0x7E0`–`0x7E7` (ECU específico).
- **Response IDs:** `0x7E8`–`0x7EF`.
- Service IDs: `0x01` (Mode 1 - live data), `0x09` (vehicle info), `0x22` (Mode 22 - enhanced).
- Estructura: `[length, SID, PID, data..., padding]`.
- DLC siempre 8 con padding de ceros.

### UDS (Unified Diagnostic Services, ISO 14229)
- IDs típicos: `0x7E0` (physical request), `0x7E8` (physical response).
- Service IDs comunes: `0x10` (DiagnosticSessionControl), `0x22` (ReadDataByIdentifier), `0x27` (SecurityAccess), `0x2E` (WriteDataByIdentifier), `0x31` (RoutineControl), `0x34` (RequestDownload), `0x36` (TransferData).
- importante para chiptuning: `0x34/0x36/0x37` (download/upload) son los servicios usados por herramientas como Kess/Flex.

### Notas de Implementación en `can-protocol`
- `CanFrame { id, is_extended, data: [u8; 8], dlc: u8 }` es la representación genérica.
- `analyze_frame()` retorna `DecodedProtocol::Obd2Request(...)`, `UdsMessage(...)`, o `Raw`.
- **Extender** con nuevos protocolos: agregar variantes a `DecodedProtocol` y funciones en módulos nuevos (ej: `isotp.rs` para ISO-TP).

## SocketCAN Compatibility

El firmware es compatible con SocketCAN en Linux gracias al protocolo gs_usb. El driver `gs_usb` en el kernel de Linux crea interfaces `canX` que se comportan como interfaces CAN nativas.

### Comandos Útiles de Verificación
```bash
# Configurar interfaz
sudo ip link set can0 up type can bitrate 500000

# Enviar trama
cansend can0 123#DEADBEEF

# Capturar tráfico
candump can0

# Con DLT (para logging)
dlt-receive localhost 3490
```

## Pines del Hardware (STM32F446RE)
- **CAN:** PB8=RX, PB9=TX (bxCAN1, CANFD-capable pero solo CAN clásico por ahora).
- **USB:** PA11=D−, PA12=D+ (USB OTG FS, 48MHz clock).
- **LED:** Pendiente de implementar.

## Reglas al Modificar Protocolo

1. **Nunca** cambiar el tamaño de `GsHostFrame` o `GsTxMsg` de 20 bytes — rompe compatibilidad con el driver gs_usb del kernel Linux.
2. Siempre mantener `#[repr(C)]` y derivar `Pod` (bytemuck) en structs de protocolo.
3. Agregar nuevos `GS_USB_BREQ_*` solo si el driver Linux los soporta, o si documentas que es una extensión propietaria.
4. Los IDs de CAN estándar (11-bit) nunca deben exceder `0x7FF`. Para IDs extendidos, siempre setear el bit EFF.
5. Al agregar decodificación de protocolo en `can-protocol`, mantener la función `analyze_frame()` como punto de entrada único. Los métodos helper (ej: `parse_obd2_request()`) se implementan en sus propios módulos.