# Building a Bare-Metal OS in Rust (aarch64) — Build Log

> Running journal of building a hobby OS kernel from scratch in Rust, virtualized
> in QEMU on Apple Silicon, targeting a real Raspberry Pi 4/5 later. Every step,
> command, and decision recorded here so it can become an article.

## Goal & constraints

- **What:** hobby kernel from scratch (bootloader + kernel), aim for a usable thing.
- **Language:** Rust (`no_std`, bare metal).
- **Dev target:** QEMU, virtualized on the dev machine.
- **Arch:** `aarch64`. The Mac is Apple Silicon (aarch64) *and* the Pi 4/5 is
  aarch64 — same instruction set, so one binary path serves both. Big win.
- **Hardware later:** Raspberry Pi 4/5.

## Environment (as found)

| Tool   | Version |
|--------|---------|
| rustc  | 1.94.1 stable (replaced by nightly below) |
| cargo  | 1.94.1 |
| rustup | 1.29.0 |
| QEMU   | 11.0.1 (installed via `brew install qemu`) |
| nightly | 1.98.0-nightly (6368fd52c 2026-05-29) |

Bare-metal Rust needs **nightly** (for `build-std`-style features and unstable
target handling) plus the `rust-src` component and the `aarch64-unknown-none`
target.

## Step 1 — Toolchain

```sh
brew install qemu
rustup toolchain install nightly --component rust-src
rustup component add llvm-tools-preview --toolchain nightly
rustup target add aarch64-unknown-none --toolchain nightly
```

`aarch64-unknown-none` is a Tier-2 target with a precompiled `core`, so no custom
target JSON or `build-std` is required for a minimal kernel.

QEMU ships the `virt` machine plus `raspi3b` / `raspi4b`. We develop on `virt`
(clean, well-documented MMIO map) and port to `raspi4b` for the real board.

## Step 2 — Project layout

```
os/
├── Cargo.toml            # bin "kernel", panic = "abort"
├── rust-toolchain.toml   # pins nightly + components + target
├── linker.ld             # memory layout, entry at 0x40000000
├── .cargo/config.toml    # default target + qemu runner + linker flag
├── src/
│   ├── boot.s            # assembly entry: park cores, set sp, zero bss
│   └── main.rs           # no_std kernel, UART print
└── run.sh                # build + boot in QEMU, auto-stop
```

### Key decisions

