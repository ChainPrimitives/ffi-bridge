//! # types — Shared FFI-safe type aliases and helpers
//!
//! This module re-exports the primitive FFI types and defines higher-level
//! type aliases and conversion utilities used throughout the crate.

use crate::errors::FfiError;
use crate::memory::FfiBuffer;

// ─── Type aliases ─────────────────────────────────────────────────────────────

/// A fallible FFI operation: either produces an [`FfiBuffer`] or an [`FfiError`].
pub type FfiBufferResult = Result<FfiBuffer, FfiError>;

// ─── BridgeValue ──────────────────────────────────────────────────────────────

/// A strongly-typed wrap around `FfiBuffer` that carries a JSON-serialized value.
///
/// Provides idiomatic Rust serialization/deserialization helpers while
/// remaining FFI-safe when consumed as a raw `FfiBuffer`.
pub struct BridgeValue {
    inner: FfiBuffer,
}

impl BridgeValue {
    /// Serialize a value to JSON and wrap it.
    pub fn new<T: serde::Serialize>(value: &T) -> Result<Self, FfiError> {
        Ok(BridgeValue { inner: FfiBuffer::from_json(value)? })
    }

    /// Deserialize the contained JSON into `T`.
    pub fn decode<T: serde::de::DeserializeOwned>(&self) -> Result<T, FfiError> {
        unsafe { self.inner.to_json() }
    }

    /// Consume the `BridgeValue` and return the underlying buffer.
    pub fn into_buffer(self) -> FfiBuffer {
        self.inner
    }
}

// ─── Utility functions ────────────────────────────────────────────────────────

/// Validate that a raw pointer is non-null, returning [`FfiError::NullPointer`] if it is.
///
/// # Safety
///
/// The pointer must be valid for reads if it is non-null.
#[inline]
pub unsafe fn check_not_null<T>(ptr: *const T) -> Result<*const T, FfiError> {
    if ptr.is_null() {
        Err(FfiError::NullPointer)
    } else {
        Ok(ptr)
    }
}

/// Validate that a mutable pointer is non-null, returning [`FfiError::NullPointer`] if it is.
///
/// # Safety
///
/// The pointer must be valid for writes if it is non-null.
#[inline]
pub unsafe fn check_not_null_mut<T>(ptr: *mut T) -> Result<*mut T, FfiError> {
    if ptr.is_null() {
        Err(FfiError::NullPointer)
    } else {
        Ok(ptr)
    }
}

/// Read a C-string pointer into an owned `String`.
///
/// # Safety
///
/// `ptr` must point to a valid null-terminated UTF-8 sequence.
pub unsafe fn cstr_to_string(ptr: *const std::os::raw::c_char) -> Result<String, FfiError> {
    if ptr.is_null() {
        return Err(FfiError::NullPointer);
    }
    std::ffi::CStr::from_ptr(ptr)
        .to_str()
        .map(|s| s.to_string())
        .map_err(|e| FfiError::InvalidUtf8(e.to_string()))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[test]
    fn bridge_value_round_trip() {
        let p = Point { x: 10, y: -5 };
        let bv = BridgeValue::new(&p).unwrap();
        let decoded: Point = bv.decode().unwrap();
        assert_eq!(decoded, p);
        // Manually free the inner buffer since BridgeValue has no Drop
        crate::memory::ffi_buffer_free(bv.into_buffer());
    }

    #[test]
    fn check_not_null_with_null_ptr() {
        let ptr: *const u8 = std::ptr::null();
        let result = unsafe { check_not_null(ptr) };
        assert!(matches!(result, Err(FfiError::NullPointer)));
    }

    #[test]
    fn check_not_null_with_valid_ptr() {
        let value: u8 = 42;
        let ptr: *const u8 = &value;
        let result = unsafe { check_not_null(ptr) };
        assert!(result.is_ok());
    }
}
