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
│       ├── main.rs        # Entry point: pipeline de 6 fases, sin lógica de negocio
│       ├── app_context.rs # Estado central: channels, señales, CanService, callbacks
│       ├── usb_setup.rs   # build_usb_stack(), StaticCells, tipos USB (genérico sobre Driver)
│       └── tasks/
│           ├── mod.rs      # Re-exports públicos
│           ├── can.rs      # can_driver_task (actor único CAN, bus-off recovery, métricas)
│           ├── usb_run.rs  # usb_run (corre UsbDevice, señala USB_DEVICE_READY)
│           ├── usb_tx.rs   # usb_tx_task (CAN→host + decodificación OBD2/UDS)
│           ├── usb_rx.rs   # usb_rx_task (host→CAN)
│           └── health.rs   # health_monitor (IWDG + métricas CAN cada 30s)
├── crates/
│   ├── bsp-f446/          # Board Support Package para STM32F446
│   │   └── src/
│   │       ├── lib.rs     # init() -> Result<Board, ()>, ResetReason, config RCC/PLL
│   │       ├── usb.rs     # Driver USB OTG FS, bind_interrupts, type alias
│   │       ├── can.rs     # Driver bxCAN, start/stop/set_bit_timing
│   │       └── watchdog.rs # BspWatchdog (IWDG wrapper: new/unleash/pet)
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
- `portable-atomic` 1.0 (feature: `unsafe-assume-single-core` para Cortex-M)

## Flujo de Datos (Arquitectura Actual)
```
Bus CAN físico → bxCAN HW → Embassy CAN Driver → can_driver_task (Actor único del hardware)
                                                       ├─ RX OK → CAN_RX_CHANNEL → usb_tx_task → USB EP IN (al host)
                                                       └─ Error → USB_ECHO_CHANNEL → usb_tx_task → USB EP IN (error frame al host)

Control y TX (Host → Device → Bus CAN):
Host USB → gs_usb control transfer → GsUsbControlHandler
  ├── GS_USB_BREQ_BITTIMING → on_bit_timing_cb → CAN_CMD_CHANNEL → can_driver_task
  ├── GS_USB_BREQ_MODE (START) → on_start_cb(flags) → CAN_CMD_CHANNEL → can_driver_task
  ├── GS_USB_BREQ_MODE (STOP) → on_stop_cb → CAN_CMD_CHANNEL → can_driver_task
  └── Bulk OUT (GsTxMsg) → usb_rx_task → CAN_CMD_CHANNEL → can_driver_task
                                                                    ├─ TX OK → USB_ECHO_CHANNEL → usb_tx_task → host
                                                                    └─ TX Timeout → log + descarta
```

## Pipeline de Inicio (main.rs)
```
Fase 1: Hardware     → BSP init (Result<Board, ()>), en fallo → sys_reset
Fase 2: Stack USB    → build_usb_stack(), configuración gs_usb
Fase 3: Watchdog     → BspWatchdog::new(iwdg, 5000ms)
Fase 4: Spawn tareas → usb_run, can_driver, usb_tx, usb_rx, health_monitor
Fase 5: Confirmación → Espera USB_DEVICE_READY, USB_TX_READY, USB_RX_READY, CAN_READY (5s timeout cada una)
Fase 6: Loop         → Idle 60s (watchdog lo alimenta health_monitor)
```

## Estado Actual (WIP)
### Completado
- Workspace con 4 crates, arquitectura Clean/Clean-ish
- BSP-F446: init con PLL (84MHz sys, 48MHz USB), drivers USB y CAN
- BSP-F446: init() retorna Result<Board, ()>, lee ResetReason de RCC_CSR
- BSP-F446: módulo watchdog (BspWatchdog wrapper sobre IWDG)
- gs_usb protocol: handler de control transfers, tipos C-repr, config USB
- can-protocol: CanFrame, decodificador OBD2 y UDS (sniffer básico)
- Tarea can_driver_task: actor único del hardware CAN (RX del bus + TX + control)
- Tarea usb_tx_task: envía GsHostFrame por Bulk IN (RX del bus + echoes de TX) + decodifica OBD2/UDS
- Tarea usb_rx_task: lee GsTxMsg por Bulk OUT y los enruta a CAN_CMD_CHANNEL como CanDriverCmd::Transmit
- Tarea health_monitor: alimenta IWDG cada 1s, logea métricas CAN cada 30s
- Bucle del driver CAN (can_driver_task con `select` de 2 fuentes: CAN_CMD_CHANNEL + can.read())
- Canal unificado CAN_CMD_CHANNEL para control + TX (Start/Stop/SetBitTiming/Transmit)
- Comandos gs_usb soportados: TIMESTAMP, IDENTIFY, GET/SET_USER_ID, DEV_CAPABILITIES
- Error frames SocketCAN: mapeo de errores bxCAN → GsHostFrame con from_bus_error/from_controller_error
- Modos CAN dinámicos: listen-only, loopback, one-shot pasados desde host USB
- Filtros CAN: accept_all configurado en start() (bxCAN sin filtros rechaza todo)
- Recuperación automática de bus-off con backoff exponencial (100-3200ms, max 5 reintentos)
- Bus-off agotado envía CanControllerError::BusOff al host
- TX con timeout (100ms) y abort de mailboxes
- Interoperabilidad verificada con driver gs_usb del kernel Linux (candump funciona)
- CanService: métricas atómicas (error_count, dropped_rx, bus_off_count), mark_started/mark_stopped
- Señales de inicio: CAN_READY, USB_TX_READY, USB_RX_READY, USB_DEVICE_READY con timeout 5s
- try_send en can_driver_task (no bloquea si canal lleno), cuenta dropped_rx
- Logs rutinarios en trace!, solo eventos de altoValor en info/warn/error
- 68 tests unitarios en gs-usb-protocol + 3 en can-protocol (corren en host con `cargo test-linux`)
- Estructura modular: main.rs (pipeline 6 fases), app_context.rs (estado central), usb_setup.rs, tasks/

