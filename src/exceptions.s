// aarch64 exception vector table for EL1.
//
// The table has 16 entries, each 0x80 bytes, and must be 0x800-aligned.
// VBAR_EL1 is set to `vectors` at init. The CPU jumps to the entry matching
// (source EL / stack pointer / exception type). We run the kernel on SP_ELx,
// so hardware IRQs land in the "Current EL, SPx, IRQ" slot (offset 0x280).

.section .text

// Save the general-purpose registers on the stack before calling into Rust.
.macro SAVE_CONTEXT
    sub     sp, sp, #272
    stp     x0,  x1,  [sp, #16 * 0]
    stp     x2,  x3,  [sp, #16 * 1]
    stp     x4,  x5,  [sp, #16 * 2]
    stp     x6,  x7,  [sp, #16 * 3]
    stp     x8,  x9,  [sp, #16 * 4]
    stp     x10, x11, [sp, #16 * 5]
    stp     x12, x13, [sp, #16 * 6]
    stp     x14, x15, [sp, #16 * 7]
    stp     x16, x17, [sp, #16 * 8]
    stp     x18, x19, [sp, #16 * 9]
    stp     x20, x21, [sp, #16 * 10]
    stp     x22, x23, [sp, #16 * 11]
    stp     x24, x25, [sp, #16 * 12]
    stp     x26, x27, [sp, #16 * 13]
    stp     x28, x29, [sp, #16 * 14]
    stp     x30, xzr, [sp, #16 * 15]
    // Save the return state too. A context switch inside the handler lets other
    // exceptions overwrite the shared ELR_EL1/SPSR_EL1, so each frame must carry
    // its own copy or the final `eret` resumes at a stale PC.
    mrs     x9,  elr_el1
    mrs     x10, spsr_el1
    stp     x9,  x10, [sp, #16 * 16]
.endm

.macro RESTORE_CONTEXT
    ldp     x9,  x10, [sp, #16 * 16]
    msr     elr_el1,  x9
    msr     spsr_el1, x10
    ldp     x0,  x1,  [sp, #16 * 0]
    ldp     x2,  x3,  [sp, #16 * 1]
    ldp     x4,  x5,  [sp, #16 * 2]
    ldp     x6,  x7,  [sp, #16 * 3]
    ldp     x8,  x9,  [sp, #16 * 4]
    ldp     x10, x11, [sp, #16 * 5]
    ldp     x12, x13, [sp, #16 * 6]
    ldp     x14, x15, [sp, #16 * 7]
    ldp     x16, x17, [sp, #16 * 8]
    ldp     x18, x19, [sp, #16 * 9]
    ldp     x20, x21, [sp, #16 * 10]
    ldp     x22, x23, [sp, #16 * 11]
    ldp     x24, x25, [sp, #16 * 12]
    ldp     x26, x27, [sp, #16 * 13]
    ldp     x28, x29, [sp, #16 * 14]
    ldr     x30,      [sp, #16 * 15]
    add     sp, sp, #272
.endm

// One table entry: align to 0x80, branch to a handler.
.macro VENTRY label
.balign 0x80
    b       \label
.endm

.balign 0x800
.global vectors
vectors:
    VENTRY default_handler   // 0x000  Current EL, SP0,  Synchronous
    VENTRY default_handler   // 0x080  Current EL, SP0,  IRQ
    VENTRY default_handler   // 0x100  Current EL, SP0,  FIQ
    VENTRY default_handler   // 0x180  Current EL, SP0,  SError
    VENTRY default_handler   // 0x200  Current EL, SPx,  Synchronous
    VENTRY el1_irq           // 0x280  Current EL, SPx,  IRQ   <-- timer
    VENTRY default_handler   // 0x300  Current EL, SPx,  FIQ
    VENTRY default_handler   // 0x380  Current EL, SPx,  SError
    VENTRY default_handler   // 0x400  Lower EL aarch64, Synchronous
    VENTRY default_handler   // 0x480  Lower EL aarch64, IRQ
    VENTRY default_handler   // 0x500  Lower EL aarch64, FIQ
    VENTRY default_handler   // 0x580  Lower EL aarch64, SError
    VENTRY default_handler   // 0x600  Lower EL aarch32, Synchronous
    VENTRY default_handler   // 0x680  Lower EL aarch32, IRQ
    VENTRY default_handler   // 0x700  Lower EL aarch32, FIQ
    VENTRY default_handler   // 0x780  Lower EL aarch32, SError

el1_irq:
    SAVE_CONTEXT
    bl      rust_irq_handler
    RESTORE_CONTEXT
    eret

default_handler:
    SAVE_CONTEXT
    mrs     x0, esr_el1          // exception syndrome
    mrs     x1, elr_el1          // faulting address
    bl      rust_default_exception
    RESTORE_CONTEXT
    eret
