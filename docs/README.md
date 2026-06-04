# Documentación del Firmware

## Visión General

Documentación completa del firmware Ngin-Link para dongle USB-CAN basado en gs_usb.

## Estructura de Documentación

### Documentos Existentes

| Archivo                                                      | Descripción                                  |
| --------------------------------------------------------------| ----------------------------------------------|
| [architecture.md](architecture.md)                           | Arquitectura general del sistema             |
| [BSP-446.md](BSP-446.md)                                     | Board Support Package para STM32F446         |
| [gs-host-frame.md](gs-host-frame.md)                         | Documentación de GsHostFrame                 |
| [usb-data-flow.md](usb-data-flow.md)                         | Flujo de datos USB                           |
| [usb-tx-rx.md](usb-tx-rx.md)                                 | Implementación completa USB TX/RX            |
| [channel-design.md](channel-design.md)                       | Diseño de canales y patrones de comunicación |
| [gs-usb-types-reference.md](gs-usb-types-reference.md)       | Referencia de tipos gs_usb                   |
| [performance-optimizations.md](performance-optimizations.md) | Optimizaciones de performance                |
| [troubleshooting.md](troubleshooting.md)                     | Guía de troubleshooting y debugging          |
| [testing-guide.md](testing-guide.md)                         | Guía de testing y tests unitarios            |

## Orden de Lectura Recomendado

### Para Entender el Sistema

1. **architecture.md** - Visión general de la arquitectura
2. **BSP-446.md** - Detalles del hardware y periféricos
3. **usb-data-flow.md** - Cómo fluyen los datos USB y los control transfers
4. **gs-usb-types-reference.md** - Tipos de datos del protocolo (incluye GsIdentifyMode y GsDeviceCapabilities)

### Para Entender la Transferencia Bidireccional

1. **usb-tx-rx.md** - Implementación completa
2. **channel-design.md** - Patrones de comunicación
3. **performance-optimizations.md** - Optimizaciones

### Para Debugging

1. **troubleshooting.md** - Problemas comunes y soluciones
2. **testing-guide.md** - Tests unitarios

## Comandos de Control gs_usb Soportados

El handler `GsUsbControlHandler` implementa el set completo de BREQ que el driver `gs_usb` del kernel Linux puede solicitar, más algunas extensiones propietarias:

| BREQ | Nombre | Wire format | Notas |
|------|--------|-------------|-------|
| 1 | `BITTIMING` | `GsDeviceBitTiming` (20 B) | Configura velocidad del bus |
| 2 | `MODE` | u32 (bit 0 = start) | Start/Stop del controlador |
| 4 | `BT_CONST` | `GsDeviceBtConst` (40 B) | Reporta clock y límites |
| 5 | `DEVICE_CONFIG` | `GsDeviceConfig` (12 B) | ID de interfaces y versión |
| 6 | `TIMESTAMP` | u32 (ms) | Lee de `handler.now_ms()` |
| 7 | `IDENTIFY` | `GsIdentifyMode` (u32) | Callback `on_identify(bool)` |
| 8 | `GET_USER_ID` | u32 | Devuelve `handler.user_id` |
| 9 | `SET_USER_ID` | u32 | Actualiza `handler.user_id` |
| 11 | `DEV_CAPABILITIES` | `GsDeviceCapabilities` (4 B) | Reporta feature flags |

El bitfield de features coincide con `include/uapi/linux/can/gs_usb.h` y un test (`test_feature_flags_coinciden_con_kernel_linux`) fija los valores para evitar drift silencioso.

## Componentes Implementados

1. **GsTxMsg** - Struct para mensajes host→device
2. **GsHostFrame::from_tx_msg_echo()** - Creación de echoes
3. **BspCan::transmit()** - Transmisión CAN async
4. **CAN_CMD_CHANNEL** - Canal unificado para comandos control + TX
5. **USB_ECHO_CHANNEL** - Canal para echoes
6. **can_driver_task** - Actor único del hardware CAN (select de 2 fuentes)
7. **usb_tx_task** - Multiplexor + decodificación OBD2/UDS
8. **usb_rx_task** - Implementado completamente
9. **GsIdentifyMode / GsDeviceCapabilities** - Tipos para IDENTIFY y DEV_CAPABILITIES

### Archivos Principales

```
crates/gs-usb-protocol/src/gs_usb_types.rs   # Tipos wire (incluye nuevos structs)
crates/gs-usb-protocol/src/handler.rs        # Lógica del control transfer
crates/gs-usb-protocol/src/handler_tests.rs  # Tests unitarios
crates/bsp-f446/src/can.rs                   # Driver bxCAN
firmware/src/main.rs                         # Integración + callbacks
```

### Tests

- 60 tests unitarios (57 en gs-usb-protocol, 3 en can-protocol)
- Todos pasan en host (x86_64/aarch64)
- Build exitoso para thumbv7em-none-eabihf
- `cargo clippy -p gs-usb-protocol --all-targets -- -D warnings` limpio

## Métricas de Performance

| Métrica | Objetivo | Actual |
|---------|----------|--------|
| Throughput CAN RX | 10,000/seg | ~4,500 (limitado por USB) |
| Throughput CAN TX | 10,000/seg | ~4,500 (limitado por USB) |
| Latencia end-to-end | < 5ms | ~1.03ms |
| Drop rate | 0% a 1,000/seg | 0% verificado |
| RAM usage | < 10KB | ~2KB (canales) |

## Comandos Útiles

```bash
# Compilar firmware
cargo build --release

# Flashear con probe-rs
cargo run --release

# Tests unitarios (solo crates host-testables)
cargo test-mac -p gs-usb-protocol
cargo test-mac -p can-protocol

# Clippy estricto sobre gs-usb-protocol
cargo clippy -p gs-usb-protocol --all-targets -- -D warnings

# Logs en tiempo real
DEFMT_LOG=trace cargo run --release

# Verificar tamaño
cargo size --release
```

## Próximos Pasos

### Pendiente

- **CAN FD:** No soportado aún
- **Filtros CAN:** No hay configuración de filtros de aceptación
- **ISO-TP multi-frame:** Solo se decodifican Single Frames
- **OBD2 response parsing:** Solo requests
- **UDS sub-function/DID:** Parsing parcial
- **LEDs/Indicadores:** Callback `on_identify` existe pero el GPIO no está cableado
- **Watchdog:** No implementado
- **Persistencia de user_id:** Solo vive en RAM; no se guarda en flash

## Convenciones

- Código en español (comentarios, nombres)
- `#![no_std]` en todos los crates
- Tests unitarios para lógica pura
- `defmt` para logs (no `println!`)
- No usar `unwrap()` en paths de producción

## Recursos

- [Embassy Documentation](https://docs.embassy.dev/)
- [gs_usb kernel driver](https://github.com/torvalds/linux/blob/master/drivers/net/can/usb/gs_usb.c)
- [SocketCAN](https://www.kernel.org/doc/html/latest/networking/can.html)
- [probe-rs](https://probe.rs/)
- [defmt](https://defmt.ferrous-systems.com/)
