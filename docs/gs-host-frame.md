# GsHostFrame: formato binario USB

El driver `gs_usb` del kernel Linux espera tramas con este layout exacto en el Bulk IN endpoint.

## Estructura (20 bytes)

```
Offset  Tamaño  Campo       Descripción
──────  ──────  ──────────  ─────────────────────────────────────────
 0      4       echo_id     ID de eco (0 = trama recibida del bus)
 4      4       can_id      ID del frame + flags (ver abajo)
 8      1       can_dlc     Longitud de datos (0-8)
 9      1       channel     Canal CAN (0 = único canal)
10      1       flags       Flags de la trama (ver abajo)
11      1       reserved    Reservado (0)
12      8       data        Payload CAN (hasta 8 bytes)
```

## Flags en can_id (bits 31-29)

```
Bit 31  GS_CAN_ID_FLAG_EFF   Frame extendido (29 bits)
Bit 30  GS_CAN_ID_FLAG_RTR   Remote Transmission Request
Bit 29  GS_CAN_ID_FLAG_ERR   Error frame
```

Para IDs estándar (11 bits), los bits 31-29 son 0 y el ID va en bits 10-0.
Para IDs extendidos (29 bits), se setea bit 31 y el ID va en bits 28-0.

```
Standard:  0b000_AAAAA_AAAAA_AAAAA_AAAAA_AAAAA_AAAAA_AAA (11 bits)
Extended:  0b1_AA_BB_BB_BB_BB_BB_BB_BB_BB_BB_BB_BB_BB (29 bits)
```

## Flags del frame (byte 10)

```
Bit 0  GS_CAN_FLAG_OVERFLOW  Overflow del buffer RX
Bit 1  GS_CAN_FLAG_FD        Frame CAN FD
Bit 2  GS_CAN_FLAG_BRS       Bit Rate Switch (solo FD)
Bit 3  GS_CAN_FLAG_ESI       Error State Indicator (solo FD)
```

## Serialización con bytemuck

`GsHostFrame` es `#[repr(C)]` + `Pod`, lo que permite serializar
directamente a bytes sin copia:

```rust
let frame = GsHostFrame::from_can_frame(0x7DF, false, 8, &data);
let bytes: &[u8; 20] = bytemuck::bytes_of(&frame);
ep_in.write(bytes).await;
```

Esto funciona porque:
1. `#[repr(C)]` garantiza layout sin padding
2. `Pod` permite reinterpretar la memoria como `&[u8]`
3. `bytemuck::bytes_of()` es una reinterpretación zero-cost

## Ejemplo: ID 0x7DF, 8 bytes

```
Bytes hex:  00 00 00 00  DF 07 00 00  08 00 00 00  11 22 33 44 55 66 77 88
            ──────────── ──────────── ── ── ── ── ─────────────────────────
            echo_id=0    can_id=0x7DF  dlc ch  fl  data
                          (standard)    8   0   0
```

## Ejemplo: ID extendido 0x1ABCDEF

```
Bytes hex:  00 00 00 00  EF DE BC 9A  04 00 00 00  DE AD BE EF 00 00 00 00
                          ───────────
                          0x80000000 | 0x1ABCDEF (bit EFF set)
```