- **`panic = "abort"`** — no unwinding machinery on bare metal.
- **Entry at `0x4000_0000`** — where QEMU `virt` loads a `-kernel` image.
- **PL011 UART at `0x0900_0000`** — the `virt` machine's serial data register;
  writing a byte there shows up on the QEMU console. (On a real Pi the UART is at
  a different address — that's the main thing to change when porting.)
- **Assembly via `global_asm!(include_str!("boot.s"))`** — lets LLVM's built-in
  assembler handle `boot.s`, so no separate `build.rs` / external assembler.

### Boot sequence (`boot.s`)

1. Read `mpidr_el1`; if not core 0, `wfe`-park it (kernel is single-core for now).
2. Set the stack pointer to `__stack_top` (reserved in the linker script).
3. Zero the `.bss` section.
4. `bl kernel_main` (Rust).

## Step 3 — First boot

```sh
cargo build
./run.sh        # build + qemu, auto-stops after 5s
```

> Note: macOS has no `timeout`. `run.sh` backgrounds QEMU, `sleep`s, then `kill`s
> it — the kernel loops forever, so we stop it ourselves.

**Output:**

```
Hello from bare-metal Rust on aarch64!
Kernel alive. Halting.
```

First milestone reached: a Rust kernel boots under QEMU and talks to the world
over a UART. ✅

## The vision — an AI-native OS

Decision: this isn't a plain hobby kernel. The differentiator is **AI as a
first-class OS primitive**, not an app bolted on top. Three layers, built in order:

1. **AI syscall** — `ai_infer(prompt) -> text` lives in the syscall table, like
   `read`/`write`. Any program gets inference for free.
2. **Natural-language shell** — no bash; you state intent, the model returns a
   structured action, the kernel executes it against real syscalls (with guardrails).
3. **Agent OS framing** — process primitive = *agent*; tool calls = syscalls;
   scheduler runs agents. One story: *an OS where the kernel speaks model.*

Inference runs **on-device** (no cloud, no host) — the hard, impressive path.

### Prior art (researched)

| Project | What | Relevance |
|---------|------|-----------|
| **AIOS** (agiresearch, COLM 2025) | LLM-agent OS: agent queries → categorized syscalls (LLM/memory/storage/tool); scheduler dispatches; 2.1× faster agent serving. | Closest concept — but runs *on top of Linux* in Python. Our wedge: do it bare metal. |
| **NaSh** (arXiv 2506.13028) | Guardrails for an LLM-powered natural-language shell. | Reuse guardrail design for layer 2. |
| **Llama2.pi** (Stanford CS224N) | Bare-metal llama2.c on Pi Zero. | Failed (OOM) but gives the numbers below + bootloader path. |
| **lm.rs / llama2.rs** | No-dependency CPU LLM inference in Rust. | Port the forward pass to `no_std` for our engine. |
| **rust-raspberrypi-OS-tutorials** | Canonical aarch64 Rust OS tutorial. | Base reference for kernel internals. |

**Niche we own:** from-scratch bare-metal aarch64 kernel **+** on-device LLM as a
native syscall **+** agent-OS framing, fully self-contained on a Pi. Nobody occupies
all three at once.

### Feasibility — on-device inference numbers (from Llama2.pi)

- **7B is the wrong target.** Llama2-7B INT8 = **7.5 GB**; a single matmul on a Pi
  Zero took **up to 30 min**. Memory *and* compute infeasible for tiny boards.
- **INT8 is the sweet spot:** 28× faster than FP32, ~4× less memory. **Avoid INT4**
  — slower on ARM (no sub-byte instructions, extra casting).
- Pi Zero (512 MB) failed; ~2 GB estimated even for a stripped 7B.
- Pi 4/5 (4–8 GB, quad A72/A76) is far better, but 7B is still impractical.

**Conclusion: target tiny models.** llama2.c ships TinyStories **15M / 42M / 110M /
260M**; INT8 → a few hundred MB, runs fast in QEMU *and* on Pi 4/5. The novelty is
**LLM-as-syscall in a kernel we wrote**, not model size. Start at 15M, scale up as
the kernel gains mmap + multi-core.

## Next steps (planned)

- [x] Proper UART driver: wait on the TX-FIFO flag (`UARTFR`) instead of blind
      writes; add a `core::fmt::Write` impl so `write!`/`println!` work.
- [x] `println!` macro over the UART.
- [x] UART input + echo loop (`getc`, backspace handling).
- [x] Exception vector table (EL1) + basic exception handlers.
- [x] Timer interrupt (generic timer) → first taste of a scheduler tick.
- [x] Physical frame allocator (bump) + `__free_ram_start` linker symbol.
- [x] MMU + identity page tables (L1 1 GiB blocks: device + normal RAM).
- [x] Heap allocator (`GlobalAlloc` via `linked_list_allocator`) → `alloc` crate.
- [x] Processes + a preemptive round-robin scheduler (context switch, timer-driven).
- [ ] Port UART + load address to `raspi4b`; boot on real Pi via `kernel8.img`
      on the SD card.

### Phased roadmap (base OS first, then AI)

- **Phase 0 — base kernel:** UART `println!` + input, exception vectors, timer IRQ,
  physical + virtual memory (MMU), heap, processes + scheduler, syscalls, user mode (EL0), shell.
- **Phase 1 — AI syscall:** define `ai_infer(prompt_ptr,len,out_ptr,cap)` ABI + syscall number.
- **Phase 2 — NL shell:** shell → `ai_infer` → structured action JSON → execute via real syscalls, with whitelist guardrails (NaSh-style).
- **Phase 3 — agent OS:** process = agent w/ tool table; scheduler runs agent loops; message passing.
- **Phase 4 — on-device engine:** port lm.rs-style no-dep forward pass to `no_std`, INT8, TinyStories-15M weights in ramdisk; scale model up later.

## Step 4 — Phase 1: a real UART driver, `println!`, and input

Phase 0 wrote bytes to the UART blindly. That works in QEMU because the emulated
FIFO never fills, but on real hardware (and under load) blind writes drop characters.
Phase 1 builds a proper driver and the print machinery the rest of the OS will use.

### The PL011 register model

The UART is a bank of memory-mapped registers at base `0x0900_0000`. We need two:

| Register | Offset | Use |
|----------|--------|-----|
| `DR` (data)  | `0x00` | write a byte to send; read a byte received |
| `FR` (flags) | `0x18` | status bits |

Two flag bits matter:

- `FR_TXFF` (bit 5) — **transmit FIFO full.** Spin while set before writing `DR`.
- `FR_RXFE` (bit 4) — **receive FIFO empty.** Spin while set before reading `DR`.

That spin-on-flag handshake is the whole difference from Phase 0: we wait for the
hardware to be ready instead of assuming it.

### The driver (`src/uart.rs`)

`Uart` is a zero-sized struct — the *hardware* holds the state, so the type holds
nothing and each method computes the MMIO address directly. All accesses are
`read_volatile`/`write_volatile` so the compiler can't cache or reorder them
(it can't see the hardware's side effects).

```rust
pub fn putc(&self, c: u8) {
    while Self::flags() & FR_TXFF != 0 {}      // wait: TX FIFO has room
    unsafe { ptr::write_volatile(Self::reg(DR), c) }
}
pub fn getc(&self) -> u8 {
    while Self::flags() & FR_RXFE != 0 {}      // wait: a byte arrived
    unsafe { ptr::read_volatile(Self::reg(DR)) }
}
```

Implementing `core::fmt::Write` for `Uart` is what unlocks formatting: `write_str`
loops over the bytes (translating `\n` → `\r\n` for terminals), and Rust's `write!`
machinery turns `{}` formatting into `write_str` calls for free.

### `print!` / `println!`

Two `macro_rules!` macros wrap `write!`/`writeln!` targeting a fresh `Uart`:

```rust
macro_rules! println {
    () => { print!("\n") };
    ($($arg:tt)*) => {{
        use core::fmt::Write as _;
        let _ = writeln!($crate::uart::Uart, $($arg)*);
    }};
}
```

> Gotcha: a `macro_rules!` macro in a binary crate is **not** reachable as
> `$crate::println!` unless you add `#[macro_export]`. The fix here is simpler —
> `println!` calls `print!` by its bare name (textual scope), since both are
> defined before use in the same file.

### Input: the echo loop

`kernel_main` now reads with `getc()` and echoes, handling carriage return
(new prompt) and backspace (`\x08 \x08` erases the last glyph):

```rust
loop {
    match uart.getc() {
        b'\r' | b'\n' => { println!(); print!("echo> "); }
        0x7f | 0x08   => print!("\x08 \x08"),
        c             => uart.putc(c),
    }
}
```

### Verifying input without a keyboard

`run.sh` backgrounds QEMU, so there's no interactive stdin. To test reading, pipe
bytes in and keep stdin open briefly so the kernel has time to echo:

```sh
{ printf 'Hello\rWorld\r'; sleep 2; } \
  | qemu-system-aarch64 -M virt -cpu cortex-a72 -nographic \
      -kernel target/aarch64-unknown-none/debug/kernel
```

**Output:**

```
=== AI-OS kernel ===
Phase 1: PL011 UART driver online.
fmt check: 2 + 2 = 4
Type something; I echo it. (Ctrl-A X to quit QEMU)
echo> Hello
echo> World
echo>
```

Milestone: **formatted output** (`println!` with `{}`) **and live input** both work.
The kernel is now interactive — every later phase prints and reads through this driver. ✅

> Macro cleanup: the macros moved to `src/macros.rs` with `#[macro_export]`, the
> proper fix for the Phase 1 gotcha. They're now callable as `crate::println!`
> from any module (the IRQ handler uses this).

## Step 5 — Phase 2: exception vectors + a timer interrupt

Until now the CPU ran one straight line of code. Phase 2 adds the machinery for the
hardware to *interrupt* that flow — the foundation under preemptive multitasking,
syscalls, and fault handling. Goal: a generic-timer IRQ that prints a tick once a second.

### Background: exceptions and exception levels

On aarch64, anything that diverts the CPU — a fault, a syscall, a hardware interrupt —
is an **exception**. When one fires, the CPU jumps to an address in the **exception
vector table** pointed to by `VBAR_EL1`.

aarch64 has four privilege rings, **EL0**–**EL3** (EL0 = user, EL1 = kernel). We
confirmed where QEMU drops us by reading `CurrentEL`:

```
Running at EL1.
```

EL1 is exactly what we want for a kernel, so no level-switching is needed.

### The vector table (`exceptions.s`)

The table is **16 entries × 0x80 bytes, 0x800-aligned**. The 16 slots are the cross
product of {source: current-EL-SP0, current-EL-SPx, lower-EL-aarch64, lower-EL-aarch32}
× {type: Synchronous, IRQ, FIQ, SError}. We run the kernel on `SP_ELx`, so hardware
interrupts arrive at the **Current-EL/SPx/IRQ** slot (offset `0x280`); that's the only
one wired to a real handler (`el1_irq`), the rest go to a catch-all that prints the
syndrome and halts.

Each handler must **save the registers** it might clobber before calling Rust and
restore them after, because it's borrowing whatever context was running:

```asm
el1_irq:
    SAVE_CONTEXT          // push x0..x30
    bl  rust_irq_handler
    RESTORE_CONTEXT       // pop x0..x30
    eret                  // return to interrupted code
```

`eret` (exception return) resumes the interrupted instruction using the banked
`ELR_EL1`/`SPSR_EL1` the CPU saved on entry.

### Getting the interrupt to fire (`interrupts.rs`)

Two pieces of hardware cooperate: the **GIC** (Generic Interrupt Controller) routes
device interrupts to the CPU, and the **generic timer** is the device. On QEMU `virt`
the GIC is **GICv2** (distributor at `0x0800_0000`, CPU interface at `0x0801_0000`),
and the non-secure EL1 physical timer is **interrupt ID 30** (a PPI).

`init()` does four things:

```rust
// 1. Install the table.
asm!("msr vbar_el1, {}", in(reg) vbar);
// 2. GIC: enable distributor, enable IRQ 30, open priority mask, enable CPU iface.
write32(GICD_BASE + GICD_CTLR, 1);
write32(GICD_BASE + GICD_ISENABLER, 1 << 30);
write32(GICC_BASE + GICC_PMR, 0xFF);
write32(GICC_BASE + GICC_CTLR, 1);
// 3. Timer: fire one second (CNTFRQ ticks) from now, then enable it.
set_timer_countdown(cntfrq());
enable_timer();                       // CNTP_CTL_EL0 = ENABLE
// 4. Unmask IRQs in PSTATE.
asm!("msr daifclr, #2");
```

> **DAIF** is the PSTATE interrupt-mask field (Debug, SError, IRQ, FIQ). IRQs are
> masked at reset; `daifclr, #2` clears the I bit to let them through. Forget this
> and the timer fires but the CPU ignores it.

The handler acknowledges the interrupt, does its work, re-arms the timer, and signals
end-of-interrupt — the GIC won't deliver the next one until `EOIR` is written:

```rust
fn rust_irq_handler() {
    let iar = read32(GICC_BASE + GICC_IAR);     // acknowledge -> interrupt ID
    if (iar & 0x3FF) == TIMER_IRQ {
        let n = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
        crate::println!("tick {}", n);
        set_timer_countdown(cntfrq());          // re-arm for next second
    }
    write32(GICC_BASE + GICC_EOIR, iar);        // end of interrupt
}
```

`kernel_main` now just `wfi`s (wait-for-interrupt) in a loop; the CPU sleeps and the
timer wakes it each second.

### Result

```
=== AI-OS kernel ===
Phase 2: exception vectors + timer IRQ.
Running at EL1.
IRQs unmasked. Timer armed (1 Hz). Sleeping...
tick 1
tick 2
tick 3
```

Milestone: the **first interrupt**. The CPU now leaves the main flow, runs a handler,
and returns — exactly the mechanism Phase 5 reuses to preempt tasks and Phase 6 reuses
for syscalls. ✅

### What was learned

- Exceptions, exception levels (EL0–EL3), the 16-entry vector table and its layout.
- Why handlers must save/restore context, and what `eret` does.
- The GIC ↔ device split, GICv2 register flow (`ISENABLER`/`PMR`/`IAR`/`EOIR`).
- The ARM generic timer (`CNTFRQ`/`CNTP_TVAL`/`CNTP_CTL`) and the DAIF mask.

## Step 6 — Phase 3: physical + virtual memory (the MMU)

The "hard wall" of hobby OS dev. Two jobs: hand out physical RAM (a **frame
allocator**), and turn on the **MMU** so the CPU translates virtual → physical
addresses through **page tables**. Get the page tables wrong and the machine dies
the instant the MMU enables, with no output — so the mapping must be exactly right
before flipping the switch.

### Part A — the frame allocator (`memory.rs`)

Start simple: a **bump allocator**. Everything past the kernel image + stack is free
RAM; hand it out one 4 KiB **frame** at a time by advancing a pointer. The linker
script exports where free RAM begins:

```ld
. = ALIGN(4096);
__free_ram_start = .;     /* frame allocator owns RAM from here up */
```

`alloc_frame()` bumps the pointer, zeroes the frame, and returns its physical address.
No freeing yet — that arrives with the heap in Phase 4. RAM is 1 GiB at `0x4000_0000`
(we now pass `-m 1G` to QEMU; the run script does this for you).

### Part B — enabling the MMU (`mmu.rs`)

**Address-translation model.** With a 4 KiB granule and a 39-bit virtual address, the
top translation level is **L1, where each entry maps a 1 GiB block**. That's coarse —
and perfect for a first identity map. One L1 table, two entries:

| Index | Range | Memory type | Covers |
|-------|-------|-------------|--------|
| `[0]` | `0x0000_0000`–`0x4000_0000` | **Device** | GIC `0x0800_0000`, UART `0x0900_0000` |
| `[1]` | `0x4000_0000`–`0x8000_0000` | **Normal** | kernel, stack, page tables, free RAM |

**Identity map** = VA equals PA. That's the trick that keeps the currently-running
code valid the moment translation turns on: every address it's already using maps to
itself.

**Descriptor bits.** A valid L1 entry with bits[1:0]=`0b01` is a *block*. We OR in the
output address plus: `AF` (access flag — a fault if unset), shareability, and an
`AttrIndx` selecting a memory type from `MAIR_EL1`:

```rust
L1[0] = 0x0000_0000 | VALID | AF | ATTR_DEVICE;             // device
L1[1] = 0x4000_0000 | VALID | AF | SH_INNER | ATTR_NORMAL;  // normal RAM
```

**Why device vs normal matters:** MMIO (UART/GIC) must be *Device* memory — no
caching, no reordering, no speculation. Map it as Normal cacheable and your register
writes vanish into a cache line and never reach the hardware. `MAIR_EL1` defines the
two types; `AttrIndx` in each descriptor picks one:

```rust
let mair = (0xFF << 8) | 0x00;   // attr1 = Normal WB, attr0 = Device-nGnRnE
```

**The control registers, in order:**

```rust
asm!("msr mair_el1, {}", in(reg) mair);     // memory-type table
asm!("msr tcr_el1,  {}", in(reg) tcr);      // T0SZ=25 (39-bit), 4KiB granule, WB, inner-shareable
asm!("msr ttbr0_el1,{}", in(reg) l1);       // where the table lives
asm!("dsb ish");                            // table writes visible…
asm!("tlbi vmalle1"); asm!("dsb ish"); asm!("isb");  // …flush stale TLB, synchronize
// flip the switch:
sctlr |= (1<<0) | (1<<2) | (1<<12);         // M (MMU) | C (D-cache) | I (I-cache)
asm!("msr sctlr_el1, {}", in(reg) sctlr);
asm!("isb");
```

The barriers aren't optional: `dsb` guarantees the table stores landed in memory
before the MMU reads them, `tlbi` drops any stale translations, and `isb` makes the
CPU re-fetch under the new regime.

### Result

```
frame alloc: 0x4008a000, 0x4008b000 (1073168384 bytes free)
MMU enabled: SCTLR.M=1
vmem r/w @ 0x4008c000 = 0xc0ffee
IRQs unmasked, timer armed (1 Hz). Sleeping...
tick 1
tick 2
tick 3
```

Three things prove the mapping is correct: the kernel **keeps printing** after enable
(UART device mapping good), a write/read of a fresh frame returns `0xc0ffee` (normal
RAM mapping good), and **ticks keep coming** (GIC device mapping + IRQs survive paging).
The hard wall is behind us. ✅

### What was learned

- Frame allocation; bump vs. free-list; the `__free_ram_start` linker symbol.
- aarch64 translation: granule, VA size/T0SZ, translation levels, 1 GiB block entries.
- Identity mapping and why it keeps running code alive across enable.
- Block-descriptor bits (`VALID`/`AF`/`SH`/`AttrIndx`) and `MAIR` memory types.
- Device vs. Normal memory — and why MMIO **must** be Device.
- `TCR_EL1`/`TTBR0_EL1`/`SCTLR_EL1`, and the `dsb`/`tlbi`/`isb` barrier dance.

## Step 7 — Phase 4: a heap, and the `alloc` crate

So far the kernel had no dynamic memory: `core` only, no `Vec`/`Box`/`String`.
A heap fixes that. Rust's `alloc` crate provides those types but needs *you* to
supply a **global allocator** — a type implementing `GlobalAlloc` (alloc + dealloc).

### Bump allocator vs. real heap

The Phase 3 frame allocator is bump-only: it never frees. That's fine for page
tables, useless for a heap where `Vec` grows and drops constantly. So the heap needs
a free-capable allocator. Rather than hand-roll a free list, we use the well-worn
`linked_list_allocator` crate — it tracks free blocks in an intrusive linked list
and supports real `dealloc`. (It's `no_std`, so it drops straight into a bare-metal
kernel.)

### Wiring it up (`heap.rs`)

Give the allocator a fixed region — a 1 MiB static buffer in `.bss` (zeroed, in
identity-mapped Normal RAM) — and register it as *the* global allocator:

```rust
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init() {
    unsafe { ALLOCATOR.lock().init(&raw mut HEAP as *mut u8, HEAP_SIZE); }
}
```

`#[global_allocator]` is the hook: once a `GlobalAlloc` is registered, every
`Box::new`/`Vec::push`/`format!` routes through it. `LockedHeap` wraps the allocator
in a spinlock (one core today, but `GlobalAlloc` requires `Sync`).

Then in `main.rs`: `extern crate alloc;`, call `heap::init()`, and the standard
collections work:

```rust
let mut v: Vec<u32> = Vec::new();
for i in 0..5 { v.push(i * i); }
let boxed = Box::new(0xABCDu32);
let s = alloc::format!("{} squares + box {:#x}", v.len(), *boxed);
```

> Gotcha: `Box<u32>` doesn't implement `LowerHex`, so `{:#x}` needs the dereferenced
> value `*boxed`, not the box.

### Result

```
heap (1024 KiB): vec=[0, 1, 4, 9, 16] -> 5 squares + box 0xabcd
```

`Vec` grew on the heap, `Box` allocated and printed, `format!` built a `String`.
The kernel now has dynamic memory — every later phase (process tables, the shell's
parsed input, the model's tensors) leans on this. ✅

### What was learned

- `GlobalAlloc` / `#[global_allocator]` — how Rust connects `alloc` to your memory.
- Why a heap needs free support (vs. the bump frame allocator).
- Pulling a `no_std` crate into a bare-metal kernel.
- `extern crate alloc;` and the `Vec`/`Box`/`String`/`format!` surface.

## Step 8 — Phase 5: processes + a preemptive scheduler

This is the "it's a real OS" milestone. Up to now the kernel ran a single thread
of control. Now it runs **multiple kernel threads** and switches between them on a
timer — no thread has to cooperate or yield. That's *preemptive multitasking*.

### What a context switch actually is

A "task" (kernel thread) is just: a stack + a saved set of CPU registers. To freeze
a running thread and resume another, you only need to save the registers the calling
convention says a function must preserve across a call — the AAPCS **callee-saved**
set: `x19–x28`, the frame pointer `x29`, the link register `x30`, and `SP`. Everything
else (`x0–x18`) the compiler already treats as clobberable at any call site, so it
doesn't need saving. That's 13 values.

`cpu_switch(prev, next)` (`switch.s`) stores those 13 from the CPU into `*prev`,
loads 13 from `*next`, and `ret`s — which jumps to the freshly-restored `x30`. The
thread that was frozen weeks of wall-clock ago resumes as if the function call just
returned.

```asm
cpu_switch:
    mov     x2, sp
    stp     x19, x20, [x0, #0]
    ...
    stp     x29, x30, [x0, #80]
    str     x2,       [x0, #96]   // save prev
    ldp     x19, x20, [x1, #0]
    ...
    ldp     x29, x30, [x1, #80]
    ldr     x2,       [x1, #96]
    mov     sp, x2                // load next
    ret                           // -> restored x30
```

The `Context` struct in `task.rs` mirrors those offsets exactly (`x19@0 … x30@88,
sp@96`); if they drift, you jump to garbage.

### Starting a brand-new thread: the trampoline

A frozen thread resumes by *returning* from `cpu_switch`. But a thread that has never
run has nothing to return to. The trick: hand-craft its initial context so the first
`ret` lands on a small **trampoline**, with the real entry point stashed in `x19`:

```asm
task_trampoline:
    msr     daifclr, #2     // unmask IRQs so the new thread is itself preemptible
    blr     x19             // call entry()
1:  wfe                     // if entry returns, park
    b       1b
```

`spawn()` allocates a 64 KiB stack, sets `context.x19 = entry`, `context.x30 =
task_trampoline`, `context.sp = stack_top`, and pushes the `Box<Task>` into the
scheduler. First time it's scheduled, `cpu_switch` "returns" into the trampoline,
which unmasks interrupts and calls `entry`.

### Driving it from the timer

`schedule()` is dead simple — round-robin to the next task and switch:

```rust
let prev = s.current;
let next = (prev + 1) % s.tasks.len();
s.current = next;
cpu_switch(&mut s.tasks[prev].context, &s.tasks[next].context);
```

The preemption comes from calling it inside the timer IRQ. The handler now bumps the
tick count, re-arms the timer (bumped to **100 Hz** for snappy 10 ms slices), writes
`EOIR`, then calls `task::schedule()`. The running task never asked to be switched —
the hardware interrupt did it.

### The bug that cost the most: a shared `ELR_EL1`

First run faulted instantly: `ESR=0x…86000005` (instruction abort, translation fault)
with `ELR=0x10_0000_0000` — a garbage PC. The switches into the new tasks worked; the
crash came when control rotated **back to `kmain`** and it tried to `eret`.

Cause: the exception entry/exit macros saved `x0–x30` but **not** `ELR_EL1`/`SPSR_EL1`.
Normally fine — a handler erets immediately and `ELR_EL1` still holds the interrupted
PC. But here the handler *context-switches away* before erets. While `kmain`'s handler
frame sat dormant, other tasks took their own timer IRQs, each **overwriting the single
shared `ELR_EL1`**. By the time `kmain` finally erets, `ELR_EL1` held stale garbage.

Fix: make every exception frame self-contained — save `ELR_EL1` + `SPSR_EL1` on entry,
restore them before `eret` (frame grew 256 → 272 bytes):

```asm
// SAVE_CONTEXT, after the GP regs:
    mrs     x9,  elr_el1
    mrs     x10, spsr_el1
    stp     x9,  x10, [sp, #16 * 16]
// RESTORE_CONTEXT, before the GP regs:
    ldp     x9,  x10, [sp, #16 * 16]
    msr     elr_el1,  x9
    msr     spsr_el1, x10
```

Lesson: the moment a handler can switch tasks, *all* per-exception CPU state — not
just the GP registers — has to live in that task's frame.

### Making shared state safe under preemption

The instant a timer IRQ can switch tasks at *any* instruction, every piece of state
shared between tasks becomes a race. Two bit us in review:

- **The UART.** `println!` emits a line byte-by-byte. Preempt a task mid-line and
  the next task's bytes splice into it — garbled output.
- **The heap.** `linked_list_allocator`'s `LockedHeap` is a spin `Mutex`. If a task
  is frozen *holding* that lock and the IRQ handler (or the next task) then allocates,
  it spins on a lock the frozen holder can't release.

Fix: a tiny critical-section primitive (`sync.rs`) that masks IRQs and restores the
*previous* DAIF (so it nests):

```rust
pub fn irq_save() -> u64 {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif); asm!("msr daifset, #2"); }
    daif
}
pub fn irq_restore(daif: u64) { unsafe { asm!("msr daif, {}", in(reg) daif) } }
pub fn without_preempt<R>(f: impl FnOnce() -> R) -> R {
    let s = irq_save(); let r = f(); irq_restore(s); r
}
```

`println!` now emits each line inside `without_preempt`, and the `#[global_allocator]`
is wrapped so every `alloc`/`dealloc` runs with IRQs masked — the heap lock is never
held across a context switch. With IRQs masked only for the brief critical section,
preemption everywhere else is unaffected.

### Result

Two demo threads each busy-loop (≈2M `nop`s, **never** voluntarily yielding), then
allocate a `Vec` on the **shared heap** and print a beat. Pure preemption makes them
interleave — and the heap stays consistent across the switches:

```
[B] beat 24 (sum 276)
[A] beat 24 (sum 276)
[B] beat 25 (sum 300)
[A] beat 25 (sum 300)
[B] beat 26 (sum 325)
```

Neither task calls `yield`. The 100 Hz timer rotates `kmain → A → B → kmain → …`,
and both tasks hammer the shared heap every beat with no corruption or deadlock. ✅

### What was learned

- A thread is just a stack + the 13 callee-saved registers; switching = save 13, load 13.
- Bootstrapping a never-run thread via a hand-built context + a trampoline.
- Preemption = call the scheduler from the timer IRQ; tasks need not cooperate.
- Exception handlers that switch tasks **must** save/restore `ELR_EL1`/`SPSR_EL1`,
  because that state is shared and gets clobbered by intervening exceptions.
- Preemption turns *any* shared state (UART, heap lock) into a race; guard it with a
  nest-safe IRQ-masked critical section (`sync::without_preempt`), not a spinlock —
  a spinlock held across a context switch is exactly the hang you're avoiding.

## Reproduce

```sh
git clone <repo> && cd os
rustup show          # nightly auto-selected via rust-toolchain.toml
./run.sh
```
