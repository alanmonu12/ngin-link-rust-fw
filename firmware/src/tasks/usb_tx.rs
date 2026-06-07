use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_usb::driver::{Endpoint, EndpointIn};
use gs_usb_protocol::gs_usb_types::GsHostFrame;
use crate::channels::{CAN_RX_CHANNEL, USB_ECHO_CHANNEL};

#[embassy_executor::task]
pub async fn usb_tx_task(mut ep_in: bsp_f446::usb::BspUsbEndpointIn) {
    ep_in.wait_enabled().await;
    info!("USB TX: Endpoint IN habilitado, listo para enviar tramas al host");

    loop {
        let host_frame = match select(CAN_RX_CHANNEL.receive(), USB_ECHO_CHANNEL.receive()).await {
            Either::First(frame) => {
                match can_protocol::analyze_frame(&frame) {
                    can_protocol::DecodedProtocol::Obd2Request(cmd) => {
                        defmt::info!("OBD2: {:?}", defmt::Debug2Format(&cmd));
                    }
                    can_protocol::DecodedProtocol::UdsMessage(msg) => {
                        defmt::info!("UDS: {:?}", defmt::Debug2Format(&msg));
                    }
                    can_protocol::DecodedProtocol::Raw => {}
                }

                GsHostFrame::from_can_frame(frame.id, frame.is_extended, frame.dlc, &frame.data)
            }
            Either::Second(echo_frame) => echo_frame,
        };

        let bytes = bytemuck::bytes_of(&host_frame);
        info!("USB TX: can_id=0x{:08X} dlc={} flags=0x{:02X} echo_id={}", host_frame.can_id, host_frame.can_dlc, host_frame.flags, host_frame.echo_id);

        match ep_in.write(bytes).await {
            Ok(n) => info!("USB TX OK: {} bytes enviados al host", n),
            Err(e) => error!("USB TX: Error al escribir al endpoint: {:?}", defmt::Debug2Format(&e)),
        }
    }
}