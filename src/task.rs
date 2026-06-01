//! Kernel threads + a round-robin scheduler.
//!
//! Tasks are kernel threads: they share the kernel address space and run at EL1,
//! each with its own stack. A task is frozen/resumed by `cpu_switch`, which swaps
//! the callee-saved registers + SP. Scheduling is **preemptive**: the timer IRQ
//! (Phase 2) calls `schedule()`, so tasks need not yield to share the CPU.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

const STACK_SIZE: usize = 64 * 1024;

/// Saved callee-saved state. Field order/offsets must match `switch.s`.
#[repr(C)]
#[derive(Default)]
struct Context {
    x19: u64, x20: u64, x21: u64, x22: u64,
    x23: u64, x24: u64, x25: u64, x26: u64,
    x27: u64, x28: u64, x29: u64, x30: u64, // x30 = LR
    sp: u64,
}

struct Task {
    context: Context,
    _stack: Vec<u8>, // owns the thread's stack
    #[allow(dead_code)]
    name: &'static str,
}

struct Scheduler {
    tasks: Vec<Box<Task>>, // Box → stable address for context pointers
    current: usize,
}

static mut SCHED: Option<Scheduler> = None;

extern "C" {
    fn cpu_switch(prev: *mut Context, next: *const Context);
    fn task_trampoline();
}

/// Register the currently-running kernel code as task 0. Its context is filled in
/// the first time it's switched *out*.
pub fn init() {
    let task0 = Box::new(Task {
        context: Context::default(),
        _stack: Vec::new(),
        name: "kmain",
    });
    unsafe {
        SCHED = Some(Scheduler {
            tasks: vec![task0],
            current: 0,
        });
    }
}

/// Create a new kernel thread that begins at `entry`.
pub fn spawn(name: &'static str, entry: extern "C" fn()) {
    let mut stack = vec![0u8; STACK_SIZE];
    let top = (unsafe { stack.as_mut_ptr().add(STACK_SIZE) } as u64) & !0xF;

    let mut context = Context::default();
    context.x19 = entry as usize as u64; // trampoline does `blr x19`
    context.x30 = task_trampoline as *const () as u64; // first resume lands here
    context.sp = top;

    let task = Box::new(Task { context, _stack: stack, name });
    unsafe {
        #[allow(static_mut_refs)]
        SCHED.as_mut().unwrap().tasks.push(task);
    }
}

/// Round-robin to the next task. Called from the timer IRQ (IRQs already masked
/// in the handler, so this is not re-entrant).
pub fn schedule() {
    unsafe {
        #[allow(static_mut_refs)]
        let s = match SCHED.as_mut() {
            Some(s) => s,
            None => return,
        };
        if s.tasks.len() < 2 {
            return;
        }
        let prev = s.current;
        let next = (prev + 1) % s.tasks.len();
        s.current = next;

        let prev_ctx = &mut s.tasks[prev].context as *mut Context;
        let next_ctx = &s.tasks[next].context as *const Context;
        cpu_switch(prev_ctx, next_ctx);
    }
}
