//! Device access is isolated from portable capture parsing and future control logic.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

pub const MOZA_VENDOR_ID: u16 = 0x346e;
pub const MOZA_STALK_PRODUCT_ID: u16 = 0x0024;
