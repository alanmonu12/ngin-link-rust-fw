# Ngin-Link: USB-to-CAN Dongle Firmware (STM32F4 + Rust)

![Language](https://img.shields.io/badge/Language-Rust-orange.svg)
![Architecture](https://img.shields.io/badge/Arch-ARM%20Cortex--M4-blue)
![Framework](https://img.shields.io/badge/Framework-Embassy-green)
![Status](https://img.shields.io/badge/Status-WIP-yellow)
![License](https://img.shields.io/badge/License-MIT-brightgreen)

<div align="center">
  <img src="imgs/ngin-logo.png" alt="Ngin Performance" width="250"/>
</div>
<br>


Dongle USB-CAN open-source basado en el protocolo [gs_usb](https://github.com/torvalds/linux/blob/master/drivers/net/can/usb/gs_usb.c), compatible con **SocketCAN** en Linux. Diseñado para sniffing y diagnóstico de buses CAN en vehículos (chiptuning, debugging de herramientas como Kess/Flex).

Proyecto de **Ngin Performance** — hardware open-source, software open-source.

## Hardware

- **Microcontrolador:** STM32F446RE (ARM Cortex-M4, 84 MHz sysclk, 48 MHz USB)
- **USB:** OTG FS (PA11=DM, PA12=DP)
- **CAN:** bxCAN1 (PB8=RX, PB9=TX)
- **Cristal externo:** 8 MHz (modo bypass, típico en placas Nucleo)
- **Compatibilidad:** SocketCAN en Linux (driver `gs_usb` incluido en kernel)

## Stack Tecnológico

| Componente | Tecnología |
|------------|------------|
| Lenguaje | Rust (`#![no_std]`, bare-metal) |
| Framework | Embassy 0.6 (async embedded) |
| MCU | STM32F446RE (Cortex-M4, 84 MHz) |
| Target | `thumbv7em-none-eabihf` |
| Debug/Flash | probe-rs |
| Protocolo USB | gs_usb (VID: `0x1d50`, PID: `0x606f`) |
| Periféricos | USB OTG FS (PA11/PA12), bxCAN1 (PB8/PB9) |
| Logs | defmt (RTT) |

## Arquitectura del Workspace

```
ngin-link-rust-fw/
├── Cargo.toml              # Workspace root (resolver = "2")
├── firmware/               # Binario principal
│   ├── src/
│   │   ├── main.rs         # Pipeline de 6 fases, sin lógica de negocio
│   │   ├── app_context.rs  # Estado central: channels, señales, CanService, callbacks
│   │   ├── usb_setup.rs    # build_usb_stack(), StaticCells, tipos USB
│   │   └── tasks/
│   │       ├── can.rs      # can_driver_task (actor único CAN, bus-off recovery)
│   │       ├── usb_run.rs  # usb_run (corre UsbDevice, señala USB_DEVICE_READY)
│   │       ├── usb_tx.rs   # usb_tx_task (CAN→host + decodificación OBD2/UDS)
│   │       ├── usb_rx.rs   # usb_rx_task (host→CAN)
│   │       └── health.rs   # health_monitor (IWDG + métricas CAN cada 30s)
│   └── build.rs            # Linker flags (--nmagic)
├── crates/
│   ├── bsp-f446/           # Board Support Package para STM32F446
│   │   └── src/
│   │       ├── lib.rs      # init() → Result<Board, ()>, ResetReason, config RCC/PLL
│   │       ├── usb.rs      # Driver USB OTG FS, bind_interrupts, type alias
│   │       ├── can.rs      # Driver bxCAN, start/stop/set_bit_timing
│   │       └── watchdog.rs # BspWatchdog (IWDG wrapper)
│   ├── gs-usb-protocol/    # Protocolo gs_usb (independiente de hardware)
│   │   └── src/
│   │       ├── lib.rs      # Config USB default (VID/PID), no_std
│   │       ├── gs_usb_types.rs  # Structs C-repr: GsHostFrame, GsTxMsg, GsDeviceMode...
│   │       ├── handler.rs       # GsUsbControlHandler: control_in/control_out
│   │       └── handler_tests.rs # Tests unitarios (corren en host)
│   └── can-protocol/       # Decodificador OBD2/UDS
│       └── src/
│           ├── lib.rs      # CanFrame, analyze_frame(), DecodedProtocol
│           ├── obd2.rs     # parse_obd2_request()
│           └── uds.rs      # parse_uds_message()
├── docs/                   # Documentación técnica
├── imgs/                   # Logos y diagramas
├── .cargo/config.toml      # Target ARM, runner probe-rs, alias test-linux
└── Makefile                # Atajos: flash, build, test-linux, attach
```

## Flujo de Datos

```
Bus CAN físico → bxCAN HW → Embassy CAN Driver → can_driver_task (Actor único)
                                                       ├─ RX OK → CAN_RX_CHANNEL → usb_tx_task → USB EP IN (al host)
                                                       └─ Error → USB_ECHO_CHANNEL → usb_tx_task → USB EP IN (error frame)

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
Fase 5: Confirmación → Espera USB_DEVICE_READY, USB_TX_READY, USB_RX_READY, CAN_READY (5s timeout)
Fase 6: Loop         → Idle 60s (watchdog lo alimenta health_monitor)
```

## Estado Actual

### Completado

- BSP-F446: init con PLL (84MHz sys, 48MHz USB), Result<Board, ()>, ResetReason
- BSP-F446: módulo watchdog (BspWatchdog wrapper sobre IWDG)
- gs_usb protocol: handler de control transfers, structs C-repr/Pod, config USB
- can-protocol: CanFrame, decodificador OBD2 y UDS (sniffer básico)
- Tarea `can_driver_task`: actor único CAN (RX+TX+control con `select`)
- Tarea `usb_tx_task`: envía GsHostFrame por Bulk IN + decodifica OBD2/UDS
- Tarea `usb_rx_task`: lee GsTxMsg por Bulk OUT → CAN_CMD_CHANNEL como Transmit
- Tarea `health_monitor`: alimenta IWDG cada 1s, logea métricas CAN cada 30s
- Canal unificado CAN_CMD_CHANNEL para control + TX (Start/Stop/SetBitTiming/Transmit)
- Error frames SocketCAN: mapeo de errores bxCAN → GsHostFrame con from_bus_error/from_controller_error
- Modos CAN dinámicos: listen-only, loopback, one-shot pasados desde host USB
- Filtros CAN: accept_all configurado en start()
- Recuperación automática de bus-off con backoff exponencial (100–3200ms, máx 5 reintentos)
- Bus-off agotado envía CanControllerError::BusOff al host
- TX con timeout (100ms) y abort de mailboxes
- Interoperabilidad verificada con driver gs_usb del kernel Linux (candump funciona)
- CanService: métricas atómicas (error_count, dropped_rx, bus_off_count)
- Señales de inicio: CAN_READY, USB_TX_READY, USB_RX_READY, USB_DEVICE_READY con timeout 5s
- 68 tests unitarios en gs-usb-protocol + 3 en can-protocol (corren en host)

### Pendiente

- [ ] CAN FD: soporte para tramas de alta velocidad
- [ ] Timestamps embebidos en GsHostFrame (actualmente solo por control transfer)
- [ ] Filtros CAN configurables (siempre accept_all)
- [ ] ISO-TP multi-frame (solo Single Frames decodificados)
- [ ] OBD2 response parsing (solo requests)
- [ ] UDS sub-function/DID parsing parcial
- [ ] LEDs/indicadores de estado (callback `on_identify` existe pero sin GPIO)
- [ ] Persistencia de user_id en flash (solo vive en RAM)
- [ ] Flag GS_CAN_FLAG_OVERFLOW completo por frame (placeholder actual)

## Requisitos

- [Rust](https://www.rust-lang.org/tools/install) con target ARM:
  ```bash
  rustup target add thumbv7em-none-eabihf
  ```
- [probe-rs](https://probe.rs/) para flashear y depurar
- Linux (recomendado) con driver `gs_usb` (incluido en kernel)

## Comandos

```bash
# Compilar para el target ARM
cargo build --release

# Compilar y flashear con probe-rs
cargo run --release
# o bien:
make flash

# Conectar a target ya flasheado (debug)
make attach

# Tests unitarios en host (Linux)
cargo test-linux
# o bien:
make test-linux

# Test de un crate específico
cargo test-linux -p gs-usb-protocol
cargo test-linux -p can-protocol

# Logs en tiempo real (defmt)
DEFMT_LOG=trace cargo run --release
```

## Uso con SocketCAN (Linux)

Una vez flasheado el dongle y conectado a un PC con Linux:

```bash
# Verificar que el dispositivo se detecta
ip link show

# Configurar la interfaz CAN
sudo ip link set can0 type can bitrate 500000 listen-only on
sudo ip link set can0 up

# Escuchar tramas CAN
candump can0

# Filtrar por ID
candump can0,7DF:7FF

# Sniff con decodificación OBD2/UDS (logs del firmware)
DEFMT_LOG=info cargo run --release
```

## Estructura de Crates

| Crate | Propósito | Testeable en Host |
|-------|-----------|-------------------|
| `firmware` | Binario principal, pipeline de 6 fases y orquestación de tareas Embassy | No |
| `bsp-f446` | Abstracción de hardware específica del STM32F446 (USB, CAN, IWDG) | No |
| `gs-usb-protocol` | Protocolo gs_usb, 100% independiente de hardware | Sí (68 tests) |
| `can-protocol` | Lógica de decodificación OBD2/UDS | Sí (3 tests) |

## Licencia

Este proyecto se distribuye bajo la licencia **MIT**. Consulta el archivo `LICENSE` para más detalles.