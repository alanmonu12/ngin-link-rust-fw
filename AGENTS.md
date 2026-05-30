# AGENTS.md - Contexto del Proyecto Ngin-Link Firmware

## Descripción General
Firmware en Rust para un dongle USB-CAN basado en gs_usb. Hardware open-source, software open-source. Diseñado para sniffing/diagnóstico de buses CAN en vehículos (aplicaciones de chiptuning, debugging de herramientas como Kess/Flex). Es compatible con SocketCAN en Linux gracias al protocolo gs_usb.

## Stack Tecnológico
- **Lenguaje:** Rust (`#![no_std]`, bare-metal)
- **Framework:** Embassy (async embedded)
- **MCU:** STM32F446RE (ARM Cortex-M4, 84MHz sysclk, 48MHz USB)
- **Target:** `thumbv7em-none-eabihf`
- **Debug/Flash:** probe-rs (`probe-rs run --chip STM32F446RE`)
- **Protocolo USB:** gs_usb (VID: 0x1d50, PID: 0x606f)
- **Periféricos:** USB OTG FS (PA11/PA12), bxCAN1 (PB8=RX, PB9=TX)

## Arquitectura del Workspace

```
ngin-link-rust-fw/
├── Cargo.toml              # Workspace root (resolver = "2")
├── firmware/               # Binario principal
│   ├── Cargo.toml
│   ├── build.rs           # Linker flags (--nmagic)
│   └── src/
│       └── main.rs        # Entry point, tareas Embassy, ruteo CAN<->USB
├── crates/
│   ├── bsp-f446/          # Board Support Package para STM32F446
│   │   └── src/
│   │       ├── lib.rs     # init() -> Board, config RCC/PLL (84MHz/48MHz USB)
│   │       ├── usb.rs     # Driver USB OTG FS, bind_interrupts, type alias
│   │       └── can.rs     # Driver bxCAN, start/stop/set_bit_timing
│   ├── gs-usb-protocol/   # Protocolo gs_usb (independiente de hardware)
│   │   └── src/
│   │       ├── lib.rs     # Config USB default (VID/PID), no_std con cfg_attr
│   │       ├── gs_usb_types.rs  # Structs C-repr: GsDeviceBitTiming, GsDeviceBtConst, etc.
│   │       ├── handler.rs       # GsUsbControlHandler: control_in/control_out
│   │       └── handler_tests.rs # Tests unitarios (corren en host)
│   └── can-protocol/      # Decodificador OBD2/UDS
│       └── src/
│           ├── lib.rs     # CanFrame, analyze_frame(), DecodedProtocol enum
│           ├── obd2.rs    # parse_obd2_request()
│           └── uds.rs     # parse_uds_message()
├── docs/
│   └── BSP-446.md         # Documentación detallada del BSP
├── .cargo/
│   └── config.toml        # Target, runner probe-rs, alias para test-linux
└── GEMINI.md              # Instrucciones para agentes AI (referencia)
```

## Dependencias Clave (workspace.dependencies en Cargo.toml)
- `embassy-stm32` 0.6.0 (features: stm32f446re, unstable-pac, memory-x, time-driver-any, exti)
- `embassy-executor` 0.10.0 (platform-cortex-m, executor-thread)
- `embassy-sync` 0.6.0
- `embassy-usb` 0.6.0
- `embassy-time` 0.5.1
- `cortex-m-rt` 0.7, `cortex-m` 0.7.7
- `defmt` 0.3, `bytemuck` 1.16.0

## Flujo de Datos (Arquitectura Actual)
```
Bus CAN físico → bxCAN HW → Embassy CAN Driver → can_rx_task (Productor)
                                                       ↓
                                               CAN_RX_CHANNEL (Channel<CanFrame, 32>)
                                                       ↓
                                               usb_tx_task (Consumidor) → [TODO: USB EP IN]
```

**Control (Host → Device):**
```
Host USB → gs_usb control transfer → GsUsbControlHandler
  ├── GS_USB_BREQ_BITTIMING → on_bit_timing_cb → CAN_CTRL_CHANNEL → can_rx_task
  ├── GS_USB_BREQ_MODE (START) → on_start_cb → CAN_CTRL_CHANNEL → can_rx_task
  └── GS_USB_BREQ_MODE (STOP) → on_stop_cb → CAN_CTRL_CHANNEL → can_rx_task
```

## Estado Actual (WIP)
### Completado
- Workspace con 4 crates, arquitectura Clean/Clean-ish
- BSP-F446: init con PLL (84MHz sys, 48MHz USB), drivers USB y CAN
- gs_usb protocol: handler de control transfers, tipos C-repr, config USB
- can-protocol: CanFrame, decodificador OBD2 y UDS (sniffer básico)
- Tarea can_rx_task: recibe tramas CAN, las decodifica, envía por canal
- Canal de control USB→CAN para start/stop/bit_timing
- Tests unitarios para gs_usb_handler y can-protocol (corren en host)

### Pendiente / TODO
- **usb_tx_task:** No envía tramas al host aún (solo lee del canal)
- **USB Bulk IN endpoint:** No está configurado para enviar datos
- **USB Bulk OUT endpoint:** No existe接收 comandos de trama desde el host
- **Transmisión CAN:** No hay tarea para enviar tramas CAN desde el host
- **CAN FD:** No soportado aún
- **Timestamps:** No implementados en tramas USB
- **Filtros CAN:** No hay configuración de filtros de recepción
- **Error handling en USB:** Falta manejo robusto de errores USB
- **LEDs/Indicadores:** No implementados
- **Watchdog:** No implementado

## Convenciones del Proyecto
- Código en español (comentarios, nombres de variables, commits)
- Commits siguen formato: `tipo(alcance): descripción`
  - Tipos: feat, fix, refactor, test, docs
- `#![no_std]` en todos los crates excepto cuando se ejecutan tests en host
- `gs-usb-protocol` debe ser 100% independiente de hardware (testeable en host)
- `bsp-f446` es el único crate que sabe del chip específico
- `can-protocol` es puro lógica, sin dependencias Embassy
- No usar `unwrap()`/`expect()` en paths de producción
- Usar `defmt` para logs (no `println!` ni `log`)

## Comandos Útiles
```bash
# Compilar para el target ARM
cargo build --release

# Flashear con probe-rs
cargo run --release

# Tests unitarios en host (gs-usb-protocol y can-protocol)
cargo test-linux    # alias para --target x86_64-unknown-linux-gnu
cargo test-mac      # alias para --target aarch64-apple-darwin

# Test de un crate específico
cargo test-linux -p gs-usb-protocol
cargo test-linux -p can-protocol

# Ver logs en tiempo real (defmt)
DEFMT_LOG=trace cargo run --release
```

## Notas para Agentes AI
1. Siempre escribir Rust idiomático y seguro, siguiendo patrones Embassy
2. Todo el código debe ser `#![no_std]` compatible (no usar alloc sin justificación)
3. El código debe ser educativo y estar bien comentado (propósito del proyecto)
4. Al modificar gs_usb_types.rs, asegurar que los structs sean `#[repr(C)]` y `Pod`
5. Los tests de gs_usb_protocol y can_protocol corren en host, no en el MCU
6. El BSP encapsula TODO lo relacionado al hardware específico del STM32F446
7. El flujo principal está en main.rs: can_rx_task → Canal → usb_tx_task
8. Para agregar nuevas capacidades USB, modificar GsUsbControlHandler
