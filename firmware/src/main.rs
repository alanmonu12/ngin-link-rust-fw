#![no_std]
#![no_main]

mod app_context;
mod tasks;
mod usb_setup;

use defmt::*;
use embassy_executor::Spawner;
use embassy_time::{Duration, with_timeout};
use gs_usb_protocol::default_gs_usb_config;
use usb_setup::UsbStack;

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // ── Fase 1: Hardware ──────────────────────────────────────────────
    let board = match bsp_f446::init() {
        Ok(b) => b,
        Err(_) => {
            error!("BSP: Fallo crítico en inicialización. Reseteando...");
            cortex_m::asm::delay(1_000_000);
            cortex_m::peripheral::SCB::sys_reset();
        }
    };
    info!("BSP: Hardware OK. Reset: {:?}", board.reset_reason);

    // ── Fase 2: Stack USB ────────────────────────────────────────────
    let UsbStack { device, ep_in, ep_out } = usb_setup::build_usb_stack(
        board.usb_driver,
        default_gs_usb_config(),
        app_context::on_start_cb,
        app_context::on_stop_cb,
        app_context::on_bit_timing_cb,
        app_context::on_identify_cb,
        app_context::now_ms,
    );
    trace!("USB: Stack construido");

    // ── Fase 3: Watchdog ─────────────────────────────────────────────
    let wdg = bsp_f446::watchdog::BspWatchdog::new(board.iwdg, 5000);
    trace!("IWDG: Habilitado, timeout 5s");

    // ── Fase 4: Spawn de tareas ──────────────────────────────────────
    spawner.spawn(tasks::usb_run(device).unwrap());
    spawner.spawn(tasks::can_driver_task(board.can_driver).unwrap());
    spawner.spawn(tasks::usb_tx_task(ep_in).unwrap());
    spawner.spawn(tasks::usb_rx_task(ep_out).unwrap());
    spawner.spawn(tasks::health_monitor(wdg).unwrap());

    // ── Fase 5: Confirmación de inicio ──────────────────────────────
    let mut all_ok = true;

    match with_timeout(Duration::from_secs(5), app_context::USB_DEVICE_READY.wait()).await {
        Ok(()) => info!("  ✓ USB device"),
        Err(_) => { error!("  ✗ USB device (timeout)"); all_ok = false; }
    }
    match with_timeout(Duration::from_secs(5), app_context::USB_TX_READY.wait()).await {
        Ok(()) => info!("  ✓ USB TX"),
        Err(_) => { error!("  ✗ USB TX (timeout)"); all_ok = false; }
    }
    match with_timeout(Duration::from_secs(5), app_context::USB_RX_READY.wait()).await {
        Ok(()) => info!("  ✓ USB RX"),
        Err(_) => { error!("  ✗ USB RX (timeout)"); all_ok = false; }
    }
    match with_timeout(Duration::from_secs(5), app_context::CAN_READY.wait()).await {
        Ok(()) => info!("  ✓ CAN (esperando START del host)"),
        Err(_) => { error!("  ✗ CAN (timeout)"); all_ok = false; }
    }

    if all_ok {
        info!("[MAIN] Sistema listo — todas las tareas confirmaron inicio");
    } else {
        warn!("[MAIN] Sistema con funciones degradadas — ver logs anteriores");
    }

    // ── Fase 6: Loop principal (el watchdog lo alimenta health_monitor) ──
    loop {
        embassy_time::Timer::after_secs(60).await;
    }
}