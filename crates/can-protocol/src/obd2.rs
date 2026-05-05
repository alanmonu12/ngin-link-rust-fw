/// Modos estándar de OBD-II (Servicios)
#[derive(Debug)]
#[repr(u8)]
pub enum Obd2Mode {
    ShowCurrentData = 0x01,
    ShowFreezeFrame = 0x02,
    ShowStoredDTCs = 0x03,
    ClearDTCs = 0x04,
    Unknown(u8),
}

#[derive(Debug)]
pub struct Obd2Command {
    pub mode: Obd2Mode,
    pub pid: Option<u8>,
}

/// Parsea el payload de un Single Frame para ver si es un request OBD-II
pub fn parse_obd2_request(id: u32, payload: &[u8]) -> Option<Obd2Command> {
    // El request OBD-II de broadcast estándar usa el ID 0x7DF
    if id == 0x7DF && !payload.is_empty() {
        let mode = match payload[0] {
            0x01 => Obd2Mode::ShowCurrentData,
            0x02 => Obd2Mode::ShowFreezeFrame,
            0x03 => Obd2Mode::ShowStoredDTCs,
            0x04 => Obd2Mode::ClearDTCs,
            m => Obd2Mode::Unknown(m),
        };
        let pid = if payload.len() > 1 { Some(payload[1]) } else { None };
        
        return Some(Obd2Command { mode, pid });
    }
    None
}