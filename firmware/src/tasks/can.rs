use defmt::*;
use embassy_futures::select::{select, Either};
use gs_usb_protocol::gs_usb_types::{GsHostFrame, CanBusError};
use crate::channels::{CAN_CMD_CHANNEL, CAN_RX_CHANNEL, USB_ECHO_CHANNEL, CanDriverCmd};

#[embassy_executor::task]
pub async fn can_driver_task(mut can: bsp_f446::can::BspCan) {
    let mut is_started = false;
    let mut can_error_count: u32 = 0;

    loop {
        if is_started {
            match select(CAN_CMD_CHANNEL.receive(), can.can.read()).await {
                Either::First(cmd) => {
                    match cmd {
                        CanDriverCmd::Stop => {
                            info!("CAN: Apagando controlador");
                            is_started = false;
                            can.stop().await;
                        }
                        CanDriverCmd::Start { loopback, listen_only, one_shot } => {
                            info!("CAN: Re-aplicando modo (loopback={}, listen_only={}, one_shot={})", loopback, listen_only, one_shot);
                            can.start(loopback, listen_only, one_shot).await;
                        }
                        CanDriverCmd::SetBitTiming(timing) => {
                            info!("CAN: Reconfigurando Bit Timing en caliente: brp={}, prop={}, phase1={}, phase2={}, sjw={}",
                                timing.brp, timing.prop_seg, timing.phase_seg1, timing.phase_seg2, timing.sjw);
                            can.set_bit_timing(&timing);
                            can.reenable().await;
                        }
                        CanDriverCmd::Transmit(tx_req) => {
                            info!("CAN TX: Enviando id=0x{:03X} ext={} dlc={}", tx_req.id, tx_req.is_extended, tx_req.dlc);
                            let data_slice = &tx_req.data[..tx_req.dlc as usize];
                            match can.transmit(tx_req.id, tx_req.is_extended, tx_req.is_rtr, data_slice).await {
                                Ok(()) => {
                                    info!("CAN TX: Enviado OK");
                                    let echo_frame = GsHostFrame::from_tx_msg_echo(
                                        tx_req.echo_id,
                                        tx_req.id,
                                        tx_req.is_extended,
                                        tx_req.dlc,
                                        &tx_req.data,
                                    );
                                    let _ = USB_ECHO_CHANNEL.try_send(echo_frame);
                                }
                                Err(bsp_f446::can::CanTxError::Timeout) => {
                                    warn!("CAN TX: Timeout — sin ACK del bus, mailboxes abortados");
                                }
                                Err(e) => {
                                    error!("CAN TX: Error al transmitir: {:?}", defmt::Debug2Format(&e));
                                }
                            }
                        }
                    }
                }
                Either::Second(Ok(env)) => {
                    let rx_frame = env.frame;
                    
                    let id: u32 = match rx_frame.id() {
                        embassy_stm32::can::Id::Standard(std) => std.as_raw() as u32,
                        embassy_stm32::can::Id::Extended(ext) => ext.as_raw() as u32,
                    };
                    let is_extended = matches!(rx_frame.id(), embassy_stm32::can::Id::Extended(_));
                    
                    let mut data = [0u8; 8];
                    let payload = rx_frame.data();
                    let dlc = payload.len();
                    if dlc <= 8 {
                        data[..dlc].copy_from_slice(payload);
                    }

                    info!("CAN RX: id=0x{:03X} ext={} dlc={} data={=[u8]:#X}", id, is_extended, dlc, &data[..dlc.min(8)]);

                    let generic_frame = can_protocol::CanFrame { id, is_extended, data, dlc: dlc as u8 };
                    CAN_RX_CHANNEL.send(generic_frame).await;
                }
                Either::Second(Err(e)) => {
                    can_error_count += 1;
                    let err_frame = match &e {
                        embassy_stm32::can::enums::BusError::Stuff => {
                            warn!("CAN RX error #{}, tipo: Stuff", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Stuff)
                        }
                        embassy_stm32::can::enums::BusError::Form => {
                            warn!("CAN RX error #{}, tipo: Form", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Form)
                        }
                        embassy_stm32::can::enums::BusError::Acknowledge => {
                            warn!("CAN RX error #{}, tipo: Acknowledge", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Acknowledge)
                        }
                        embassy_stm32::can::enums::BusError::BitRecessive => {
                            warn!("CAN RX error #{}, tipo: BitRecessive", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::BitRecessive)
                        }
                        embassy_stm32::can::enums::BusError::BitDominant => {
                            warn!("CAN RX error #{}, tipo: BitDominant", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::BitDominant)
                        }
                        embassy_stm32::can::enums::BusError::Crc => {
                            warn!("CAN RX error #{}, tipo: Crc", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Crc)
                        }
                        embassy_stm32::can::enums::BusError::Software => {
                            warn!("CAN RX error #{}, tipo: Software", can_error_count);
                            GsHostFrame::from_bus_error(CanBusError::Software)
                        }
                    };
                    let _ = USB_ECHO_CHANNEL.try_send(err_frame);
                    if can.can.is_sleeping() {
                        info!("CAN: Recuperando de bus-off...");
                        can.can.enable().await;
                        can_error_count = 0;
                    }
                }
            }
        } else {
            let cmd = CAN_CMD_CHANNEL.receive().await;
            match cmd {
                CanDriverCmd::Start { loopback, listen_only, one_shot } => {
                    info!("CAN: Iniciando controlador (loopback={}, listen_only={}, one_shot={})...", loopback, listen_only, one_shot);
                    is_started = true;
                    can.start(loopback, listen_only, one_shot).await;
                }
                CanDriverCmd::Stop => {
                    info!("CAN: Ya estaba detenido");
                }
                CanDriverCmd::SetBitTiming(timing) => {
                    info!("CAN: Configurando Bit Timing: brp={}, prop={}, phase1={}, phase2={}, sjw={}", 
                        timing.brp, timing.prop_seg, timing.phase_seg1, timing.phase_seg2, timing.sjw);
                    can.set_bit_timing(&timing);
                }
                CanDriverCmd::Transmit(_) => {
                    warn!("CAN: Ignorando transmisión, controlador detenido");
                }
            }
        }
    }
}