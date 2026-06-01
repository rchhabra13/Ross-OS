//! PL011 UART driver for the QEMU `virt` machine.
//!
//! The PL011 exposes a bank of memory-mapped registers starting at `UART0_BASE`.
//! We only need two: the data register (`DR`) to read/write a byte, and the flag
//! register (`FR`) to check whether the transmit FIFO is full or the receive FIFO
//! is empty before touching `DR`.

use core::fmt;
use core::ptr;

/// Base address of UART0 on the QEMU `virt` machine.
const UART0_BASE: usize = 0x0900_0000;

/// Register offsets from the base.
const DR: usize = 0x00; // Data register: read = received byte, write = transmit byte.
const FR: usize = 0x18; // Flag register: status bits.

/// Flag-register bits we care about.
const FR_RXFE: u8 = 1 << 4; // Receive FIFO empty.
const FR_TXFF: u8 = 1 << 5; // Transmit FIFO full.

/// Zero-sized handle to the UART. The hardware is the state, so the struct holds
/// nothing; every method computes the MMIO address directly.
pub struct Uart;

impl Uart {
    #[inline]
    fn reg(off: usize) -> *mut u8 {
        (UART0_BASE + off) as *mut u8
    }

    #[inline]
    fn flags() -> u8 {
        // Volatile: the value changes in hardware, so the compiler must re-read it.
        unsafe { ptr::read_volatile(Self::reg(FR)) }
    }

    /// Send one byte, blocking until the transmit FIFO has room.
    pub fn putc(&self, c: u8) {
        while Self::flags() & FR_TXFF != 0 {}
        unsafe { ptr::write_volatile(Self::reg(DR), c) }
    }

    /// Receive one byte, blocking until one arrives.
    /// Returns in the shell phase; kept ready here.
    #[allow(dead_code)]
    pub fn getc(&self) -> u8 {
        while Self::flags() & FR_RXFE != 0 {}
        unsafe { ptr::read_volatile(Self::reg(DR)) }
    }

    /// Receive one byte if available, else `None` (non-blocking).
    /// Used once we have interrupts/scheduling (Phase 2+).
    #[allow(dead_code)]
    pub fn try_getc(&self) -> Option<u8> {
        if Self::flags() & FR_RXFE != 0 {
            None
        } else {
            Some(unsafe { ptr::read_volatile(Self::reg(DR)) })
        }
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            // Terminals expect CRLF; translate bare LF to CR+LF.
            if b == b'\n' {
                self.putc(b'\r');
            }
            self.putc(b);
        }
        Ok(())
    }
}

/// Backing function for `print!`/`println!`. The whole format is emitted with IRQs
/// masked so a preemptive context switch can't interleave another task's bytes
/// mid-line (the UART is shared, unlocked state).
pub fn _print(args: fmt::Arguments) {
    crate::sync::without_preempt(|| {
        let _ = fmt::Write::write_fmt(&mut Uart, args);
    });
}
