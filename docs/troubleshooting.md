# Troubleshooting y Debugging

## Visión General

Guía para diagnosticar y resolver problemas comunes en la implementación USB TX/RX.

## Síntomas y Soluciones

### 1. El device no es detectado por Linux

**Síntomas:**
```bash
$ lsusb | grep 1d50:606f
(no output)
```

**Causas posibles:**
1. VID/PID incorrecto
2. Descripción USB inválida
3. Endpoint IN/OUT no configurado
4. Firmware no compilado correctamente

**Diagnóstico:**
```bash
# Verificar USB con más detalle
lsusb -v -d 1d50:606f

# Verificar dmesg
dmesg | tail -20

# Buscar errores USB
dmesg | grep -i usb
```

**Soluciones:**
1. Verificar `default_gs_usb_config()` en `gs_usb_protocol/src/lib.rs`
2. Verificar configuración de endpoints en `main.rs`
3. Re-compilar y flashear: `cargo run --release`

### 2. El device se detecta pero no crea interface CAN

**Síntomas:**
```bash
$ ip link show can0
Device "can0" does not exist.
```

**Causas posibles:**
1. Driver `gs_usb` no cargado
2. Interface no creada automáticamente
3. Error en control transfers

**Diagnóstico:**
```bash
# Verificar si el driver está cargado
lsmod | grep gs_usb

# Cargar el driver manualmente
sudo modprobe gs_usb

# Verificar interface
ip link show type can

# Forzar creación de interface
sudo ip link add dev can0 type can
```

**Soluciones:**
1. Cargar driver: `sudo modprobe gs_usb`
2. Crear interface manualmente: `sudo ip link add dev can0 type can`
3. Verificar logs del firmware con `DEFMT_LOG=trace`

### 3. Error "No such device" al configurar CAN

**Síntomas:**
```bash
$ sudo ip link set can0 type can bitrate 500000
RTNETLINK answers: No such device
```

**Causas posibles:**
1. Interface no creada
2. Driver no reconoce el device
3. Error en control transfer de bit timing

**Diagnóstico:**
```bash
# Verificar interface
ip link show

# Verificar driver
dmesg | grep gs_usb

# Probar con candump (aunque no funcione)
candump can0
```

**Soluciones:**
1. Crear interface: `sudo ip link add dev can0 type can`
2. Verificar que el firmware responde a `GS_USB_BREQ_BT_CONST`
3. Revisar logs del firmware

### 4. Error "Address already in use" al subir interface

**Síntomas:**
```bash
$ sudo ip link set can0 up
RTNETLINK answers: Address already in use
```

**Causas posibles:**
1. Interface ya está activa
2. Otro proceso la está usando
3. Error en el firmware al procesar START

**Soluciones:**
```bash
# Verificar estado
ip link show can0

# Bajar y subir de nuevo
sudo ip link set can0 down
sudo ip link set can0 up

# Matar procesos que usen can0
sudo fuser -k can0
```

### 5. No se reciben tramas (candump vacío)

**Síntomas:**
```bash
$ candump can0
(no output, esperando tramas...)
```

**Causas posibles:**
1. CAN no está activo (falta `ip link set can0 up`)
2. Bus CAN sin tráfico
3. Firmware no está en modo START
4. Error en USB TX

**Diagnóstico:**
```bash
# Verificar que CAN está activo
ip link show can0

# Verificar tráfico en el bus (si hay otro dispositivo)
cangen can0 -g 100 -I 42A -L 8 -D 1122334455667788 -p 100

# Verificar logs del firmware
DEFMT_LOG=info cargo run --release
```

**Soluciones:**
1. Activar interface: `sudo ip link set can0 up`
2. Verificar conexión física CAN_H/CAN_L
3. Verificar que el firmware recibe START: logs con `CAN: Iniciando controlador...`

### 6. No se pueden enviar tramas (cansend falla)

**Síntomas:**
```bash
$ cansend can0 123#DEADBEEF
write: No route to host
```

**Causas posibles:**
1. CAN no está activo
2. No hay ACK en el bus (dispositivo único)
3. Error en USB RX del firmware
4. Buffer CAN TX lleno

