//! Exception/interrupt setup: vector table, GICv2, and the ARM generic timer.
//!
//! Flow to get a periodic interrupt:
//!   1. Point VBAR_EL1 at the vector table (`exceptions.s`).
//!   2. Enable the GIC distributor + CPU interface, enable the timer interrupt.
//!   3. Program the generic timer to fire once per second.
//!   4. Unmask IRQs in PSTATE (clear the DAIF.I bit).
//! Then `el1_irq` -> `rust_irq_handler` runs every tick.

use core::arch::asm;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

// GICv2 base addresses on the QEMU `virt` machine.
const GICD_BASE: usize = 0x0800_0000; // Distributor
const GICC_BASE: usize = 0x0801_0000; // CPU interface

const GICD_CTLR: usize = 0x000;
const GICD_ISENABLER: usize = 0x100; // set-enable, 1 bit per interrupt
const GICC_CTLR: usize = 0x000;
const GICC_PMR: usize = 0x004; // priority mask
const GICC_IAR: usize = 0x00C; // interrupt acknowledge
const GICC_EOIR: usize = 0x010; // end of interrupt

/// Non-secure EL1 physical timer = PPI, interrupt ID 30.
const TIMER_IRQ: u32 = 30;

static TICKS: AtomicU64 = AtomicU64::new(0);

#[inline]
unsafe fn write32(addr: usize, val: u32) {
    ptr::write_volatile(addr as *mut u32, val);
}

#[inline]
unsafe fn read32(addr: usize) -> u32 {
    ptr::read_volatile(addr as *const u32)
}

#[inline]
fn cntfrq() -> u64 {
    let v: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) v) };
    v
}

#[inline]
fn set_timer_countdown(ticks: u64) {
    unsafe { asm!("msr cntp_tval_el0, {}", in(reg) ticks) };
}

#[inline]
fn enable_timer() {
    // CNTP_CTL_EL0: bit0 ENABLE, bit1 IMASK (0 = unmasked).
    unsafe { asm!("msr cntp_ctl_el0, {}", in(reg) 1u64) };
}

/// Current exception level (1 = EL1, 2 = EL2, ...).
pub fn current_el() -> u64 {
    let v: u64;
    unsafe { asm!("mrs {}, CurrentEL", out(reg) v) };
    (v >> 2) & 0b11
}

pub fn init() {
    unsafe {
        // 1. Install the vector table.
        extern "C" {
            static vectors: u8;
        }
        let vbar = &vectors as *const u8 as u64;
        asm!("msr vbar_el1, {}", in(reg) vbar);

        // 2. GIC: enable distributor, enable timer IRQ, open priority mask,
        //    enable CPU interface.
        write32(GICD_BASE + GICD_CTLR, 1);
        let reg = (TIMER_IRQ / 32) as usize * 4;
        write32(GICD_BASE + GICD_ISENABLER + reg, 1 << (TIMER_IRQ % 32));
        write32(GICC_BASE + GICC_PMR, 0xFF);
        write32(GICC_BASE + GICC_CTLR, 1);

        // 3. Timer: fire at 100 Hz (snappy preemption).
        set_timer_countdown(cntfrq() / 100);
        enable_timer();

        // 4. Unmask IRQs (clear PSTATE.I).
        asm!("msr daifclr, #2");
    }
}

#[no_mangle]
extern "C" fn rust_irq_handler() {
    let iar = unsafe { read32(GICC_BASE + GICC_IAR) };
    let intid = iar & 0x3FF;

    if intid == TIMER_IRQ {
        TICKS.fetch_add(1, Ordering::Relaxed);
        set_timer_countdown(cntfrq() / 100); // re-arm for the next 10 ms slice
    }

    // Tell the GIC we're done with this interrupt before we switch tasks.
    unsafe { write32(GICC_BASE + GICC_EOIR, iar) };

    // Preempt: rotate to the next task on every timer tick.
    if intid == TIMER_IRQ {
        crate::task::schedule();
    }
}

#[no_mangle]
extern "C" fn rust_default_exception(esr: u64, elr: u64) -> ! {
    crate::println!("[EXC] unhandled: ESR={:#018x} ELR={:#018x}", esr, elr);
    loop {
        unsafe { asm!("wfe") }
    }
}
