use embassy_usb::control::{InResponse, OutResponse, Request, RequestType};
use embassy_usb::Handler;

// Lo hacemos 'pub' para que mod.rs pueda verlo
pub struct GsUsbControlHandler;

impl Handler for GsUsbControlHandler {
    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        defmt::info!("-> IN Req: {}, Val: {}, Len: {}", req.request, req.value, req.length);
        
        if req.request_type == RequestType::Vendor {
            match req.request {
                5 => { // DEVICE_CONFIG
                    let config_data = [0u8, 0, 0, 0,  1, 0, 0, 0,  1, 0, 0, 0];
                    let len = core::cmp::min(buf.len(), config_data.len());
                    buf[..len].copy_from_slice(&config_data[..len]);
                    return Some(InResponse::Accepted(&buf[..len]));
                }
                4 => { // BT_CONST
                    let mut bt_const = [0u8; 40];
                    let feature: u32 = 0;               
                    let fclk_can: u32 = 42_000_000;     
                    let tseg1_min: u32 = 1;             
                    let tseg1_max: u32 = 16;
                    let tseg2_min: u32 = 1;
                    let tseg2_max: u32 = 8;
                    let sjw_max: u32 = 4;
                    let brp_min: u32 = 1;
                    let brp_max: u32 = 1024;
                    let brp_inc: u32 = 1;

                    bt_const[0..4].copy_from_slice(&feature.to_le_bytes());
                    bt_const[4..8].copy_from_slice(&fclk_can.to_le_bytes());
                    bt_const[8..12].copy_from_slice(&tseg1_min.to_le_bytes());
                    bt_const[12..16].copy_from_slice(&tseg1_max.to_le_bytes());
                    bt_const[16..20].copy_from_slice(&tseg2_min.to_le_bytes());
                    bt_const[20..24].copy_from_slice(&tseg2_max.to_le_bytes());
                    bt_const[24..28].copy_from_slice(&sjw_max.to_le_bytes());
                    bt_const[28..32].copy_from_slice(&brp_min.to_le_bytes());
                    bt_const[32..36].copy_from_slice(&brp_max.to_le_bytes());
                    bt_const[36..40].copy_from_slice(&brp_inc.to_le_bytes());

                    let len = core::cmp::min(buf.len(), bt_const.len());
                    buf[..len].copy_from_slice(&bt_const[..len]);
                    return Some(InResponse::Accepted(&buf[..len]));
                }
                _ => return Some(InResponse::Rejected),
            }
        }
        None
    }

    fn control_out(&mut self, req: Request, buf: &[u8]) -> Option<OutResponse> {
        defmt::info!("<- OUT Req: {}, Val: {}, Len: {}", req.request, req.value, req.length);
        
        if req.request_type == RequestType::Vendor {
            match req.request {
                0 => return Some(OutResponse::Accepted), // HOST_FORMAT
                1 => return Some(OutResponse::Accepted), // BITTIMING
                2 => { // MODE
                    if buf.len() >= 4 {
                        let mode = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                        if mode == 1 {
                            defmt::info!("[USB] Comando START recibido");
                        } else if mode == 0 {
                            defmt::info!("[USB] Comando STOP recibido");
                        }
                    }
                    return Some(OutResponse::Accepted);
                }
                _ => return Some(OutResponse::Rejected),
            }
        }
        None
    }
}

pub mod Handler;