**Diagnóstico:**
```bash
# Verificar estado
ip link show can0

# Probar con loopback interno
sudo modprobe can_dev
sudo modprobe can_raw
ip link set can0 type can bitrate 500000 loopback on
sudo ip link set can0 up
cansend can0 123#DEADBEEF
candump can0  # Debería ver la trama
```

**Soluciones:**
1. Verificar que el firmware está en modo START
2. Usar loopback para pruebas
3. Verificar logs de USB RX: `USB RX: Recibidos X bytes`
4. Verificar que `CAN_CMD_CHANNEL` no está lleno

### 7. Echo de TX no funciona

**Síntomas:**
- `cansend` retorna éxito pero no se ve echo en `candump`
- Driver gs_usb reporta timeout

**Causas posibles:**
1. `USB_ECHO_CHANNEL` no está conectado
2. `usb_tx_task` no está leyendo echoes
3. Error al crear `GsHostFrame::from_tx_msg_echo()`
4. `echo_id` no se preserva correctamente

**Diagnóstico:**
```bash
# Verificar logs del firmware
DEFMT_LOG=trace cargo run --release

# Buscar mensajes de echo
# Debería ver: "USB TX: Enviando echo con ID X"
```

**Soluciones:**
1. Verificar que `usb_tx_task` usa `select()` con ambos canales
2. Verificar que `can_driver_task` envía a `USB_ECHO_CHANNEL` después de TX exitoso
3. Verificar que `echo_id` se preserva del `GsTxMsg`

### 8. Drop de mensajes bajo alta carga

**Síntomas:**
- `candump` muestra tramas faltantes
- `cangen` reporta drops
- Logs muestran warnings de "Cola CAN TX llena"

**Causas posibles:**
1. USB es bottleneck (limitado a ~1ms)
2. Canales demasiado pequeños
3. CPU sobrecargada
4. Prioridades incorrectas

**Diagnóstico:**
```bash
# Generar carga alta
cangen can0 -g 1 -I 42A -L 8 -p 10

# Monitorear drops
candump can0 | wc -l
# Debería ser similar a la tasa de envío
```

**Soluciones:**
1. Aumentar capacidad de canales: `Channel<..., 64>`
2. Usar BufferedCan en lugar de Can
3. Agregar prioridades a tareas
4. Optimizar compilación (LTO, opt-level)

### 9. Error "InvalidCanId" en transmisión

**Síntomas:**
```
CAN TX: Error al transmitir: InvalidCanId
```

**Causas posibles:**
1. ID > 0x7FF para standard
2. ID > 0x1FFFFFFF para extended
3. Flags malinterpretados

**Diagnóstico:**
```rust
// Verificar que se extrae el ID correctamente
let msg: GsTxMsg = bytemuck::pod_read_unaligned(&buf);
defmt::info!("ID: 0x{:08X}, extended: {}", msg.id(), msg.is_extended());
```

**Soluciones:**
1. Verificar que `GsTxMsg::id()` aplica la máscara correcta
2. Verificar que `is_extended()` lee el bit 31 de `can_id`
3. Verificar que el host envía flags correctamente

### 10. Stack overflow

**Síntomas:**
- Hard fault
- Comportamiento indefinido
- Resets espontáneos

**Causas posibles:**
1. Tareas con buffers grandes en stack
2. Recursión
3. Arrays grandes en variables locales

**Diagnóstico:**
```rust
// Agregar magic numbers al stack
static mut STACK_MARKER: [u8; 1024] = [0xAA; 1024];

// En cada tarea, verificar que el marker no fue modificado
fn check_stack() {
    unsafe {
        if STACK_MARKER[0] != 0xAA {
            defmt::error!("Stack overflow detectado!");
        }
    }
}
```

**Soluciones:**
1. Mover buffers a `static`
2. Reducir tamaño de buffers locales
3. Usar `heapless` para colecciones estáticas

## Herramientas de Debug

### 1. defmt (Defragmented Formatting)

