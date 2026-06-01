# Referencia de Tipos gs_usb

## Visión General

Documentación completa de todos los tipos definidos en `gs_usb_types.rs` y su uso en el protocolo gs_usb.

## Constantes

### Flags de ID CAN

```rust
pub const GS_CAN_ID_FLAG_EFF: u32 = 1 << 31;  // Extended Frame Format (29-bit)
pub const GS_CAN_ID_FLAG_RTR: u32 = 1 << 30;  // Remote Transmission Request
pub const GS_CAN_ID_FLAG_ERR: u32 = 1 << 29;  // Error Frame
```

### Máscaras de ID

```rust
pub const GS_CAN_ID_MASK_SFF: u32 = 0x0000_07FF;  // Standard (11-bit): 0x000 - 0x7FF
pub const GS_CAN_ID_MASK_EFF: u32 = 0x1FFF_FFFF;  // Extended (29-bit): 0x0000000 - 0x1FFFFFFF
```

### Flags de Frame

```rust
pub const GS_CAN_FLAG_OVERFLOW: u8 = 1 << 0;  // Buffer overflow
pub const GS_CAN_FLAG_FD: u8 = 1 << 1;        // CAN FD frame
pub const GS_CAN_FLAG_BRS: u8 = 1 << 2;       // Bit Rate Switch (FD)
pub const GS_CAN_FLAG_ESI: u8 = 1 << 3;       // Error State Indicator (FD)
pub const GS_USB_FLAG_TX_ECHO: u8 = 1 << 0;   // Echo de transmisión
```

### Vendor Requests (BREQ)

| Constante | Valor | Dirección | Uso |
|-----------|-------|-----------|-----|
| `GS_USB_BREQ_HOST_FORMAT` | 0 | Host→Dev | Formato de datos del host |
| `GS_USB_BREQ_BITTIMING` | 1 | Host→Dev | Configurar bit timing |
| `GS_USB_BREQ_MODE` | 2 | Host→Dev | Iniciar/Detener CAN |
| `GS_USB_BREQ_BERR` | 3 | Host→Dev | Habilitar errores |
| `GS_USB_BREQ_BT_CONST` | 4 | Dev→Host | Constantes de timing |
| `GS_USB_BREQ_DEVICE_CONFIG` | 5 | Dev→Host | Configuración del device |
| `GS_USB_BREQ_TIMESTAMP` | 6 | Dev→Host | Timestamp del device |
| `GS_USB_BREQ_IDENTIFY` | 7 | Host→Dev | Activar LED identificación |
| `GS_USB_BREQ_GET_USER_ID` | 8 | Dev→Host | Obtener ID de usuario |
| `GS_USB_BREQ_SET_USER_ID` | 9 | Host→Dev | Establecer ID de usuario |
| `GS_USB_BREQ_DATA_BITTIMING` | 10 | Host→Dev | Bit timing CAN FD |
| `GS_USB_BREQ_DEV_CAPABILITIES` | 11 | Dev→Host | Capacidades del device |
| `GS_USB_BREQ_SET_TERMINATION` | 12 | Host→Dev | Terminal CAN |
| `GS_USB_BREQ_GET_TERMINATION` | 13 | Dev→Host | Obtener terminal |
| `GS_USB_BREQ_SET_FD_MODE` | 14 | Host→Dev | Habilitar CAN FD |

### Features

```rust
pub const GS_CAN_FEATURE_LISTEN_ONLY: u32 = 1 << 0;  // Solo escucha (no ACK)
pub const GS_CAN_FEATURE_LOOP_BACK: u32 = 1 << 1;    // Modo loopback interno
pub const GS_CAN_FEATURE_TRIPLE_SAMPLE: u32 = 1 << 2;
pub const GS_CAN_FEATURE_ONE_SHOT: u32 = 1 << 3;
pub const GS_CAN_FEATURE_HW_TIMESTAMP: u32 = 1 << 4;
pub const GS_CAN_FEATURE_IDENTIFY: u32 = 1 << 5;
pub const GS_CAN_FEATURE_USER_ID: u32 = 1 << 6;
pub const GS_CAN_FEATURE_PAD_PKTS_TO_MAX_PKT_SIZE: u32 = 1 << 7;
pub const GS_CAN_FEATURE_FD: u32 = 1 << 8;
```

Las posiciones de bit coinciden con `include/uapi/linux/can/gs_usb.h` del kernel Linux — un test (`test_feature_flags_coinciden_con_kernel_linux`) fija estos valores para evitar drift.

## Structs

### GsDeviceConfig

