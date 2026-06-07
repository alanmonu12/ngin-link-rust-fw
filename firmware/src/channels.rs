use defmt::*;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use gs_usb_protocol::gs_usb_types::{
    GsDeviceBitTiming, GsHostFrame, GsTxMsg,
    GS_CAN_FLAG_LOOP_BACK, GS_CAN_FLAG_LISTEN_ONLY, GS_CAN_FLAG_ONE_SHOT,
};

pub static CAN_RX_CHANNEL: Channel<CriticalSectionRawMutex, can_protocol::CanFrame, 32> = Channel::new();
pub static CAN_CMD_CHANNEL: Channel<CriticalSectionRawMutex, CanDriverCmd, 16> = Channel::new();
pub static USB_ECHO_CHANNEL: Channel<CriticalSectionRawMutex, GsHostFrame, 16> = Channel::new();

pub enum CanDriverCmd {
    Start { loopback: bool, listen_only: bool, one_shot: bool },
    Stop,
    SetBitTiming(GsDeviceBitTiming),
    Transmit(CanTxRequest),
}

pub struct CanTxRequest {
    pub echo_id: u32,
    pub id: u32,
    pub is_extended: bool,
    pub is_rtr: bool,
    pub data: [u8; 8],
    pub dlc: u8,
}

pub fn on_start_cb(flags: u32) {
    let loopback = (flags & GS_CAN_FLAG_LOOP_BACK) != 0;
    let listen_only = (flags & GS_CAN_FLAG_LISTEN_ONLY) != 0;
    let one_shot = (flags & GS_CAN_FLAG_ONE_SHOT) != 0;
    let _ = CAN_CMD_CHANNEL.try_send(CanDriverCmd::Start { loopback, listen_only, one_shot });
}

pub fn on_stop_cb() {
    let _ = CAN_CMD_CHANNEL.try_send(CanDriverCmd::Stop);
}

pub fn on_bit_timing_cb(timing: GsDeviceBitTiming) {
    let _ = CAN_CMD_CHANNEL.try_send(CanDriverCmd::SetBitTiming(timing));
}

pub fn on_identify_cb(on: bool) {
    if on {
        info!("IDENTIFY: LED encendido");
    } else {
        info!("IDENTIFY: LED apagado");
    }
}

pub fn now_ms() -> u32 {
    embassy_time::Instant::now().as_millis() as u32
}

pub fn parse_tx_msg(tx_msg: &GsTxMsg) -> CanTxRequest {
    CanTxRequest {
        echo_id: tx_msg.echo_id,
        id: tx_msg.id(),
        is_extended: tx_msg.is_extended(),
        is_rtr: tx_msg.is_rtr(),
        data: tx_msg.data,
        dlc: tx_msg.dlc(),
    }
}