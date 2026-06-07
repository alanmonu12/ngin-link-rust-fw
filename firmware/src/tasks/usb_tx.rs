use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_usb::driver::{Endpoint, EndpointIn};
use gs_usb_protocol::gs_usb_types::GsHostFrame;
use crate::app_context::{CAN_RX_CHANNEL, USB_ECHO_CHANNEL, USB_TX_READY};

#[embassy_executor::task]
pub async fn usb_tx_task(mut ep_in: bsp_f446::usb::BspUsbEndpointIn) {
    ep_in.wait_enabled().await;
    USB_TX_READY.signal(());
    trace!("USB TX: Endpoint IN habilitado");

    loop {
        let host_frame = match select(CAN_RX_CHANNEL.receive(), USB_ECHO_CHANNEL.receive()).await {
            Either::First(frame) => {
                match can_protocol::analyze_frame(&frame) {
                    can_protocol::DecodedProtocol::Obd2Request(cmd) => {
                        trace!("OBD2: {:?}", defmt::Debug2Format(&cmd));
                    }
                    can_protocol::DecodedProtocol::UdsMessage(msg) => {
                        trace!("UDS: {:?}", defmt::Debug2Format(&msg));
                    }
                    can_protocol::DecodedProtocol::Raw => {}
                }

                GsHostFrame::from_can_frame(frame.id, frame.is_extended, frame.dlc, &frame.data)
            }
            Either::Second(echo_frame) => echo_frame,
        };

        let bytes = bytemuck::bytes_of(&host_frame);
        trace!("USB TX: can_id=0x{:08X} dlc={} flags=0x{:02X} echo_id={}", host_frame.can_id, host_frame.can_dlc, host_frame.flags, host_frame.echo_id);

        match ep_in.write(bytes).await {
            Ok(_) => {}
            Err(e) => error!("USB TX: Error al escribir al endpoint: {:?}", defmt::Debug2Format(&e)),
        }
    }
}