**Dirección:** Dev → Host (control_in)
**BREQ:** `GS_USB_BREQ_DEVICE_CONFIG` (5)

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default)]
pub struct GsDeviceConfig {
    pub reserved1: u8,
    pub reserved2: u8,
    pub reserved3: u8,
    pub interface_count: u8,  // Número de interfaces CAN
    pub sw_version: u32,      // Versión del software
    pub hw_version: u32,      // Versión del hardware
}
```

**Tamaño:** 12 bytes

**Uso:**
- El driver Linux solicita esta información al inicio
- Permite al driver saber cuántas interfaces CAN tiene el device

### GsDeviceBtConst

**Dirección:** Dev → Host (control_in)
**BREQ:** `GS_USB_BREQ_BT_CONST` (4)

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GsDeviceBtConst {
    pub feature: u32,      // Features soportados (GS_CAN_FEATURE_*)
    pub fclk_can: u32,     // Frecuencia del reloj CAN en Hz (48MHz para STM32F446)
    pub tseg1_min: u32,    // Time segment 1 mínimo
    pub tseg1_max: u32,    // Time segment 1 máximo
    pub tseg2_min: u32,    // Time segment 2 mínimo
    pub tseg2_max: u32,    // Time segment 2 máximo
    pub sjw_max: u32,      // Sync Jump Width máximo
    pub brp_min: u32,      // Baud Rate Prescaler mínimo
    pub brp_max: u32,      // Baud Rate Prescaler máximo
    pub brp_inc: u32,      // Incremento del prescaler
}
```

**Tamaño:** 40 bytes

**Uso:**
- El driver Linux usa esta información para calcular el bit timing automáticamente
- Permite al driver soportar diferentes frecuencias de reloj

### GsDeviceBitTiming

**Dirección:** Host → Dev (control_out)
**BREQ:** `GS_USB_BREQ_BITTIMING` (1)

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GsDeviceBitTiming {
    pub prop_seg: u32,     // Propagation segment
    pub phase_seg1: u32,   // Phase segment 1
    pub phase_seg2: u32,   // Phase segment 2
    pub sjw: u32,          // Sync Jump Width
    pub brp: u32,          // Baud Rate Prescaler
}
```

**Tamaño:** 20 bytes

**Uso:**
- El host envía esta estructura para configurar la velocidad del bus CAN
- El BSP lo convierte a `NominalBitTiming` de Embassy

### GsHostFrame (device → host)

**Dirección:** Dev → Host (Bulk IN)

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default)]
pub struct GsHostFrame {
    pub echo_id: u32,      // 0 para RX, >0 para echo de TX
    pub can_id: u32,       // ID + flags (EFF, RTR, ERR)
    pub can_dlc: u8,       // Data Length Code (0-8)
    pub channel: u8,       // Canal (0 para un solo CAN)
    pub flags: u8,         // GS_USB_FLAG_* o GS_CAN_FLAG_*
    pub reserved: u8,      // Reservado (0)
    pub data: [u8; 8],     // Payload CAN
}
```

**Tamaño:** 20 bytes

**Campos detallados:**

#### echo_id
- **0:** Trama recibida del bus CAN
- **>0:** Echo de transmisión (el mismo ID que envió el host)
- Permite al driver correlacionar TX con confirmaciones

#### can_id
```
Bit 31: EFF flag (Extended Frame Format)
Bit 30: RTR flag (Remote Transmission Request)
Bit 29: ERR flag (Error Frame)
Bits 28-0: ID del mensaje CAN

Ejemplo:
- 0x00000123 = ID estándar 0x123
- 0x80000123 = ID extendido 0x123
- 0x40000123 = RTR con ID 0x123
```

#### can_dlc
- **0-8:** Bytes de datos válidos
- **>8:** Reservado para CAN FD (no implementado aún)

#### flags
- **Bit 0:** `GS_USB_FLAG_TX_ECHO` - Es un echo de transmisión
- **Bit 1:** `GS_USB_FLAG_FD` - Trama CAN FD
- **Bit 2:** `GS_USB_FLAG_BRS` - Bit Rate Switch
- **Bit 3:** `GS_USB_FLAG_ESI` - Error State Indicator

### GsTxMsg (host → device)

**Dirección:** Host → Dev (Bulk OUT)

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default)]
pub struct GsTxMsg {
    pub echo_id: u32,      // ID para eco (el device lo retorna)
    pub can_id: u32,       // ID + flags
    pub can_dlc: u8,       // DLC (0-8)
    pub channel: u8,       // Canal (0)
    pub flags: u8,         // Flags
    pub reserved: u8,      // Reservado
    pub data: [u8; 8],     // Payload
}
```

**Tamaño:** 20 bytes

**Métodos:**
```rust
impl GsTxMsg {
    pub fn id(&self) -> u32;        // ID sin flags
    pub fn is_extended(&self) -> bool;  // ¿Tiene flag EFF?
    pub fn is_rtr(&self) -> bool;       // ¿Tiene flag RTR?
    pub fn dlc(&self) -> u8;            // DLC con límite de 8
}
```

### GsIdentifyMode

**Dirección:** Host → Device (control_out)
**BREQ:** `GS_USB_BREQ_IDENTIFY` (7)

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default, PartialEq, Eq)]
pub struct GsIdentifyMode {
    pub mode: u32,
}
```

**Tamaño:** 4 bytes

Valores válidos:
- `0` (`GsIdentifyMode::OFF`) — apagar LED de identificación
- `1` (`GsIdentifyMode::ON`) — encender LED

