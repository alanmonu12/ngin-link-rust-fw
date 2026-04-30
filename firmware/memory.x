MEMORY
{
  /* DEFINICIONES FÍSICAS (Hardware del STM32H750) */
  /* Flash: Donde vive tu código */
  FLASH   : ORIGIN = 0x08000000, LENGTH = 128K
  
  /* DTCM: RAM de ultra-alta velocidad (Pegada al CPU) */
  DTCMRAM : ORIGIN = 0x20000000, LENGTH = 128K
  
  /* RAM (AXI): RAM de propósito general (Grande) */
  RAM     : ORIGIN = 0x24000000, LENGTH = 512K
}

/* ALIAS LÓGICOS (Software de Rust) */
REGION_ALIAS("REGION_TEXT", FLASH);
REGION_ALIAS("REGION_RODATA", FLASH);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);

/* Poner el Stack en DTCM es vital para el rendimiento del motor */
REGION_ALIAS("REGION_STACK", DTCMRAM);