```rust
// Logs con defmt
info!("Iniciando sistema...");
warn!("Buffer CAN TX al {}%", usage);
error!("Error USB: {:?}", error);
trace!("Trama recibida: {:?}", frame);
```

**Uso:**
```bash
DEFMT_LOG=trace cargo run --release
```

### 2. probe-rs (Debug con Probe)

```bash
# Flashear y ejecutar
cargo run --release

# Debug con GDB
probe-rs debug --chip STM32F446RE

# Capturar panics
probe-rs run --chip STM32F446RE -- panic
```

### 3. USB Sniffing

```bash
# Instalar usbmon
sudo modprobe usbmon

# Capturar tráfico USB
sudo tcpdump -i usbmon0 -w capture.pcap

# Analizar con Wireshark
wireshark capture.pcap
```

### 4. CAN Sniffing

```bash
# Ver todo el tráfico
candump any

# Filtrar por ID
candump can0,42F:7FF

# Guardar a archivo
candump can0 > capture.log

# Analizar con can-utils
canplayer -I capture.log
```

### 5. Contadores de Performance

```rust
// Agregar al firmware
static mut RX_COUNT: u32 = 0;
static mut TX_COUNT: u32 = 0;
static mut DROP_COUNT: u32 = 0;

// En usb_rx_task:
unsafe { RX_COUNT += 1; }

// En can_driver_task (después de transmitir exitosamente):
unsafe { TX_COUNT += 1; }

// En usb_rx_task (cuando falla try_send):
unsafe { DROP_COUNT += 1; }

// Reportar cada 10 segundos
defmt::info!("Stats: RX={}, TX={}, DROP={}", 
    unsafe { RX_COUNT }, unsafe { TX_COUNT }, unsafe { DROP_COUNT });
```

## Flujo de Debugging

### 1. No funciona nada

```
1. Verificar compilación: cargo build --release
2. Verificar flasheo: cargo run --release
3. Verificar logs: DEFMT_LOG=info
4. Verificar USB: lsusb -v
5. Verificar CAN: ip link show
```

### 2. Solo RX funciona

```
1. Verificar USB RX: logs de "USB RX: Recibidos X bytes"
2. Verificar CAN TX: logs de "CAN TX: Error"
3. Verificar CAN_CMD_CHANNEL: logs de "Cola CAN CMD llena"
4. Verificar CAN transmite: candump en otro dispositivo
```

### 3. Solo TX funciona

```
1. Verificar CAN RX: logs de "CAN: Trama recibida"
2. Verificar CAN_RX_CHANNEL: uso del canal
3. Verificar USB TX: logs de "USB TX: Error"
4. Verificar host lee: candump debería mostrar tramas
```

### 4. Echo no funciona

```
1. Verificar USB_ECHO_CHANNEL: logs de envío
2. Verificar usb_tx_task lee echoes: select() con USB_ECHO_CHANNEL
3. Verificar echo_id: logs del ID en echo
4. Verificar flags: GS_USB_FLAG_TX_ECHO activo
```

## Logs Esperados

### Startup Normal

```
Hardware y relojes configurados. Iniciando driver USB...
USB TX: Endpoint IN habilitado, listo para enviar tramas al host
USB RX: Endpoint OUT habilitado, listo para recibir tramas del host
¡Sistema configurado y listo!
```

### Operación Normal

```
CAN: Iniciando controlador...
OBD2: Obd2Request { ... }
UDS: UdsMessage { ... }
USB RX: Recibidos 20 bytes del host
CAN TX: Trama transmitida exitosamente
```

### Errores

```
USB TX: Error al escribir al endpoint: ...
USB RX: Error al leer del endpoint: ...
CAN TX: Error al transmitir: InvalidCanId
USB RX: Cola CAN TX llena, descartando trama
```

## Referencias

- [Embassy Debugging](https://docs.embassy.dev/embassy-stm32/)
- [probe-rs Documentation](https://probe.rs/)
- [defmt Book](https://defmt.ferrous-systems.com/)
- [SocketCAN Troubleshooting](https://www.kernel.org/doc/html/latest/networking/can.html)
