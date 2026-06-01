# Ross-OS

A hobby operating-system kernel written from scratch in Rust for **aarch64** —
no libraries, no Linux, just the bare metal. It boots on the QEMU `virt` machine
and is designed to port to a real **Raspberry Pi 4/5**.

The long-term goal is an **AI-native OS**: once the base kernel is solid, AI becomes
a first-class kernel primitive (an `ai_infer` syscall, a natural-language shell, and
an agent-OS framing) with **on-device inference** — no cloud.

Every step is documented in detail in [`ARTICLE.md`](ARTICLE.md), a build journal
written alongside the code.

## Status

The base kernel is built up phase by phase. Done so far:

| Phase | What landed |
|------:|-------------|
| 1 | PL011 UART driver, `print!`/`println!`, interactive input echo |
| 2 | EL1 exception vector table, GICv2, ARM generic-timer IRQ |
| 3 | Physical frame allocator (bump) + MMU with identity page tables |
| 4 | Kernel heap (`linked_list_allocator`) → the `alloc` crate works |
| 5 | Kernel threads + **preemptive** round-robin scheduler |

Next: syscalls + EL0 user mode, then a shell, then the AI layers.

### Phase 5 highlight — preemptive multitasking

Tasks are kernel threads switched by a timer IRQ at 100 Hz — no thread has to yield.
Two demo threads busy-loop (never cooperate), allocate on a shared heap, and still
interleave cleanly:

```
[B] beat 24 (sum 276)
[A] beat 24 (sum 276)
[B] beat 25 (sum 300)
[A] beat 25 (sum 300)
```

Shared state (UART, heap lock) is guarded by an IRQ-masked critical section so a
spinlock is never held across a context switch.

## Architecture

```
src/
  boot.s         Entry point: park secondary cores, set SP, zero BSS, call kernel_main
  main.rs        Kernel entry; wires the phases together; demo tasks
  uart.rs        PL011 UART MMIO driver + core::fmt::Write
  macros.rs      print!/println! (IRQ-safe)
  exceptions.s   EL1 vector table + context save/restore (incl. ELR/SPSR)
  interrupts.rs  GICv2 + generic timer; IRQ handler drives the scheduler
  memory.rs      Physical frame allocator (bump)
  mmu.rs         MMU enable + identity map (1 GiB L1 blocks: device + RAM)
  heap.rs        Global allocator (IRQ-safe wrapper over LockedHeap)
  task.rs        Task structs + round-robin scheduler
  switch.s       cpu_switch (callee-saved context switch) + task_trampoline
  sync.rs        IRQ-masked critical sections (without_preempt)
linker.ld        Load at 0x4000_0000; stack + __free_ram_start symbols
```

See [`GLOSSARY.md`](GLOSSARY.md) for every term used.

## Build & run

Requires the Rust nightly toolchain (auto-selected via `rust-toolchain.toml`) and
`qemu-system-aarch64`.

```sh
# macOS
brew install qemu

# build + boot in QEMU (auto-stops after 5s since the kernel loops)
./run.sh

# or longer
./run.sh 8
```

`run.sh` builds the `kernel` binary for `aarch64-unknown-none` and boots it on the
QEMU `virt` machine with serial wired to stdout.

## Target hardware

Developed on Apple Silicon (aarch64) → same architecture as the Pi, so one binary
path. Porting to a real Raspberry Pi 4/5 (different UART base address, `kernel8.img`
on the SD card) is a later phase.

## License

MIT
