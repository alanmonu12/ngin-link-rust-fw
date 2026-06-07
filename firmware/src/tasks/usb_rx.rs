use defmt::*;
use embassy_usb::driver::{Endpoint, EndpointOut};
use gs_usb_protocol::gs_usb_types::GsTxMsg;
use crate::channels::{CAN_CMD_CHANNEL, CanDriverCmd};

#[embassy_executor::task]
pub async fn usb_rx_task(mut ep_out: bsp_f446::usb::BspUsbEndpointOut) {
    ep_out.wait_enabled().await;
    info!("USB RX: Endpoint OUT habilitado, listo para recibir tramas del host");

    let mut buf = [0u8; 64];
    loop {
        match ep_out.read(&mut buf).await {
            Ok(n) if n >= core::mem::size_of::<GsTxMsg>() => {
                let tx_msg: GsTxMsg = bytemuck::pod_read_unaligned(&buf[..core::mem::size_of::<GsTxMsg>()]);
                info!("USB RX: TX request id=0x{:03X}, ext={}, dlc={}, echo_id={}", 
                    tx_msg.id(), tx_msg.is_extended(), tx_msg.dlc(), tx_msg.echo_id);
                let tx_req = crate::channels::parse_tx_msg(&tx_msg);

                match CAN_CMD_CHANNEL.try_send(CanDriverCmd::Transmit(tx_req)) {
                    Ok(()) => {}
                    Err(_) => {
                        warn!("USB RX: Cola CAN CMD llena, descartando trama");
                    }
                }
            }
            Ok(n) => {
                warn!("USB RX: Trama incompleta ({} bytes, esperados {})", n, core::mem::size_of::<GsTxMsg>());
            }
            Err(e) => {
                error!("USB RX: Error al leer del endpoint: {:?}", defmt::Debug2Format(&e));
            }
        }
    }
}