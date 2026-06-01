//! Kernel heap — turns a fixed RAM region into a `GlobalAlloc` so the `alloc`
//! crate (`Vec`, `Box`, `String`, …) works.
//!
//! We hand a static 1 MiB buffer to `linked_list_allocator`, which tracks free
//! blocks in an intrusive linked list and supports real `dealloc` (unlike the
//! bump frame allocator). One global allocator is enough for the whole kernel.

use core::alloc::{GlobalAlloc, Layout};
use linked_list_allocator::LockedHeap;

const HEAP_SIZE: usize = 1024 * 1024; // 1 MiB

/// Backing storage, lives in `.bss` (zeroed, in identity-mapped Normal RAM).
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

/// `LockedHeap` is a spin `Mutex`. Under preemption that's a hazard: if the timer
/// IRQ fires while a task holds the lock and the handler (or the next task) tries
/// to allocate, it spins on a lock the frozen holder can't release. Wrapping every
/// alloc/dealloc in an IRQ-masked critical section makes them atomic w.r.t. the
/// scheduler, so the lock is never held across a context switch.
struct IrqSafeHeap(LockedHeap);

unsafe impl GlobalAlloc for IrqSafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        crate::sync::without_preempt(|| self.0.alloc(layout))
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        crate::sync::without_preempt(|| self.0.dealloc(ptr, layout))
    }
}

#[global_allocator]
static ALLOCATOR: IrqSafeHeap = IrqSafeHeap(LockedHeap::empty());

pub fn init() {
    unsafe {
        ALLOCATOR.0.lock().init(&raw mut HEAP as *mut u8, HEAP_SIZE);
    }
}

pub fn size() -> usize {
    HEAP_SIZE
}
