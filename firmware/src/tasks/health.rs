use defmt::*;
use bsp_f446::watchdog::BspWatchdog;
use crate::app_context::CAN_SERVICE;

#[embassy_executor::task]
pub async fn health_monitor(mut wdg: BspWatchdog) {
    info!("HEALTH: Monitor iniciado, watchdog alimentado cada 1s");
    let mut secs: u32 = 0;

    loop {
        wdg.pet();

        if secs % 30 == 0 {
            let started = CAN_SERVICE.is_started();
            let errors = CAN_SERVICE.error_count.load(portable_atomic::Ordering::Relaxed);
            let dropped = CAN_SERVICE.dropped_rx.load(portable_atomic::Ordering::Relaxed);
            let bus_off = CAN_SERVICE.bus_off_count.load(portable_atomic::Ordering::Relaxed);
            info!(
                "HEALTH: CAN {} | errores={} perdidos={} bus_off={}",
                if started { "ACTIVO" } else { "DETENIDO" },
                errors, dropped, bus_off
            );
        }

        secs += 1;
        embassy_time::Timer::after_secs(1).await;
    }
}