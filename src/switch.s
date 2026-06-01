// Cooperative/preemptive context switch between kernel threads.
//
//   cpu_switch(prev: *mut Context, next: *const Context)
//     x0 = prev, x1 = next
//
// Only the AAPCS callee-saved state must be preserved across a function call:
// x19..x28, FP (x29), LR (x30), and SP. Caller-saved registers are already dealt
// with by the compiler at the call site, so saving these 13 values is enough to
// freeze a thread and resume it later. The Context struct layout must match these
// offsets exactly (x19@0 … x30@88, sp@96).

.section .text
.global cpu_switch
cpu_switch:
    mov     x2, sp
    stp     x19, x20, [x0, #0]
    stp     x21, x22, [x0, #16]
    stp     x23, x24, [x0, #32]
    stp     x25, x26, [x0, #48]
    stp     x27, x28, [x0, #64]
    stp     x29, x30, [x0, #80]
    str     x2,       [x0, #96]

    ldp     x19, x20, [x1, #0]
    ldp     x21, x22, [x1, #16]
    ldp     x23, x24, [x1, #32]
    ldp     x25, x26, [x1, #48]
    ldp     x27, x28, [x1, #64]
    ldp     x29, x30, [x1, #80]
    ldr     x2,       [x1, #96]
    mov     sp, x2
    ret                          // returns to the restored LR (x30)

// First-run entry for a freshly spawned thread. cpu_switch `ret`s here with
// x19 = entry fn. Enable IRQs (so the thread is preemptible) then call it.
.global task_trampoline
task_trampoline:
    msr     daifclr, #2          // unmask IRQs in the new thread
    blr     x19                  // call entry()
1:  wfe                          // if entry returns, park forever
    b       1b
