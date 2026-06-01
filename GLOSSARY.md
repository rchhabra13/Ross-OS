# Glossary — every term, plain English

Reference for the article. Grouped by topic; ordered roughly by when you meet them.

## Boot & kernel basics

- **Bare metal** — no OS underneath. Your code runs directly on the CPU. You *are* the OS.
- **`no_std`** — Rust without the standard library (`std` needs an OS). Only `core` is
  available: no heap, no files, no threads. Bare metal requires it.
- **`no_main`** — skip Rust's normal program startup; you define the entry point yourself.
- **Freestanding binary** — a program that needs no OS or runtime to run.
- **Cross-compile** — build on one machine (the Mac) for a different target (`aarch64-unknown-none`).
- **Target triple** — `arch-vendor-os`, e.g. `aarch64-unknown-none`. "none" = no OS = bare metal.
- **Linker script (`linker.ld`)** — tells the linker *where in memory* each section goes and the entry address.
- **Sections** — `.text` (code), `.rodata` (constants), `.data` (initialized variables),
  `.bss` (zero-initialized variables; zeroed at boot to save image size).
- **Entry point (`_start`)** — the first instruction the CPU runs after the image loads.
- **Boot assembly** — minimal asm that sets up the stack and zeroes BSS before Rust can run.
- **Stack pointer (`sp`)** — CPU register pointing at the call stack; must be set before any function call.

## Hardware I/O

- **UART** — serial port hardware; sends/receives bytes one bit at a time over a wire.
  Your console before any display driver exists.
- **PL011** — the specific ARM UART peripheral used by QEMU `virt` and the Raspberry Pi.
- **MMIO (memory-mapped I/O)** — hardware registers appear as memory addresses. Writing to
  `0x0900_0000` sends a byte to the UART.
- **Volatile** — "do not optimize this read/write away." Required for MMIO because the
  compiler can't see hardware side effects.
- **Register (hardware)** — a named slot in a peripheral holding a value (data, status, control).
- **TX/RX FIFO** — transmit/receive queues inside the UART. Check the status flag before
  writing/reading or you drop bytes.
- **Flag register (FR)** — PL011 status register. Bit 5 = TX FIFO full (`TXFF`),
  bit 4 = RX FIFO empty (`RXFE`).

## Memory

- **Physical memory** — actual RAM addresses.
- **Virtual memory** — a fake, per-process address space the CPU maps onto physical RAM. Gives isolation.
- **MMU (Memory Management Unit)** — CPU hardware that translates virtual → physical addresses.
- **Page** — a fixed-size memory chunk (typically 4 KB); the unit of mapping.
- **Page table** — the map from virtual page → physical page, plus permission bits.
- **Frame allocator** — tracks which physical pages are free/used.
- **Heap** — dynamically allocated memory (`Vec`, `Box`, `String`); needs an allocator you write.
  Unlocks Rust's `alloc` crate.

## CPU modes & exceptions (aarch64)

- **Exception level (EL0–EL3)** — privilege rings. **EL0** = user programs, **EL1** = kernel,
  EL2 = hypervisor, EL3 = firmware.
- **Exception** — the CPU stops normal flow and jumps to a handler. Covers faults, syscalls, interrupts.
- **Exception vector table** — a table of handler addresses the CPU jumps to per exception type.
  Must exist before interrupts/syscalls work.
- **Interrupt / IRQ** — hardware signals "attend to me now" (timer, keypress). An asynchronous exception.
- **Generic timer** — ARM's built-in timer; fires an IRQ every N ticks → drives preemption.
- **`svc`** — "supervisor call" instruction; a user program (EL0) executes it to ask the kernel (EL1)
  for service = a system call.
- **`wfe` / `wfi`** — "wait for event / interrupt"; park the CPU in low power until something happens.
- **`mpidr_el1`** — register holding the CPU core ID; used to park extra cores and run on core 0.
- **Context switch** — save one task's registers, load another's. The mechanism behind multitasking.

## Processes & OS services

- **Process / task** — a running program plus its state (registers, stack, memory map).
- **Scheduler** — decides which task runs next. Round-robin is the simplest policy.
- **Preemption** — a timer IRQ forcibly pauses a task to run another (vs. cooperative, where tasks yield voluntarily).
- **System call (syscall)** — a controlled gateway from user code into the kernel (`read`, `write`, our `ai_infer`).
- **User mode (EL0)** — unprivileged; can't touch hardware directly, must go through syscalls. Real protection.
- **ABI (Application Binary Interface)** — the calling contract: which registers carry arguments and return values.
- **ELF** — an executable file format. An ELF loader parses it, maps it into memory, and runs it.
- **Ramdisk** — a filesystem held entirely in RAM; the simplest way to ship files (e.g. model weights).

## AI / inference

- **LLM (large language model)** — a model that predicts the next token of text.
- **Token** — a chunk of text (roughly a word-piece); models operate on tokens, not characters.
- **Tokenizer** — converts text ↔ token IDs.
- **Forward pass** — one run of the model: input tokens → output scores.
- **Logits** — raw per-token scores at the output; you sample one to pick the next token.
- **Transformer** — the model architecture (attention + feedforward layers).
- **Weights / parameters** — the learned numbers. "7B" = 7 billion parameters.
- **Quantization** — storing weights in fewer bits. **INT8** = 8-bit integers (≈4× smaller,
  ≈28× faster than FP32). **INT4** = 4-bit (smaller, but slower on ARM — avoid).
- **Pruning** — deleting unimportant weights to produce a sparse model.
- **`ai_infer` syscall** — *our* primitive: prompt in → generated text out, served by the kernel.

## Phased roadmap (term → phase)

| Phase | Build | Key terms |
|-------|-------|-----------|
| 0 ✅ | Boot + UART print | bare metal, no_std, linker script, BSS, MMIO, UART |
| 1 | UART driver + `println!` + input | TX/RX FIFO, volatile, `fmt::Write` |
| 2 | Exception vectors + timer IRQ | EL0/EL1, vector table, IRQ, generic timer |
| 3 | Physical + virtual memory | frame allocator, page, page table, MMU |
| 4 | Heap allocator | heap, `alloc` |
| 5 | Processes + scheduler | task, context switch, preemption |
| 6 | Syscalls + user mode | `svc`, ABI, EL0 |
| 7 | Shell | command parse/dispatch |
| 8 | Ramdisk + ELF loader | ramdisk, ELF |
| 9 | `ai_infer` syscall ABI | the syscall contract |
| 10 | NL shell + guardrails | structured action, whitelist |
| 11 | Agent OS | agent, tool table |
| 12 | On-device engine | forward pass, tokenizer, logits, INT8 |
| 13 | Port to real Pi 4/5 | kernel8.img, Pi UART |
