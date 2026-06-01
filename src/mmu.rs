//! Enable the MMU with a minimal identity mapping.
//!
//! 4 KiB granule, 39-bit VA → top level is L1 where each entry maps a **1 GiB block**.
//! A single L1 table identity-maps everything we touch with two entries:
//!   - [0]  0x0000_0000..0x4000_0000  Device  (GIC @ 0x0800_0000, UART @ 0x0900_0000)
//!   - [1]  0x4000_0000..0x8000_0000  Normal  (kernel image, stack, page tables, free RAM)
//!
//! Because the mapping is identity (VA == PA), the running code keeps working the
//! instant the MMU turns on — provided every region it uses is in the table.

use core::arch::asm;

#[repr(C, align(4096))]
struct Table([u64; 512]);

/// The single L1 translation table. In `.bss` (RAM), so it's identity-mapped.
static mut L1: Table = Table([0; 512]);

// Block-descriptor bits (lower attributes).
const VALID: u64 = 1 << 0; // bits[1:0]=0b01 at L1 ⇒ block descriptor
const AF: u64 = 1 << 10; // Access Flag (fault if 0 on access)
const SH_INNER: u64 = 0b11 << 8; // Inner shareable (for Normal memory)
const ATTR_DEVICE: u64 = 0 << 2; // AttrIndx = 0 → MAIR attr0 (Device)
const ATTR_NORMAL: u64 = 1 << 2; // AttrIndx = 1 → MAIR attr1 (Normal WB)

pub fn init() {
    unsafe {
        let l1 = &raw mut L1;

        // [0] device, [1] normal RAM.
        (*l1).0[0] = 0x0000_0000 | VALID | AF | ATTR_DEVICE;
        (*l1).0[1] = 0x4000_0000 | VALID | AF | SH_INNER | ATTR_NORMAL;

        // MAIR_EL1: attr0 = Device-nGnRnE (0x00), attr1 = Normal write-back (0xFF).
        let mair: u64 = (0xFF << 8) | 0x00;
        asm!("msr mair_el1, {}", in(reg) mair);

        // TCR_EL1: T0SZ=25 (39-bit VA), 4 KiB granule (TG0=00),
        // inner/outer write-back cacheable, inner shareable, 32-bit PA (IPS=000).
        let tcr: u64 = 25                 // T0SZ
            | (0b01 << 8)                 // IRGN0 = WB
            | (0b01 << 10)                // ORGN0 = WB
            | (0b11 << 12);               // SH0   = inner shareable
        asm!("msr tcr_el1, {}", in(reg) tcr);

        // Point translation at our table.
        asm!("msr ttbr0_el1, {}", in(reg) l1 as u64);

        // Make table writes visible, flush TLB, synchronize.
        asm!("dsb ish");
        asm!("tlbi vmalle1");
        asm!("dsb ish");
        asm!("isb");

        // Turn it on: SCTLR_EL1.M (MMU) + C (D-cache) + I (I-cache).
        let mut sctlr: u64;
        asm!("mrs {}, sctlr_el1", out(reg) sctlr);
        sctlr |= (1 << 0) | (1 << 2) | (1 << 12);
        asm!("msr sctlr_el1, {}", in(reg) sctlr);
        asm!("isb");
    }
}

/// True if SCTLR_EL1.M is set.
pub fn enabled() -> bool {
    let sctlr: u64;
    unsafe { asm!("mrs {}, sctlr_el1", out(reg) sctlr) };
    sctlr & 1 != 0
}
