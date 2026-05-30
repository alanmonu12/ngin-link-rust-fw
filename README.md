# Ngin-Link: USB-to-CAN Dongle Firmware (STM32F4 + Rust)

![Language](https://img.shields.io/badge/Language-Rust-orange.svg)
![Architecture](https://img.shields.io/badge/Arch-ARM%20Cortex--M4-blue)
![Framework](https://img.shields.io/badge/Framework-Embassy-green)
![Status](https://img.shields.io/badge/Status-WIP-yellow)
![License](https://img.shields.io/badge/License-MIT-brightgreen)

Dongle USB-CAN open-source basado en el protocolo [gs_usb](https://github.com/torvalds/linux/blob/master/drivers/net/can/usb/gs_usb.c), compatible con **SocketCAN** en Linux. Diseñado para sniffing y diagnóstico de buses CAN en vehículos (chiptuning, debugging de herramientas como Kess/Flex).

## Hardware

- **Microcontrolador:** STM32F446RE (ARM Cortex-M4, 84 MHz)
- **USB:** OTG FS (PA11=DM, PA12=DP)
- **CAN:** bxCAN1 (PB8=RX, PB9=TX)
- **Cristal externo:** 8 MHz (modo bypass, típico en placas Nucleo)
- **Compatibilidad:** SocketCAN en Linux (driver `gs_usb` en kernel)

## Stack Tecnológico

| Componente | Tecnología |
|------------|------------|
| Lenguaje | Rust (`#![no_std]`, bare-metal) |
| Framework | Embassy 0.6 (async embedded) |
| Debug/Flash | probe-rs |
| Protocolo USB | gs_usb (VID: 0x1d50, PID: 0x606f) |
| Logs | defmt (RTT) |

## Arquitectura del Workspace

```
ngin-link-rust-fw/
├── Cargo.toml                  # Workspace root
├── firmware/                   # Binario principal
│   └── src/main.rs             # Entry point, tareas Embassy, ruteo CAN↔USB
├── crates/
│   ├── bsp-f446/               # Board Support Package (HAL ajustada)
│   │   └── src/
│   │       ├── lib.rs          # init() → Board, config RCC/PLL
│   │       ├── usb.rs          # Driver USB OTG FS
│   │       └── can.rs          # Driver bxCAN
│   ├── gs-usb-protocol/        # Protocolo gs_usb (independiente de HW)
│   │   └── src/
│   │       ├── lib.rs          # Config USB default (VID/PID)
│   │       ├── gs_usb_types.rs # Structs C-repr del protocolo
│   │       ├── handler.rs      # Control handler (control_in/control_out)
│   │       └── handler_tests.rs # Tests unitarios (corren en host)
│   └── can-protocol/           # Decodificador OBD2/UDS
│       └── src/
│           ├── lib.rs          # CanFrame, analyze_frame()
│           ├── obd2.rs         # Parser OBD-II
│           └── uds.rs          # Parser UDS (ISO 14229)
└── .cargo/config.toml          # Target ARM, runner probe-rs
```

## Flujo de Datos

```
Bus CAN → bxCAN HW → Embassy CAN Driver → can_rx_task
                                              ↓
                                      CAN_RX_CHANNEL (32 msgs)
                                              ↓
                                      usb_tx_task → [TODO: USB EP IN → Host]
```

**Control (Host → Device):**
```
Host USB → gs_usb control transfer → GsUsbControlHandler
  ├── BITTIMING → Configura velocidad CAN
  ├── MODE START → Inicia escucha del bus
  └── MODE STOP → Detiene el controlador
```

## Estado Actual

### Completado
- Workspace con arquitectura modular (4 crates)
- BSP-F446: PLL configurado (84 MHz sys, 48 MHz USB)
- gs_usb: Handler de control transfers, tipos C-repr, config USB
- can-protocol: Decodificador OBD2 y UDS (sniffer en tiempo real)
- Tarea `can_rx_task`: Recibe tramas CAN, las decodifica, envía por canal
- Canal de control USB→CAN para start/stop/bit_timing
- Tests unitarios para gs_usb y can-protocol (corren en host)

### Pendiente
- [ ] `usb_tx_task`: Enviar tramas al host por USB Bulk IN
- [ ] USB Bulk OUT: Recibir comandos de transmisión desde el host
- [ ] Transmisión CAN: Enviar tramas CAN desde el host
- [ ] CAN FD: Soporte para tramas de alta velocidad
- [ ] Timestamps en tramas USB
- [ ] Filtros CAN configurables
- [ ] Manejo robusto de errores USB
- [ ] LEDs/indicadores de estado
- [ ] Watchdog

## Requisitos

- [Rust](https://www.rust-lang.org/tools/install) con target ARM:
  ```bash
  rustup target add thumbv7em-none-eabihf
  ```
- [probe-rs](https://probe.rs/) para flashear y depurar
- Linux (recomendado) con driver `gs_usb` (incluido en kernel)

## Comandos

```bash
# Compilar
cargo build --release

# Flashear con probe-rs
cargo run --release

# Tests unitarios en host
cargo test --target x86_64-unknown-linux-gnu
cargo test --target aarch64-apple-darwin  # macOS

# Test de un crate específico
cargo test --target x86_64-unknown-linux-gnu -p gs-usb-protocol
cargo test --target x86_64-unknown-linux-gnu -p can-protocol

# Logs en tiempo real
DEFMT_LOG=trace cargo run --release
```

## Uso con SocketCAN (Linux)

Una vez flasheado el dongle y conectado a un PC con Linux:

```bash
# Verificar que el dispositivo se detecta
ip link show

# Configurar la interfaz CAN
sudo ip link set can0 type can bitrate 500000
sudo ip link set can0 up

# Escuchar tramas CAN
candump can0

# Filtrar por ID
candump can0,7DF:7FF

# Sniff con decodificación OBD2/UDS (logs del firmware)
DEFMT_LOG=info cargo run --release
```

## Estructura de Crates

| Crate | Propiedad | Testeable en Host |
|-------|-----------|-------------------|
| `firmware` | Binario principal, orquesta tareas Embassy | No |
| `bsp-f446` | Abstracción de hardware específica del STM32F446 | No |
| `gs-usb-protocol` | Protocolo gs_usb, 100% independiente de HW | Sí |
| `can-protocol` | Lógica de decodificación OBD2/UDS | Sí |

## Licencia

Este proyecto se distribuye bajo la licencia **MIT**. Consulta el archivo `LICENSE` para más detalles.