El handler solo invoca el callback `on_identify(bool)` si la feature `GS_CAN_FEATURE_IDENTIFY` está activa en `capabilities` — esto previene activar hardware que no existe.

### GsDeviceCapabilities

**Dirección:** Device → Host (control_in)
**BREQ:** `GS_USB_BREQ_DEV_CAPABILITIES` (11) — *no estándar, no usado por el driver `gs_usb` del kernel*

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default, PartialEq, Eq)]
pub struct GsDeviceCapabilities {
    pub feature: u32,
}
```

**Tamaño:** 4 bytes

Mismo bitfield de features que `GsDeviceBtConst::feature` (ver tabla de Features arriba). Pensado para exponer capacidades por un endpoint dedicado sin tener que pedir el BT_CONST de 40 bytes. El firmware lo popula con `with_feature(...)` encadenable.

## Layout de Bits - can_id

### Standard ID (11-bit)

```
31 30 29 28 27 26 25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
 0  0  0 └─────────────────────────────── ID (11 bits) ─────────────────────────────────────────────┘
```

### Extended ID (29-bit)

```
31 30 29 28 27 26 25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
 1  0  0 └───────────────────────────────── ID (29 bits) ─────────────────────────────────────────┘
```

### RTR Frame

```
31 30 29 28 27 26 25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
 0  1  0 └─────────────────────────────── ID (11 bits) ─────────────────────────────────────────────┘
```

## Serialización con bytemuck

### Requisitos

- `#[repr(C)]` - Orden de campos predecible
- `Pod` - Tipo "plain old data" (sin padding)
- `Zeroable` - Se puede inicializar con ceros

### Ejemplo de serialización

```rust
// Device → Host
let frame = GsHostFrame::from_can_frame(0x123, false, 8, &data);
let bytes: &[u8; 20] = bytemuck::bytes_of(&frame);
ep_in.write(bytes).await;

// Host → Device
let bytes = ep_out.read(&mut buf).await?;
let msg: GsTxMsg = bytemuck::pod_read_unaligned(&buf[..20]);
```

### Layout en memoria (little-endian)

```
GsHostFrame (20 bytes):
Offset  Size  Campo
0       4     echo_id (LE)
4       4     can_id (LE)
8       1     can_dlc
9       1     channel
10      1     flags
11      1     reserved
12      8     data
```

## Compatibilidad con gs_usb

### Driver Linux

El firmware es compatible con el driver `gs_usb` del kernel Linux:

```bash
# Verificar que el device es detectado
lsusb | grep 1d50:606f

# Configurar interface CAN
sudo ip link set can0 type can bitrate 500000

# Activar interface
sudo ip link set can0 up

# Usar con SocketCAN
candump can0
cansend can0 123#DEADBEEF
```

### Formato de tramas

El driver Linux espera exactamente 20 bytes por trama:
- RX: GsHostFrame con echo_id = 0
- TX: GsTxMsg con echo_id > 0
- Echo: GsHostFrame con echo_id > 0 y flags = GS_USB_FLAG_TX_ECHO

## Tests

Todos los tests están en `gs_usb_types.rs`:

```bash
# Ejecutar todos los tests
cargo test -p gs-usb-protocol

# Tests específicos
cargo test test_gs_host_frame_tamaño
cargo test test_gs_tx_msg_deserializacion
cargo test test_from_tx_msg_echo
```

**Cobertura:**
- Tamaño de structs (20 bytes cada uno)
- Serialización/deserialización con bytemuck
- Extracción de IDs (SFF/EFF)
- Detección de RTR
- Límites de DLC
- Creación de echoes

## Errores Comunes

### 1. ID inválido

```rust
// Standard ID > 0x7FF
let result = StandardId::new(0x800);
assert!(result.is_none()); // Correcto: ID inválido

// Extended ID > 0x1FFFFFFF
let result = ExtendedId::new(0x20000000);
assert!(result.is_none()); // Correcto: ID inválido
```

### 2. DLC > 8 (CAN clásico)

```rust
let msg = GsTxMsg { can_dlc: 10, .. };
assert_eq!(msg.dlc(), 8); // Se limita a 8
```

### 3. Flags incorrectos

```rust
// ERROR: Usar GS_CAN_ID_FLAG_EFF en can_dlc
let frame = GsHostFrame {
    can_dlc: GS_CAN_ID_FLAG_EFF as u8, // ¡Incorrecto!
    ..
};

// CORRECTO: Usar flags en can_id
let frame = GsHostFrame {
    can_id: 0x123 | GS_CAN_ID_FLAG_EFF, // Correcto
    ..
};
```

## Referencias

- [gs_usb.h (kernel)](https://github.com/torvalds/linux/blob/master/include/uapi/linux/can/gs_usb.h)
- [gs_usb.c (driver)](https://github.com/torvalds/linux/blob/master/drivers/net/can/usb/gs_usb.c)
- [SocketCAN documentation](https://www.kernel.org/doc/html/latest/networking/can.html)
