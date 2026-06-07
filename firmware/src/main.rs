#![no_std]
#![no_main]

mod channels;
mod tasks;
mod usb_setup;

use defmt::*;
use embassy_executor::Spawner;
use gs_usb_protocol::default_gs_usb_config;
use usb_setup::UsbStack;

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let board = bsp_f446::init();
    info!("Hardware y relojes configurados. Iniciando driver USB...");

    let UsbStack { device, ep_in, ep_out } = usb_setup::build_usb_stack(
        board.usb_driver,
        default_gs_usb_config(),
        channels::on_start_cb,
        channels::on_stop_cb,
        channels::on_bit_timing_cb,
        channels::on_identify_cb,
        channels::now_ms,
    );

    spawner.spawn(tasks::usb_run(device).unwrap());
    spawner.spawn(tasks::can_driver_task(board.can_driver).unwrap());
    spawner.spawn(tasks::usb_tx_task(ep_in).unwrap());
    spawner.spawn(tasks::usb_rx_task(ep_out).unwrap());

    info!("¡Sistema configurado y listo!");

    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}