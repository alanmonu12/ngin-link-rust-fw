# Flujo de datos CAN → USB

Este documento describe cómo fluyen los datos desde el bus CAN físico hasta el host Linux, y cómo el host envía comandos de control al device.

---

## Recepción: Bus CAN → Host

Es el flujo principal del firmware. Cada trama que llega al bus CAN debe llegar al host lo más rápido posible, sin perder datos.

<div align="center">
  <img src="../imgs/Flujo-CAN-USB.png" alt="Flujo CAN → USB" />
  <p><em>Flujo completo de datos: desde el bus CAN físico hasta el host Linux vía SocketCAN.</em></p>
</div>

### Paso a paso

1. **bxCAN Hardware recibe la trama.** El controlador CAN del STM32F446 tiene un FIFO de hardware que almacena tramas recibidas. Cuando llega una trama válida, se dispara la interrupción RX0.

2. **`can.can.read()` convierte a formato genérico.** Embassy provee un driver async que lee del FIFO y devuelve un `Envelope` con los campos `id`, `data` y `dlc`. Esto abstrae los registros del hardware.

3. **`can_rx_task` decodifica inline.** Antes de enviar al canal, la tarea intenta identificar el protocolo: ¿es OBD2? ¿es UDS? ¿es una trama raw? Si es diagnóstico, loguea el resultado con `defmt` para debugging en tiempo real.

4. **`CAN_RX_CHANNEL` absorbe ráfagas.** Es un buffer circular de 32 slots. Si el host no está leyendo (USB lento), la trama se queda en el buffer en lugar de perderse. Si el buffer se llena, `can_rx_task` se bloquea hasta que `usb_tx_task` libere un slot.

5. **`usb_tx_task` serializa con zero-copy.** Convierte `CanFrame` a `GsHostFrame` (ambos 20 bytes, `#[repr(C)]`) y usa `bytemuck::bytes_of()` para reinterpretar la memoria como bytes. No hay copia — solo un puntero reinterpretado.

6. **USB Bulk IN envía al host.** Escribe 20 bytes al endpoint IN. El host los recibe como una trama SocketCAN estándar.

### Por qué importa el diseño

- **Sin `alloc`:** No hay malloc/free. Los canales tienen tamaño fijo en tiempo de compilación.
- **Lock-free:** `embassy_sync::channel` usa interrupciones atómicas, no mutexes. `can_rx_task` puede correr en contexto de interrupción si fuera necesario.
- **Zero-copy:** La serialización con `bytemuck` evita copiar byte por byte. En un Cortex-M4 a 84MHz, esto ahorra ~50 ciclos por trama.

---

## Control: Host → Device

El host configura el device usando **control transfers** USB (vendor requests). Estos son los comandos que envía el driver `gs_usb` de Linux al conectar el dispositivo.

<div align="center">
  <img src="../imgs/Control-Host-Device.png" alt="Control Host → Device" />
  <p><em>Control transfers: configuración de bit timing, start y stop del controlador CAN.</em></p>
</div>

### Secuencia típica de inicialización

Cuando el usuario ejecuta `sudo ip link set can0 up`, el driver gs_usb realiza esta secuencia:

1. **`GS_USB_BREQ_BT_CONST`** — Pregunta al device: ¿qué frecuencia de reloj soportas? El firmware responde con `GsDeviceBtConst` (40 bytes) que incluye `fclk_can = 48MHz` y los límites de `tseg1`, `tseg2`, `sjw` y `brp`.

2. **`GS_USB_BREQ_BITTIMING`** — El driver calcula el bit timing óptimo para 500kbps y envía `GsDeviceBitTiming` al firmware. El callback `on_bit_timing_cb` lo enruta por `CAN_CTRL_CHANNEL` hasta `can_rx_task`, que configura el hardware CAN.

3. **`GS_USB_BREQ_MODE` (START=1)** — Activa el controlador CAN. El callback `on_start_cb` envía `CanCommand::Start` por el canal de control. `can_rx_task` llama a `can.start()` y comienza a escuchar el bus.

4. **`GS_USB_BREQ_MODE` (STOP=0)`** — Detiene el CAN. Útil para reconfigurar o apagar.

### Por qué usar canales para control

Los callbacks de USB corren en **contexto de interrupción**. No pueden hacer `await` (bloquear). Por eso usan `try_send()`: si la cola está llena, descarta silenciosamente. Para comandos esporádicos como start/stop esto es aceptable — la probabilidad de perder un comando es despreciable.

---

## Máquina de estados: can_rx_task

`can_rx_task` es la tarea más compleja del firmware. Debe manejar tres fuentes de datos simultáneamente usando `select3()`.

<div align="center">
  <img src="../imgs/Estados-de-can_rx_task.png" alt="Estados de can_rx_task" />
  <p><em>Máquina de estados de can_rx_task: transiciones entre modo DETENIDO e INICIADO.</em></p>
</div>

### Estados

| Estado | Descripción |
|--------|-------------|
| **DETENIDO** | CAN apagado. No escucha el bus. Solo procesa comandos de configuración. |
| **INICIADO** | CAN activo. Escucha 3 fuentes: comandos de control, tramas del bus, y solicitudes TX del host. |

### Transiciones

- **DETENIDO → INICIADO:** Cuando llega `CanCommand::Start` (ya sea por `on_start_cb` o directamente del canal).
- **INICIADO → DETENIDO:** Cuando llega `CanCommand::Stop`.
- **INICIADO → INICIADO:** `CanCommand::Start` se ignora (ya está activo). `SetBitTiming` requiere STOP primero (el hardware no permite cambiar velocidad en caliente).

### El problema de select3()

`can_rx_task` debe escuchar 3 fuentes sin prioridad fija:

```
select3(
    CAN_CTRL_CHANNEL.receive(),  // Comandos USB
    can.can.read(),              // Tramas del bus
    CAN_TX_CHANNEL.receive()     // Solicitudes TX del host
)
```

Si solo usara `select()` anidado, tendría dos problemas:
- **Menor eficiencia:** Dos selects en cascada = ~150 ciclos vs ~100 con select3.
- **Código ilegible:** Los `Either::First(Either::Second(...))` son confusos.

`select3()` resuelve ambos: un solo punto de decisión, más rápido y más claro.

### Flujo de una trama RX típica

```
1. can.can.read() retorna Ok(Envelope)
2. Se extrae id, data, dlc del envelope
3. Se crea CanFrame con esos campos
4. Se decodifica: OBD2, UDS, o Raw
5. Se envía a CAN_RX_CHANNEL.try_send()
6. Si la cola está llena → se bloquea (espera)
7. usb_tx_task consume la trama
8. Se serializa con bytemuck
9. Se escribe al EP IN (20 bytes)
10. Host recibe la trama en SocketCAN
```

### Flujo de una trama TX típica

```
1. Host envía GsTxMsg (20 bytes) por EP OUT
2. usb_rx_task lee y parsea con bytemuck
3. Se envía a CAN_TX_CHANNEL.try_send()
4. can_rx_task recibe la solicitud
5. Llama a can.transmit() al hardware
6. Si el ID es inválido → log error, se continúa
7. Si es exitoso → envía echo a USB_ECHO_CHANNEL
8. usb_tx_task envía el echo al host
9. Host confirma que la trama fue enviada
```
