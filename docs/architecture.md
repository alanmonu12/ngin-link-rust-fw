# Arquitectura del firmware

## Principios de diseño

1. **Separación por responsabilidad**: Cada crate tiene un único propósito
2. **Independencia de hardware**: La lógica de protocolo no sabe del chip
3. **Zero-copy donde importa**: `bytemuck` para serialización, canales Embassy para messaging
4. **Async everywhere**: Tareas Embassy cooperan sin bloquear el CPU

## Crates

<div align="center">
  <img src="../imgs/ Estructura-de-Crates.png" alt="Estructura de Crates" />
  <p><em>Estructura modular del firmware: firmware orquesta BSP, protocolo gs_usb y decodificación CAN.</em></p>
</div>

## Tareas Embassy

<div align="center">
  <img src="../imgs/Tareas-Embassy.png" alt="Tareas Embassy" />
  <p><em>Tareas asíncronas Embassy: USB driver, CAN RX, USB TX y USB RX (fase 2).</em></p>
</div>

## Canales

```
CAN_RX_CHANNEL:     Channel<CanFrame, 32>         CAN RX → USB TX
CAN_CMD_CHANNEL:    Channel<CanDriverCmd, 16>      USB Control / TX → CAN driver
USB_ECHO_CHANNEL:   Channel<GsHostFrame, 16>      TX echo → USB TX
```

`CanDriverCmd` es un enum que unifica control y transmisión:
- `Start` — Habilita el controlador CAN
- `Stop` — Lo pone en modo silencio
- `SetBitTiming(GsDeviceBitTiming)` — Configura la velocidad
- `Transmit(CanTxRequest)` — Solicitud de transmisión desde el host

## Por qué este diseño

| Decisión | Razón |
|----------|-------|
| `Channel` en vez de `Mutex+Queue` | Embassy channels son lock-free y async-aware |
| `CriticalSectionRawMutex` | Funciona en interrupt context (CAN RX puede venir de IRQ) |
| `#[repr(C)]` + `Pod` en frames | Serialización zero-copy al USB, compatible con el driver Linux |
| Separar gs_usb_protocol del BSP | Permite testear el handler de control transfers en host sin hardware |
| Buffer de 32 en CAN_RX_CHANNEL | Absorbe ráfagas típicas de diagnóstico (flash, lectura masiva de PIDs) |
| Endpoint IN de 64 bytes | Máximo para USB Full Speed, un GsHostFrame cabe exacto (20 bytes) |
| Canal único `CAN_CMD_CHANNEL` | Simplifica el actor del driver CAN: un solo `select` de 2 fuentes en lugar de `select3` |
