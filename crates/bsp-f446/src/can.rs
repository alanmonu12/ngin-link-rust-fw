use embassy_stm32::bind_interrupts;
use embassy_stm32::can::{Can, Fifo, Mailbox, Rx0InterruptHandler, Rx1InterruptHandler, SceInterruptHandler, TxInterruptHandler};
use embassy_stm32::can::filter::Mask32;
use embassy_stm32::can::enums::FrameCreateError;
use embassy_stm32::peripherals;

bind_interrupts!(pub struct Irqs {
    CAN1_RX0 => Rx0InterruptHandler<peripherals::CAN1>;
    CAN1_RX1 => Rx1InterruptHandler<peripherals::CAN1>;
    CAN1_SCE => SceInterruptHandler<peripherals::CAN1>;
    CAN1_TX => TxInterruptHandler<peripherals::CAN1>;
});

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CanTxError {
    FrameError(FrameCreateError),
    Timeout,
}

pub struct BspCan {
    pub can: Can<'static>,
}

impl BspCan {

    pub async fn start(&mut self, loopback: bool, listen_only: bool, one_shot: bool) {
        // Configurar filtro de aceptación: sin filtros activos, el bxCAN rechaza TODAS las tramas.
        // Bank 0 con Mask32::accept_all() permite recibir cualquier ID (standard + extended).
        self.can.modify_filters()
            .enable_bank(0, Fifo::Fifo0, Mask32::accept_all());

        // Entrar en modo init, configurar modo y bit timing, y al hacer drop salir de init.
        // Nota: leave_init_mode() del Drop de CanConfig pone el periférico en sleep mode.
        // one_shot = No retransmitir automáticamente frames con error (ACK, etc.)
        self.can.modify_config()
            .set_silent(listen_only)
            .set_loopback(loopback)
            .set_automatic_retransmit(!one_shot);

        // Salir explícitamente del sleep mode y sincronizar con el bus CAN.
        // enable() espera 11 bits recesivos consecutivos (bus idle) antes de confirmar.
        // Sin esta llamada, el periférico permanece en sleep y no recibe ni transmite nada.
        self.can.enable().await;

        defmt::info!("CAN: Iniciado OK (sleep={}, silent={}, loopback={}, nart={})",
            self.can.is_sleeping(), listen_only, loopback, one_shot);
    }

    pub async fn stop(&mut self) {
        // Poner el CAN en modo silencioso (Listen-only): no acusa recibo (ACK)
        // ni interfiere físicamente con otros dispositivos en la red CAN.
        // Después de modify_config() el periférico queda en sleep, así que
        // hay que llamar enable() para sacarlo de sleep.
        self.can.modify_config()
            .set_silent(true);
        self.can.enable().await;
    }

    /// Re-habilita el periférico CAN después de un modify_config() en caliente.
    /// modify_config() entra en init mode y al salir pone el CAN en sleep.
    /// Este método lo saca del sleep y lo sincroniza con el bus.
    pub async fn reenable(&mut self) {
        self.can.enable().await;
    }

    pub fn set_bit_timing(&mut self, timing: &gs_usb_protocol::gs_usb_types::GsDeviceBitTiming) {
        // En la versión 0.6.0 de embassy-stm32, ya no manipulamos el registro BTR directamente.
        // Usamos NominalBitTiming, que recibe los valores reales (sin restar 1).
        // En la arquitectura CAN del STM32, prop_seg y phase_seg1 están combinados.
        // Usamos .max(1) para garantizar que los valores NonZero nunca sean 0 y evitar pánicos.
        let bt = embassy_stm32::can::util::NominalBitTiming {
            sync_jump_width: core::num::NonZeroU8::new((timing.sjw as u8).max(1)).unwrap(),
            seg1: core::num::NonZeroU8::new(((timing.prop_seg + timing.phase_seg1) as u8).max(1)).unwrap(),
            seg2: core::num::NonZeroU8::new((timing.phase_seg2 as u8).max(1)).unwrap(),
            prescaler: core::num::NonZeroU16::new((timing.brp as u16).max(1)).unwrap(),
        };

        self.can.modify_config()
            .set_bit_timing(bt);
    }

    pub async fn transmit(&mut self, id: u32, is_extended: bool, is_rtr: bool, data: &[u8]) -> Result<(), CanTxError> {
        use embassy_stm32::can::{Id, StandardId, ExtendedId, Frame};

        let can_id = if is_extended {
            match ExtendedId::new(id) {
                Some(eid) => Id::Extended(eid),
                None => return Err(CanTxError::FrameError(FrameCreateError::InvalidCanId)),
            }
        } else {
            match StandardId::new(id as u16) {
                Some(sid) => Id::Standard(sid),
                None => return Err(CanTxError::FrameError(FrameCreateError::InvalidCanId)),
            }
        };

        let frame = if is_rtr {
            Frame::new_remote(can_id, data.len()).map_err(CanTxError::FrameError)?
        } else {
            Frame::new_data(can_id, data).map_err(CanTxError::FrameError)?
        };

        match embassy_time::with_timeout(embassy_time::Duration::from_millis(100), self.can.write(&frame)).await {
            Ok(_status) => Ok(()),
            Err(_) => {
                self.can.abort(Mailbox::Mailbox0);
                self.can.abort(Mailbox::Mailbox1);
                self.can.abort(Mailbox::Mailbox2);
                Err(CanTxError::Timeout)
            }
        }
    }
}
