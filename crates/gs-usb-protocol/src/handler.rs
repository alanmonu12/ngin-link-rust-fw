// Usamos defmt en producción (microcontrolador)
#[cfg(not(test))]
use defmt::info;

// Ignoramos los logs en las pruebas del host para evitar errores de compilación con defmt
#[cfg(test)]
macro_rules! info { ($($arg:tt)*) => {} }

use embassy_usb::control::{InResponse, OutResponse, Request, RequestType};
use embassy_usb::Handler;

use crate::gs_usb_types::*;

/// Longitud en bytes de un timestamp (u32 little-endian) y de un user_id.
const U32_LEN: usize = core::mem::size_of::<u32>();

/// Estructura que maneja los Control Transfers del protocolo gs_usb.
///
/// Cada callback se invoca de forma **sincrónica** desde el contexto del
/// driver USB, por lo que deben ser `fn` puros (no async) y nunca bloquear.
/// Si se necesita notificar a una tarea Embassy, se debe enviar por un
/// `Channel` desde dentro del callback.
pub struct GsUsbControlHandler {
    /// Callback invocado al recibir `GS_USB_BREQ_MODE` con bit START.
    /// El argumento `mode` contiene los flags de modo (LOOPBACK, LISTEN_ONLY, etc.).
    pub on_start: Option<fn(u32)>,
    /// Callback invocado al recibir `GS_USB_BREQ_MODE` con bit STOP.
    pub on_stop: Option<fn()>,
    /// Callback invocado al recibir `GS_USB_BREQ_BITTIMING`.
    pub on_bit_timing: Option<fn(GsDeviceBitTiming)>,
    /// Callback invocado al recibir `GS_USB_BREQ_IDENTIFY`.
    /// El argumento es `true` cuando el LED debe encenderse.
    pub on_identify: Option<fn(bool)>,

    /// Fuente de tiempo para `GS_USB_BREQ_TIMESTAMP`.
    /// Por defecto devuelve 0; el firmware debe setear esto a una función
    /// que devuelva los milisegundos actuales (típicamente
    /// `embassy_time::Instant::now().as_millis() as u32`).
    pub now_ms: fn() -> u32,

    /// ID de usuario reportado por `GS_USB_BREQ_GET_USER_ID` y modificado
    /// por `GS_USB_BREQ_SET_USER_ID`.
    pub user_id: u32,

    /// Capacidades reportadas por `GS_USB_BREQ_DEV_CAPABILITIES`.
    pub capabilities: GsDeviceCapabilities,
}

impl Default for GsUsbControlHandler {
    fn default() -> Self {
        Self {
            on_start: None,
            on_stop: None,
            on_bit_timing: None,
            on_identify: None,
            now_ms: default_now_ms,
            user_id: 0,
            capabilities: GsDeviceCapabilities::default(),
        }
    }
}

/// Devuelve 0 por defecto. El firmware debe reemplazar `now_ms` con su
/// fuente de tiempo real antes de iniciar el USB.
fn default_now_ms() -> u32 {
    0
}

