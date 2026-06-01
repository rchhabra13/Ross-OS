#![no_std]
#![no_main]

extern crate alloc;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

use alloc::boxed::Box;
use alloc::vec::Vec;

#[macro_use]
mod macros;
mod heap;
mod interrupts;
mod memory;
mod mmu;
mod sync;
mod task;
mod uart;

global_asm!(include_str!("boot.s"));
global_asm!(include_str!("exceptions.s"));
global_asm!(include_str!("switch.s"));

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    println!("=== AI-OS kernel ===");
    println!("Phase 5: processes + preemptive scheduler.");
    println!("Running at EL{}.", interrupts::current_el());

    // --- Physical frame allocator ---
    memory::init();
    let a = memory::alloc_frame().unwrap();
    let b = memory::alloc_frame().unwrap();
    println!("frame alloc: {:#x}, {:#x} ({} bytes free)", a, b, memory::free_bytes());

    // --- Enable the MMU (identity map) ---
    mmu::init();
    println!("MMU enabled: SCTLR.M={}", mmu::enabled() as u8);

    // Prove translation works: write/read a freshly allocated frame through its VA.
    let frame = memory::alloc_frame().unwrap();
    let p = frame as *mut u64;
    unsafe {
        core::ptr::write_volatile(p, 0xC0FFEE);
        let got = core::ptr::read_volatile(p);
        println!("vmem r/w @ {:#x} = {:#x}", frame, got);
    }

    // --- Heap: the `alloc` crate now works ---
    heap::init();
    let mut v: Vec<u32> = Vec::new();
    for i in 0..5 {
        v.push(i * i);
    }
    let boxed = Box::new(0xABCDu32);
    let s = alloc::format!("{} squares + box {:#x}", v.len(), *boxed);
    println!("heap ({} KiB): vec={:?} -> {}", heap::size() / 1024, v, s);

    // --- Tasks: register kmain as task 0, spawn two demo threads ---
    task::init();
    task::spawn("A", task_a);
    task::spawn("B", task_b);
    println!("spawned tasks A + B; starting preemptive scheduler.");

    // --- Timer IRQ now drives schedule() — must come last ---
    interrupts::init();
    println!("timer armed (100 Hz). kmain yields the CPU.\n");

    // kmain is itself task 0; the round-robin will rotate back here.
    loop {
        unsafe { asm!("wfi") }
    }
}

/// Demo thread. Busy-loops (never voluntarily yields) and prints a beat;
/// the timer IRQ preempts it, proving the scheduler is preemptive.
extern "C" fn task_a() {
    let mut n: u64 = 0;
    loop {
        for _ in 0..2_000_000 {
            unsafe { asm!("nop") }
        }
        n += 1;
        // Touch the shared heap every beat to exercise concurrent alloc/free
        // across preemption — proves the IRQ-masked allocator never deadlocks.
        let v: Vec<u64> = (0..n).collect();
        println!("[A] beat {} (sum {})", n, v.iter().sum::<u64>());
    }
}

extern "C" fn task_b() {
    let mut n: u64 = 0;
    loop {
        for _ in 0..2_000_000 {
            unsafe { asm!("nop") }
        }
        n += 1;
        let v: Vec<u64> = (0..n).collect();
        println!("[B] beat {} (sum {})", n, v.iter().sum::<u64>());
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[PANIC] {}", info);
    loop {
        unsafe { asm!("wfe") }
    }
}
