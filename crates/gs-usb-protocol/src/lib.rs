// Aplicamos no_std solo cuando compilamos para el microcontrolador (bare-metal)
#![cfg_attr(target_os = "none", no_std)]

// Declaramos nuestro submódulo (en minúsculas)
pub mod handler;
pub mod gs_usb_types;

use embassy_usb::Config;

// Proveemos la configuración estándar del dispositivo gs_usb
pub fn default_gs_usb_config() -> Config<'static> {
    let mut config_usb = Config::new(0x1d50, 0x606f);
    config_usb.manufacturer = Some("Ngin");
    config_usb.product = Some("Ngin-link PoC");
    config_usb.serial_number = Some("12345678");
    config_usb.max_power = 100;
    config_usb.max_packet_size_0 = 64;
    config_usb
}

// Solo incluimos las pruebas si estamos en la PC (evita errores en Rust-Analyzer)
#[cfg(all(test, not(target_os = "none")))]
mod handler_tests;