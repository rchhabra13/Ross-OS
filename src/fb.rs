//! Linear framebuffer via QEMU **ramfb**.
//!
//! The `virt` machine has no framebuffer by default. `-device ramfb` adds one
//! whose scanout address is set by the guest through the **fw_cfg** interface:
//! we allocate a chunk of RAM, then DMA a small config struct (address, format,
//! width/height/stride — all big-endian) into the `etc/ramfb` fw_cfg file. From
//! then on QEMU continuously scans out whatever we write into that RAM.
//!
//! Pixel format is XRGB8888 (DRM fourcc `XR24`): each pixel is a little-endian
//! `u32` of the form `0x00RRGGBB`.

use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

// --- fw_cfg MMIO (QEMU virt) ---
const FWCFG_BASE: usize = 0x0902_0000;
const FWCFG_DATA: usize = FWCFG_BASE + 0x00; // sequential data port (byte reads)
const FWCFG_SEL: usize = FWCFG_BASE + 0x08; // selector (write, big-endian u16)
const FWCFG_DMA: usize = FWCFG_BASE + 0x10; // DMA address (write, big-endian u64)

const FW_CFG_FILE_DIR: u16 = 0x0019;

// fw_cfg DMA control bits.
const DMA_SELECT: u32 = 0x08;
const DMA_WRITE: u32 = 0x10;

// DRM_FORMAT_XRGB8888.
const FOURCC_XR24: u32 = 0x3432_5258;

pub const WIDTH: usize = 1024;
pub const HEIGHT: usize = 768;
const STRIDE: usize = WIDTH * 4;

static FB: AtomicUsize = AtomicUsize::new(0);

/// The ramfb config the guest hands to QEMU. All fields big-endian. Exactly 28
/// bytes are transferred (`length` below), matching QEMU's `RAMFBCfg`.
#[repr(C)]
struct RamfbCfg {
    addr: u64,
    fourcc: u32,
    flags: u32,
    width: u32,
    height: u32,
    stride: u32,
}

/// fw_cfg DMA descriptor. All fields big-endian.
#[repr(C)]
struct DmaAccess {
    control: u32,
    length: u32,
    address: u64,
}

#[inline]
fn select(key: u16) {
    unsafe { ptr::write_volatile(FWCFG_SEL as *mut u16, key.to_be()) };
}

#[inline]
fn read_byte() -> u8 {
    unsafe { ptr::read_volatile(FWCFG_DATA as *const u8) }
}

fn read_bytes(buf: &mut [u8]) {
    for b in buf.iter_mut() {
        *b = read_byte();
    }
}

/// Scan the fw_cfg file directory for `etc/ramfb` and return its selector key.
fn find_ramfb_selector() -> Option<u16> {
    select(FW_CFG_FILE_DIR);
    let mut cnt = [0u8; 4];
    read_bytes(&mut cnt);
    let count = u32::from_be_bytes(cnt);

    for _ in 0..count.min(256) {
        // struct FWCfgFile { u32 size; u16 select; u16 reserved; char name[56]; }
        let mut entry = [0u8; 64];
        read_bytes(&mut entry);
        let sel = u16::from_be_bytes([entry[4], entry[5]]);
        let name = &entry[8..64];
        if name.starts_with(b"etc/ramfb\0") {
            return Some(sel);
        }
    }
    None
}

/// Allocate the framebuffer and point ramfb at it. Returns false if ramfb is
/// absent (kernel booted without `-device ramfb`).
pub fn init() -> bool {
    let sel = match find_ramfb_selector() {
        Some(s) => s,
        None => return false,
    };

    let fb = match crate::memory::alloc_contig(STRIDE * HEIGHT) {
        Some(p) => p,
        None => return false,
    };

    let cfg = RamfbCfg {
        addr: (fb as u64).to_be(),
        fourcc: FOURCC_XR24.to_be(),
        flags: 0,
        width: (WIDTH as u32).to_be(),
        height: (HEIGHT as u32).to_be(),
        stride: (STRIDE as u32).to_be(),
    };

    // DMA-write the 28-byte config into the ramfb fw_cfg file (SELECT | WRITE).
    let dma = DmaAccess {
        control: (((sel as u32) << 16) | DMA_SELECT | DMA_WRITE).to_be(),
        length: 28u32.to_be(),
        address: (&cfg as *const RamfbCfg as u64).to_be(),
    };
    unsafe {
        ptr::write_volatile(FWCFG_DMA as *mut u64, (&dma as *const DmaAccess as u64).to_be());
    }

    FB.store(fb, Ordering::Release);
    true
}

#[inline]
fn base() -> usize {
    FB.load(Ordering::Acquire)
}

/// Set one pixel. `color` is `0x00RRGGBB`.
#[inline]
pub fn put_pixel(x: usize, y: usize, color: u32) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let p = base();
    if p == 0 {
        return;
    }
    unsafe { ptr::write_volatile((p + y * STRIDE + x * 4) as *mut u32, color) };
}

/// Fill a rectangle (clipped to the screen).
pub fn fill_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    let x1 = (x + w).min(WIDTH);
    let y1 = (y + h).min(HEIGHT);
    for yy in y..y1 {
        for xx in x..x1 {
            put_pixel(xx, yy, color);
        }
    }
}

/// Paint the whole screen one color.
pub fn clear(color: u32) {
    fill_rect(0, 0, WIDTH, HEIGHT, color);
}

/// Glyph width in source pixels (before scaling).
pub const GLYPH_W: usize = 8;

/// Draw one ASCII char at pixel (x, y), magnified by `scale`. Background is left
/// untouched (transparent), so text composites over whatever is already drawn.
pub fn draw_char(x: usize, y: usize, c: u8, color: u32, scale: usize) {
    let glyph = crate::font::FONT8X8[(c as usize) & 0x7F];
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            if (bits >> col) & 1 != 0 {
                fill_rect(x + col * scale, y + row * scale, scale, scale, color);
            }
        }
    }
}

/// Draw a string left-to-right starting at (x, y). No wrapping.
pub fn draw_str(x: usize, y: usize, s: &str, color: u32, scale: usize) {
    let mut cx = x;
    for &b in s.as_bytes() {
        draw_char(cx, y, b, color, scale);
        cx += GLYPH_W * scale;
    }
}
