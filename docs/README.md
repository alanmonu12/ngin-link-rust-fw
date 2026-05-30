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
3. **usb-data-flow.md** - Cómo fluyen los datos USB
4. **gs-usb-types-reference.md** - Tipos de datos del protocolo

### Para Entender la Fase 2

1. **usb-tx-rx.md** - Implementación completa
2. **channel-design.md** - Patrones de comunicación
3. **performance-optimizations.md** - Optimizaciones

### Para Debugging

1. **troubleshooting.md** - Problemas comunes y soluciones
2. **testing-guide.md** - Tests unitarios

## Resumen de la Fase 2

### Componentes Implementados

1. **GsTxMsg** - Struct para mensajes host→device
2. **GsHostFrame::from_tx_msg_echo()** - Creación de echoes
3. **BspCan::transmit()** - Transmisión CAN async
4. **CAN_TX_CHANNEL** - Canal para solicitudes TX
5. **USB_ECHO_CHANNEL** - Canal para echoes
6. **can_rx_task** - Modificado con select3()
7. **usb_tx_task** - Modificado con select()
8. **usb_rx_task** - Implementado completamente

### Archivos Modificados

```
crates/gs-usb-protocol/src/gs_usb_types.rs
crates/bsp-f446/src/can.rs
firmware/src/main.rs
```

### Tests

- 24 tests unitarios (21 en gs-usb-protocol, 3 en can-protocol)
- Todos pasan en host (x86_64/aarch64)
- Build exitoso para thumbv7em-none-eabihf

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

# Tests unitarios
cargo test -p gs-usb-protocol --target aarch64-apple-darwin
cargo test -p can-protocol --target aarch64-apple-darwin

# Logs en tiempo real
DEFMT_LOG=trace cargo run --release

# Verificar tamaño
cargo size --release
```

## Próximos Pasos

### Fase 3: Comandos gs_usb Adicionales

- [ ] GS_USB_BREQ_TIMESTAMP
- [ ] GS_USB_BREQ_IDENTIFY
- [ ] GS_USB_BREQ_GET_USER_ID / SET_USER_ID
- [ ] GS_USB_BREQ_DEV_CAPABILITIES

### Fase 4: Manejo de Errores

- [ ] Error handling en usb_tx_task
- [ ] Error handling en usb_rx_task
- [ ] Callbacks con manejo de errores
- [ ] Watchdog

### Fase 5: CAN Avanzado

- [ ] Filtros CAN configurables
- [ ] Timestamps en tramas
- [ ] ISO-TP multi-frame
- [ ] OBD2 response parsing

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
