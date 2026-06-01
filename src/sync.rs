//! Critical sections for a preemptive single-core kernel.
//!
//! Once the timer IRQ can switch tasks at any instruction, any state shared
//! between tasks (the heap's free list, the UART) must be touched with IRQs
//! masked — otherwise a task can be frozen mid-update and another task (or the
//! IRQ handler itself) observes/corrupts a half-modified structure.
//!
//! `irq_save` masks IRQs and returns the previous DAIF so sections can nest;
//! `irq_restore` puts DAIF back exactly as it was (re-enabling only if the
//! caller's caller had them enabled). `without_preempt` wraps the common case.

use core::arch::asm;

/// Mask IRQs (set DAIF.I) and return the prior DAIF for later restore.
#[inline]
pub fn irq_save() -> u64 {
    let daif: u64;
    unsafe {
        asm!("mrs {}, daif", out(reg) daif);
        asm!("msr daifset, #2"); // set the I bit
    }
    daif
}

/// Restore DAIF saved by `irq_save`. Nest-safe: only unmasks if it was unmasked.
#[inline]
pub fn irq_restore(daif: u64) {
    unsafe { asm!("msr daif, {}", in(reg) daif) };
}

/// Run `f` with IRQs masked, then restore the previous interrupt state.
#[inline]
pub fn without_preempt<R>(f: impl FnOnce() -> R) -> R {
    let saved = irq_save();
    let r = f();
    irq_restore(saved);
    r
}