### Pendiente / TODO
- **CAN FD:** No soportado aún
- **Timestamps en GsHostFrame:** El timestamp se expone por control transfer, no embebido en cada frame
- **Filtros CAN:** No hay configuración de filtros de aceptación (siempre accept_all)
- **ISO-TP multi-frame:** Solo se decodifican Single Frames
- **OBD2 response parsing:** Solo requests
- **UDS sub-function/DID:** Parsing parcial
- **LEDs/Indicadores:** Callback `on_identify` existe pero el GPIO no está cableado
- **Persistencia de user_id:** Solo vive en RAM; no se guarda en flash

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
7. El flujo principal está en tasks/: can_driver_task → CAN_RX_CHANNEL → usb_tx_task
8. Para agregar nuevas capacidades USB, modificar GsUsbControlHandler
9. `can_driver_task` es el actor único del hardware CAN; no se puede separar RX/TX/CTRL en tareas diferentes porque `embassy_stm32::can::Can` requiere `&mut self`
10. `app_context.rs` concentra todo el estado compartido (channels, señales, CanService, callbacks); `usb_setup.rs` construye el stack USB de forma genérica sobre `Driver<'static>`
11. `gs-usb-protocol` usa `cfg(feature = "defmt")` para logs, no `cfg(test)`. Correr tests con `cargo test-linux` (sin feature defmt)
12. `portable-atomic` feature es `unsafe-assume-single-core` (no `unsafe-assume-single-thread`)
13. `gs_usb_types.rs` tiene `CanControllerError::BusOff` variant usado para bus-off notification
14. El `overflow_pending` flag en can_driver_task es un placeholder — full `GS_CAN_FLAG_OVERFLOW` implementation deferred to Fase 1
15. BSP `can.rs` tiene `CanTxError::Timeout` y `CanTxError::FrameError` variants
16. Embassy `IndependentWatchdog` está en `embassy_stm32::wdg` module (no `iwdg`)
17. Build warning sobre `#[cfg_attr(feature = "defmt")]` en bsp-f446/can.rs es benigna (missing feature declaration in Cargo.toml)

## Lecciones Aprendidas de Interoperabilidad con gs_usb

### Bug: interface_count y el driver del kernel
El struct `GsDeviceConfig` tiene un campo `icount` (1 byte) que indica la cantidad de interfaces CAN.
El driver `gs_usb` del kernel Linux hace `icount = dconf.icount + 1`, por lo que:
- `icount = 0` → 1 interfaz CAN (can0) ✅
- `icount = 1` → 2 interfaces CAN (can0, can1) ❌

Referencia: `drivers/net/can/usb/gs_usb.c` función `gs_usb_probe()`.

### Bug: echo_id para frames RX
El driver gs_usb del kernel usa `echo_id == GS_HOST_FRAME_ECHO_ID_RX` (0xFFFFFFFF) para distinguir
frames recibidos del bus de ecos de transmisión. Si se envía `echo_id = 0`, el kernel lo interpreta
como un eco de TX pendiente y lo descarta con "Unexpected unused echo id 0".
Referencia: `gs_usb_receive_bulk_callback()` en `gs_usb.c`.

### Bug: bt_const_feature debe reflejar capabilities
`GsDeviceBtConst.feature` debe coincidir con `GsDeviceCapabilities.feature`. El kernel consulta
`GS_USB_BREQ_BT_CONST` para determinar las capacidades del dispositivo. Si faltan features,
el modo listen-only no se habilita correctamente en SocketCAN.

### Bug: sw_version e IDENTIFY
El kernel solo habilita `GS_CAN_FEATURE_IDENTIFY` si `sw_version > 1`. Usar `sw_version = 1`
causa que el feature se deshabilite a pesar de estar en capabilities.

### Compatibilidad gs_usb: Struct layout
Todos los structs que se intercambian con el host deben ser `#[repr(C)]` y `Pod` (bytemuck).
El kernel usa `__packed` en los structs equivalentes. Verificar que los tamaños coincidan:
- `GsDeviceConfig`: 12 bytes
- `GsDeviceBtConst`: 40 bytes
- `GsHostFrame`: 20 bytes (header sin timestamp)
- `GsTxMsg`: 20 bytes
- `GsDeviceMode`: 8 bytes
