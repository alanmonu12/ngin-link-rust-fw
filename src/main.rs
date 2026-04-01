#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

// La macro #[embassy_executor::main] configura el entorno asíncrono por ti
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Inicializa los periféricos con la configuración de reloj por defecto
    let p = embassy_stm32::init(Default::default());
    
    info!("¡Sistema iniciado! Arrancando el PoC del adaptador CAN.");

    // Configura el pin PA5 como salida. 
    // (PA5 es donde está el LED integrado "LD2" en casi todas las placas Nucleo-64)
    let mut led = Output::new(p.PA5, Level::High, Speed::Low);

    loop {
        info!("LED Encendido");
        led.set_high();
        
        // Timer asíncrono: El CPU puede hacer otras cosas o dormir durante estos 500ms
        Timer::after(Duration::from_millis(500)).await;
        
        info!("LED Apagado");
        led.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}