use defmt::info;
use embassy_usb::control::{InResponse, OutResponse, Request, RequestType};
use embassy_usb::Handler;

use crate::gs_usb_types::*;

/// Estructura que maneja los Control Transfers del protocolo gs_usb
pub struct GsUsbControlHandler;

impl Handler for GsUsbControlHandler {
    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        // Solo manejamos peticiones de tipo Vendor
        if req.request_type != RequestType::Vendor {
            return None;
        }

        info!("-> IN Req: {}, Val: {}, Len: {}", req.request, req.value, req.length);

        match req.request {
            // El host pregunta por las constantes de temporización del CAN (Request 4)
            GS_USB_BREQ_BT_CONST => {
                // These values are specific to the bxCAN peripheral in the STM32F4 series.
                let timings = GsDeviceBtConst {
                    feature: GS_CAN_FEATURE_LISTEN_ONLY | GS_CAN_FEATURE_LOOP_BACK,
                    fclk_can: 42_000_000,
                    tseg1_min: 1,
                    tseg1_max: 16,
                    tseg2_min: 1,
                    tseg2_max: 8,
                    sjw_max: 4,
                    brp_min: 1,
                    brp_max: 1024,
                    brp_inc: 1,
                };

                let bytes = bytemuck::bytes_of(&timings);
                let len = core::cmp::min(buf.len(), bytes.len());
                buf[..len].copy_from_slice(&bytes[..len]);
                return Some(InResponse::Accepted(&buf[..len]));
            }

            // El host pregunta por las capacidades del dispositivo (Request 5)
            GS_USB_BREQ_DEVICE_CONFIG => {
                let config = GsDeviceConfig {
                    interface_count: 1, // We have one CAN interface.
                    sw_version: 1,      // Firmware version 1.
                    hw_version: 1,      // Hardware version 1.
                    ..Default::default()
                };

                let bytes = bytemuck::bytes_of(&config);
                let len = core::cmp::min(buf.len(), bytes.len());
                buf[..len].copy_from_slice(&bytes[..len]);
                return Some(InResponse::Accepted(&buf[..len]));
            }

            _ => {
                // Devolvemos None para que el stack USB genere un STALL,
                // indicando que no soportamos esta petición.
                return None;
            }
        }
    }

    fn control_out(&mut self, req: Request, buf: &[u8]) -> Option<OutResponse> {
        // Solo manejamos peticiones de tipo Vendor
        if req.request_type != RequestType::Vendor {
            return None;
        }

        info!("<- OUT Req: {}, Val: {}, Len: {}", req.request, req.value, req.length);

        match req.request {
            GS_USB_BREQ_BITTIMING => {
                if buf.len() >= core::mem::size_of::<GsDeviceBitTiming>() {
                    let timing: GsDeviceBitTiming = bytemuck::pod_read_unaligned(&buf[..core::mem::size_of::<GsDeviceBitTiming>()]);
                    info!(
                        "[USB] Nuevo Bit Timing recibido: brp={}, prop_seg={}, phase1={}, phase2={}, sjw={}",
                        timing.brp, timing.prop_seg, timing.phase_seg1, timing.phase_seg2, timing.sjw
                    );
                }
                return Some(OutResponse::Accepted);
            }
            GS_USB_BREQ_HOST_FORMAT | GS_USB_BREQ_SET_TERMINATION => {
                return Some(OutResponse::Accepted);
            }
            GS_USB_BREQ_MODE => {
                if buf.len() >= 4 {
                    let mode = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                    if (mode & 1) == 1 { // Chequeamos el bit de START/STOP
                        info!("[USB] Comando START recibido");
                    } else {
                        info!("[USB] Comando STOP recibido");
                    }
                }
                return Some(OutResponse::Accepted);
            }
            // Devolvemos None para que el stack USB genere un STALL,
            // indicando que no soportamos esta petición.
            _ => return None,
        }
    }
}