#![no_std]
#![no_main]

extern crate alloc;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

use alloc::boxed::Box;
use alloc::vec::Vec;

#[macro_use]
mod macros;
mod fb;
mod font;
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
    println!("Phase 6: graphical framebuffer (ramfb).");
    println!("Running at EL{}.", interrupts::current_el());

    // Install exception vectors first so any early fault is reported, not silent.
    interrupts::install_vectors();

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

    // --- Framebuffer: bring up a graphical screen via ramfb ---
    if fb::init() {
        fb::clear(0x0010_2840); // dark blue desktop
        fb::fill_rect(0, 0, fb::WIDTH, 36, 0x0020_60C0); // top bar
        fb::draw_str(12, 10, "Ross-OS  --  aarch64 graphical kernel", 0x00FF_FFFF, 2);

        fb::fill_rect(40, 90, 300, 200, 0x00E0_4040); // red panel
        fb::draw_str(60, 110, "PROCESSES", 0x00FF_FFFF, 2);
        fb::fill_rect(380, 90, 300, 200, 0x0040_C060); // green panel
        fb::draw_str(400, 110, "MEMORY", 0x00FF_FFFF, 2);
        fb::fill_rect(720, 90, 260, 200, 0x00F0_C020); // yellow panel
        fb::draw_str(740, 110, "SCHEDULER", 0x0000_0000, 2);

        fb::draw_str(40, 720, "preemptive multitasking + framebuffer", 0x00A0_C0FF, 1);
        println!("framebuffer up: {}x{} ramfb", fb::WIDTH, fb::HEIGHT);
    } else {
        println!("no ramfb (run with -device ramfb)");
    }

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
