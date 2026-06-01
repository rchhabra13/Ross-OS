ENTRY(_start)

SECTIONS
{
    . = 0x40000000;          /* QEMU virt loads -kernel here */

    .text : {
        KEEP(*(.text.boot))  /* _start must come first */
        *(.text*)
    }

    .rodata : { *(.rodata*) }
    .data   : { *(.data*) }

    . = ALIGN(8);
    __bss_start = .;
    .bss : { *(.bss*) *(COMMON) }
    . = ALIGN(8);
    __bss_end = .;

    . = ALIGN(16);
    . = . + 0x80000;         /* 512 KiB stack */
    __stack_top = .;

    . = ALIGN(4096);
    __free_ram_start = .;    /* frame allocator owns RAM from here up */
}
