#![no_std]

pub mod obd2;
pub mod uds;

/// Representación de una trama CAN clásica (estándar o extendida)
#[derive(Debug, Clone)]
pub struct CanFrame {
    pub id: u32,
    pub is_extended: bool,
    pub data: [u8; 8],
    pub dlc: u8,
}

/// Diferentes protocolos que nuestro firmware puede reconocer al vuelo
#[derive(Debug)]
pub enum DecodedProtocol {
    Obd2Request(obd2::Obd2Command),
    UdsMessage(uds::UdsMessage),
    Raw, // No reconocido o no es de diagnóstico
}

/// Función principal de análisis (Sniffer).
/// Toma una trama CAN cruda y evalúa si coincide con patrones conocidos.
pub fn analyze_frame(frame: &CanFrame) -> DecodedProtocol {
    // Filtramos rápidamente: El diagnóstico moderno suele usar IDs estándar
    // entre 0x700 y 0x7FF (Peticiones OBD/UDS suelen ser 0x7DF o 0x7E0..0x7E7)
    if !frame.is_extended && frame.id >= 0x700 && frame.id <= 0x7FF {
        
        // Verificamos si es un Single Frame (SF) de ISO-TP.
        // En ISO-TP, un Single Frame empieza con un byte donde los 4 bits más 
        // significativos son 0 (PCI = 0x0) y los 4 menos significativos son la longitud.
        let pci_byte = frame.data[0];
        let frame_type = pci_byte >> 4;
        let payload_len = (pci_byte & 0x0F) as usize;

        if frame_type == 0x00 && payload_len > 0 && payload_len <= 7 {
            let payload = &frame.data[1..=payload_len];
            
            // Intentamos decodificar como OBD-II clásico primero
            if let Some(obd_cmd) = obd2::parse_obd2_request(frame.id, payload) {
                return DecodedProtocol::Obd2Request(obd_cmd);
            }

            // Si no es OBD-II, intentamos decodificar como UDS
            if let Some(uds_msg) = uds::parse_uds_message(frame.id, payload) {
                return DecodedProtocol::UdsMessage(uds_msg);
            }
        }
    }

    // Si es un ID de 29-bits (Heavy Duty/J1939) o no coincide, lo dejamos crudo.
    DecodedProtocol::Raw
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obd2::Obd2Mode;
    use crate::uds::UdsService;

    #[test]
    fn test_analyze_obd2_request() {
        // Simulamos un Single Frame de ISO-TP (PCI = 0, longitud = 2)
        // Payload: [0x01 (ShowCurrentData), 0x0C (PID: RPM engine)]
        let frame = CanFrame {
            id: 0x7DF,
            is_extended: false,
            data: [0x02, 0x01, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00],
            dlc: 8,
        };
        
        let result = analyze_frame(&frame);
        match result {
            DecodedProtocol::Obd2Request(cmd) => {
                assert!(matches!(cmd.mode, Obd2Mode::ShowCurrentData));
                assert_eq!(cmd.pid, Some(0x0C));
            },
            _ => panic!("Se esperaba Obd2Request"),
        }
    }

    #[test]
    fn test_analyze_uds_request() {
        // Simulamos un request a la ECU del motor (0x7E0)
        // Payload: [0x10 (DiagnosticSessionControl), 0x01 (Default Session)]
        let frame = CanFrame {
            id: 0x7E0,
            is_extended: false,
            data: [0x02, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00],
            dlc: 8,
        };
        
        let result = analyze_frame(&frame);
        match result {
            DecodedProtocol::UdsMessage(msg) => {
                assert!(!msg.is_response);
                assert!(matches!(msg.service, UdsService::DiagnosticSessionControl));
            },
            _ => panic!("Se esperaba UdsMessage"),
        }
    }

    #[test]
    fn test_analyze_raw_frame() {
        // Un ID de broadcast común, no de diagnóstico
        let frame = CanFrame {
            id: 0x123,
            is_extended: false,
            data: [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
            dlc: 8,
        };
        
        let result = analyze_frame(&frame);
        assert!(matches!(result, DecodedProtocol::Raw));
    }
}