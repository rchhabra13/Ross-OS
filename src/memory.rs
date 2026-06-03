//! Physical frame allocator.
//!
//! Simplest useful design: a **bump allocator**. RAM that isn't part of the kernel
//! image, stack, or page tables is free; we hand it out one 4 KiB frame at a time by
//! advancing a pointer. No freeing yet (added with the heap in a later phase) — fine
//! for bootstrapping paging and early structures.

use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const FRAME_SIZE: usize = 4096;

/// RAM on the QEMU `virt` machine is 1 GiB at 0x4000_0000 (we pass `-m 1G`).
const RAM_END: usize = 0x8000_0000;

extern "C" {
    static __free_ram_start: u8;
}

/// Next free physical address. 0 until `init`.
static NEXT: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    let start = unsafe { &__free_ram_start as *const u8 as usize };
    let aligned = (start + FRAME_SIZE - 1) & !(FRAME_SIZE - 1);
    NEXT.store(aligned, Ordering::Relaxed);
}

/// Allocate one zeroed 4 KiB frame; returns its physical address.
pub fn alloc_frame() -> Option<usize> {
    let p = NEXT.fetch_add(FRAME_SIZE, Ordering::Relaxed);
    if p + FRAME_SIZE <= RAM_END {
        unsafe { ptr::write_bytes(p as *mut u8, 0, FRAME_SIZE) };
        Some(p)
    } else {
        None
    }
}

/// Allocate `bytes` of **contiguous**, frame-aligned physical RAM and return its
/// start address. The bump allocator is naturally contiguous, so this is just a
/// bigger bump — handy for a framebuffer that must be one linear region.
pub fn alloc_contig(bytes: usize) -> Option<usize> {
    let size = (bytes + FRAME_SIZE - 1) & !(FRAME_SIZE - 1);
    let p = NEXT.fetch_add(size, Ordering::Relaxed);
    if p + size <= RAM_END {
        unsafe { ptr::write_bytes(p as *mut u8, 0, size) };
        Some(p)
    } else {
        None
    }
}

/// Bytes of RAM still free.
pub fn free_bytes() -> usize {
    RAM_END.saturating_sub(NEXT.load(Ordering::Relaxed))
}
