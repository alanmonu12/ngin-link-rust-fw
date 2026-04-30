# Ngin Link Rust Firmware - AI Agent Instructions

## Project Context
This repository contains the firmware for a USB-to-CAN dongle. The tool is designed for both educational purposes (learning how CAN bus communications work) and professional use in the repair and development of aftermarket Engine Control Units (ECUs).

This tool is specifically intended to be used with production vehicles, such as in chiptuning applications alongside tools like Kess or Flex. It is designed to allow debugging of these tools when they fail, as well as serving as an educational platform to teach how these professional tools interact with vehicle systems.

## Hardware Base
- **Microcontroller:** STM32F446

## Technology Stack
- **Language:** Rust
- **Framework:** Embassy (Async embedded Rust)
- **Core Interfaces:** USB (Device) and CAN (Controller Area Network)
- **USB Protocol:** gs_usb (for seamless compatibility with Linux and SocketCAN)

## Engineering Guidelines for AI Agents
1. **Idiomatic Rust & Embassy:** Always write idiomatic, safe Rust code. Strictly adhere to Embassy's async patterns and best practices for embedded systems. Avoid blocking operations; use `async`/`await` and Embassy's synchronization primitives.
2. **Embedded Constraints (`#![no_std]`):** This is a bare-metal embedded project. Ensure all code is `#![no_std]` compatible. Avoid dynamic allocation (`alloc`) unless explicitly configured and necessary.
3. **Safety & Robustness:** Since this tool interfaces with automotive CAN buses, ensure robust error handling. Do not use `unwrap()` or `expect()` in production paths; propagate errors gracefully. Ensure safe handling of hardware peripherals.
4. **Educational Focus:** Code should be clean, well-commented, and easy to understand, as this tool is intended for teaching. Use clear variable names and provide brief explanatory comments for complex CAN or USB protocol interactions.
5. **Testing & Validation:** Where possible, write `#[test]` unit tests for logic that does not depend directly on hardware peripherals (e.g., parsing CAN frames, state machines).

## Architectural Notes
- The system primarily acts as a bridge: receiving CAN frames and sending them over USB to a host PC, and vice-versa.
- Performance and low latency are important for sniffing busy CAN buses.
