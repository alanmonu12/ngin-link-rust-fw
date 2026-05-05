/// Servicios principales de UDS (ISO 14229)
#[derive(Debug)]
#[repr(u8)]
pub enum UdsService {
    DiagnosticSessionControl = 0x10,
    EcuReset = 0x11,
    SecurityAccess = 0x27,
    CommunicationControl = 0x28,
    TesterPresent = 0x3E,
    ReadDataByIdentifier = 0x22,
    WriteDataByIdentifier = 0x2E,
    RoutineControl = 0x31,
    RequestDownload = 0x34,
    TransferData = 0x36,
    RequestTransferExit = 0x37,
    Unknown(u8),
}

#[derive(Debug)]
pub struct UdsMessage {
    pub is_response: bool,
    pub service: UdsService,
    pub sub_function_or_did: Option<u16>,
}

/// Intenta identificar si el payload es un mensaje UDS válido
pub fn parse_uds_message(id: u32, payload: &[u8]) -> Option<UdsMessage> {
    // Excluimos 0x7DF porque es específico de broadcast OBD2
    if id == 0x7DF || payload.is_empty() {
        return None;
    }

    let sid = payload[0];
    
    // Las respuestas exitosas en UDS suman 0x40 al ID del servicio
    let is_response = sid >= 0x50;
    let base_sid = if is_response { sid - 0x40 } else { sid };

    let service = match base_sid {
        0x10 => UdsService::DiagnosticSessionControl,
        0x22 => UdsService::ReadDataByIdentifier,
        0x27 => UdsService::SecurityAccess,
        0x3E => UdsService::TesterPresent,
        0x34 => UdsService::RequestDownload,
        s => UdsService::Unknown(s),
    };

    // Para este POC, no extraeremos los DIDs complejos aún
    Some(UdsMessage { is_response, service, sub_function_or_did: None })
}