.section ".text.boot"
.global _start

_start:
    // Park all cores except core 0
    mrs     x1, mpidr_el1
    and     x1, x1, #3
    cbz     x1, 2f
1:  wfe
    b       1b

2:  // Set up stack
    ldr     x1, =__stack_top
    mov     sp, x1

    // Zero the BSS
    ldr     x1, =__bss_start
    ldr     x2, =__bss_end
3:  cmp     x1, x2
    b.ge    4f
    str     xzr, [x1], #8
    b       3b

4:  bl      kernel_main

    // If kernel_main returns, hang
5:  wfe
    b       5b
