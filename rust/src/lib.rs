//! # ffi-bridge
//!
//! Memory-safe Go↔Rust FFI boundary helpers.
//!
//! This crate provides:
//! - [`FfiBuffer`] — heap-allocated byte buffer with explicit ownership semantics
//! - [`FfiString`] — UTF-8 string safe for crossing the FFI boundary
//! - [`FfiResult`] — C-ABI result type conveying either a payload or an error
//! - [`FfiError`] / [`FfiErrorCode`] — rich error taxonomy matching the C header
//! - Callback registry — register and invoke named callbacks across the FFI boundary
//! - [`catch_panic`] — converts Rust panics to [`FfiResult`] so they never cross the ABI
//!
//! ## Safety contract
//!
//! * All buffers and strings allocated by this crate **must** be freed by the
//!   corresponding `ffi_*_free` function exported from this crate.
//! * Panics are caught at every `extern "C"` boundary via [`catch_panic`].
//! * No Rust type with a `Drop` impl is allowed to cross the FFI boundary as a value;
//!   only `repr(C)` POD structs may do so.

pub mod bridge;
pub mod callback;
pub mod errors;
pub mod memory;
pub mod types;

pub use bridge::*;
pub use callback::{
    callback_count, ffi_callback_count, ffi_invoke_callback, ffi_register_callback,
    ffi_unregister_callback, register_callback, unregister_callback,
};
pub use errors::{catch_panic, ffi_result_free, FfiError, FfiErrorCode, FfiResult};
pub use memory::{
    ffi_buffer_alloc, ffi_buffer_free, ffi_string_alloc, ffi_string_free, FfiBuffer, FfiString,
};
pub use types::*;
