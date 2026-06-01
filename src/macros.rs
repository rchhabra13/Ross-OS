//! `print!` / `println!` over the UART.
//!
//! `#[macro_export]` puts these at the crate root so they're callable from any
//! module as `crate::println!` (and unqualified within the crate). This is the
//! proper fix for the bin-crate macro-path gotcha hit in Phase 1.

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::uart::_print(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println {
    () => {{ $crate::print!("\n"); }};
    ($($arg:tt)*) => {{
        $crate::uart::_print(format_args!("{}\n", format_args!($($arg)*)));
    }};
}