impl GsUsbControlHandler {
    /// Escribe `src` en `buf` (recortado al menor tamaño) y devuelve
    /// `InResponse::Accepted` apuntando a la región copiada.
    ///
    /// Si el buffer del host es más pequeño que la respuesta, copiamos
    /// lo que cabe en vez de hacer STALL: esto evita desconexiones
    /// innecesarias y permite que el host reciba al menos un valor
    /// parcial (importante para TIMESTAMP y USER_ID).
    fn write_in<'a>(buf: &'a mut [u8], src: &[u8]) -> InResponse<'a> {
        let len = core::cmp::min(buf.len(), src.len());
        buf[..len].copy_from_slice(&src[..len]);
        InResponse::Accepted(&buf[..len])
    }
}

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
                    feature: self.bt_const_feature(),
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
                Some(Self::write_in(buf, bytemuck::bytes_of(&timings)))
            }

            // El host pregunta por la configuración del dispositivo (Request 5)
            GS_USB_BREQ_DEVICE_CONFIG => {
                // icount=0 significa 1 interfaz CAN (el kernel hace icount+1).
                // icount=1 crearía 2 interfaces (can0 y can1), lo cual es incorrecto.
                let config = GsDeviceConfig {
                    interface_count: 0,
                    sw_version: 2,
                    hw_version: 1,
                    ..Default::default()
                };
                let bytes = bytemuck::bytes_of(&config);
                info!(
                    "[USB] DEVICE_CONFIG: interface_count={} sw_version={} hw_version={} bytes={=[u8]:#X}",
                    config.interface_count, config.sw_version, config.hw_version, bytes
                );
                Some(Self::write_in(buf, bytes))
            }

            // El host pregunta el timestamp actual del dispositivo (Request 6)
            // Devuelve un u32 little-endian. Si el buffer es muy pequeño se
            // copia lo que cabe para que el host reciba al menos un valor
            // parcial en vez de un STALL.
            GS_USB_BREQ_TIMESTAMP => {
                let ts = (self.now_ms)();
                Some(Self::write_in(buf, &ts.to_le_bytes()))
            }

            // El host pregunta el user_id configurado (Request 8)
            GS_USB_BREQ_GET_USER_ID => {
                Some(Self::write_in(buf, &self.user_id.to_le_bytes()))
            }

            // El host pregunta las capacidades del dispositivo (Request 11)
            // Endpoint no estándar (no usado por el driver gs_usb del kernel).
            GS_USB_BREQ_DEV_CAPABILITIES => {
                Some(Self::write_in(buf, bytemuck::bytes_of(&self.capabilities)))
            }

            // Devolvemos None para que el stack USB genere un STALL,
            // indicando que no soportamos esta petición.
            _ => None,
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
                    let timing: GsDeviceBitTiming =
                        bytemuck::pod_read_unaligned(&buf[..core::mem::size_of::<GsDeviceBitTiming>()]);
                    info!(
                        "[USB] Nuevo Bit Timing recibido: brp={}, prop_seg={}, phase1={}, phase2={}, sjw={}",
                        timing.brp, timing.prop_seg, timing.phase_seg1, timing.phase_seg2, timing.sjw
                    );
                    if let Some(cb) = self.on_bit_timing {
                        cb(timing);
                    }
                }
                Some(OutResponse::Accepted)
            }
            GS_USB_BREQ_HOST_FORMAT => {
                if buf.len() >= U32_LEN {
                    let host_fmt = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                    info!("[USB] HOST_FORMAT recibido: version={}", host_fmt);
                }
                Some(OutResponse::Accepted)
            }
            GS_USB_BREQ_SET_TERMINATION => Some(OutResponse::Accepted),
            GS_USB_BREQ_MODE => {
                if buf.len() >= core::mem::size_of::<GsDeviceMode>() {
                    let dev_mode: GsDeviceMode = bytemuck::pod_read_unaligned(&buf[..core::mem::size_of::<GsDeviceMode>()]);
                    if dev_mode.mode == 1 {
                        info!("[USB] Comando START recibido, flags=0x{:08X}", dev_mode.flags);
                        if let Some(cb) = self.on_start {
                            cb(dev_mode.flags);
                        }
                    } else {
                        info!("[USB] Comando STOP recibido");
                        if let Some(cb) = self.on_stop {
                            cb();
                        }
                    }
                }
                Some(OutResponse::Accepted)
            }
            // El host enciende/apaga el LED de identificación (Request 7)
            // Solo se invoca el callback si el feature IDENTIFY está activo.
            GS_USB_BREQ_IDENTIFY => {
                if buf.len() >= U32_LEN {
                    let mode = GsIdentifyMode {
                        mode: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
                    };
                    info!("[USB] IDENTIFY mode={}", mode.mode);
                    if self.capabilities.supports(GS_CAN_FEATURE_IDENTIFY) {
                        if let Some(cb) = self.on_identify {
                            cb(mode.is_on());
                        }
                    }
                }
                Some(OutResponse::Accepted)
            }
            // El host configura el user_id (Request 9)
            // Se actualiza el campo `user_id` siempre; el callback permite
            // al firmware persistir el valor (ej: en flash) si lo necesita.
            GS_USB_BREQ_SET_USER_ID => {
                if buf.len() >= U32_LEN {
                    self.user_id = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                    info!("[USB] SET_USER_ID -> 0x{:08X}", self.user_id);
                }
                Some(OutResponse::Accepted)
            }
            // Devolvemos None para que el stack USB genere un STALL,
            // indicando que no soportamos esta petición.
            _ => None,
        }
    }
}

impl GsUsbControlHandler {
    /// Construye el bitfield de features reportado por `GS_USB_BREQ_BT_CONST`.
    ///
    /// Por defecto: listen-only y loop-back (lo que el hardware bxCAN
    /// soporta nativamente). El firmware puede sobreescribir `capabilities`
    /// para ampliar el set de features.
    fn bt_const_feature(&self) -> u32 {
        self.capabilities.feature
    }
}