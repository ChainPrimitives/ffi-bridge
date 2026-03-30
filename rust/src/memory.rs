//! # memory — FFI-safe heap buffers and strings
//!
//! Provides [`FfiBuffer`] and [`FfiString`]: the two heap-allocated primitive
//! types that cross the Go↔Rust boundary.
//!
//! ## Ownership model
//!
//! ```text
//!   Rust allocates → caller (Go) reads → Rust frees
//! ```
//!
//! * Rust is always the allocator. Go **never** allocates these types directly.
//! * The Go side must call the matching `ffi_*_free` exported function when it
//!   is done with a value. Rust's allocator is invoked; Go's GC is not involved.
//! * `FfiBuffer` and `FfiString` are `repr(C)` structs of raw pointers + sizes.
//!   They have **no Drop impl** — they cannot be safely dropped by Rust without
//!   explicit deallocation, which is intentional: the Go side controls lifetime.

use std::alloc::{alloc, dealloc, Layout};
use std::ptr;

// ─── FfiBuffer ────────────────────────────────────────────────────────────────

/// FFI-safe byte buffer with explicit ownership semantics.
///
/// `data` points to a heap allocation of `capacity` bytes managed by
/// Rust's global allocator. `len` is the number of initialized bytes.
///
/// # Safety
///
/// * This type does **not** implement `Drop`. Callers must call
///   [`ffi_buffer_free`] to release the memory.
/// * Do not copy this struct without transferring ownership—double-free will result.
#[repr(C)]
pub struct FfiBuffer {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

// SAFETY: raw pointer is Send-able since ownership of the allocation is
// transferred across the FFI boundary one-at-a-time.
unsafe impl Send for FfiBuffer {}
unsafe impl Sync for FfiBuffer {}

impl FfiBuffer {
    /// Returns an `FfiBuffer` with all fields zero (null data pointer).
    ///
    /// Represents an empty / absent buffer. Safe to pass to [`ffi_buffer_free`].
    #[inline]
    pub fn null() -> Self {
        FfiBuffer { data: ptr::null_mut(), len: 0, capacity: 0 }
    }

    /// Allocate a new buffer of `capacity` bytes.
    ///
    /// Returns [`FfiBuffer::null`] if `capacity == 0`.
    ///
    /// # Panics
    ///
    /// Panics if the allocator returns a null pointer (OOM).
    pub fn new(capacity: usize) -> Self {
        if capacity == 0 {
            return Self::null();
        }
        let layout = Layout::array::<u8>(capacity).expect("capacity overflow");
        // SAFETY: layout is non-zero and valid.
        let data = unsafe { alloc(layout) };
        if data.is_null() {
            // Global allocator contract: null means OOM.
            panic!("ffi_buffer_alloc: out of memory (capacity={})", capacity);
        }
        FfiBuffer { data, len: 0, capacity }
    }

    /// Consume a `Vec<u8>` and wrap it as an `FfiBuffer`.
    ///
    /// `std::mem::forget` is used to prevent Vec from running its destructor;
    /// the caller must call [`ffi_buffer_free`] when done.
    pub fn from_vec(mut vec: Vec<u8>) -> Self {
        vec.shrink_to_fit();
        let buf = FfiBuffer {
            data: vec.as_mut_ptr(),
            len: vec.len(),
            capacity: vec.capacity(),
        };
        std::mem::forget(vec);
        buf
    }

    /// Serialize `value` to JSON and store it in a new `FfiBuffer`.
    pub fn from_json<T: serde::Serialize>(value: &T) -> Result<Self, crate::errors::FfiError> {
        let json = serde_json::to_vec(value)
            .map_err(|e| crate::errors::FfiError::Serialization(e.to_string()))?;
        Ok(Self::from_vec(json))
    }

    /// Return the initialized bytes as a slice.
    ///
    /// # Safety
    ///
    /// The caller must ensure `self.data` is valid for `self.len` bytes and
    /// that no concurrent write is occurring.
    #[inline]
    pub unsafe fn as_slice(&self) -> &[u8] {
        if self.data.is_null() || self.len == 0 {
            return &[];
        }
        std::slice::from_raw_parts(self.data, self.len)
    }

    /// Deserialize the buffer's contents as JSON into type `T`.
    ///
    /// # Safety
    ///
    /// Same requirements as [`as_slice`](Self::as_slice).
    pub unsafe fn to_json<T: serde::de::DeserializeOwned>(
        &self,
    ) -> Result<T, crate::errors::FfiError> {
        serde_json::from_slice(self.as_slice())
            .map_err(|e| crate::errors::FfiError::Serialization(e.to_string()))
    }

    /// Deallocate the buffer.
    ///
    /// # Safety
    ///
    /// Must only be called once. After this call `self.data` is dangling.
    pub unsafe fn dealloc(self) {
        if self.data.is_null() || self.capacity == 0 {
            return;
        }
        let layout = Layout::array::<u8>(self.capacity).expect("capacity overflow");
        dealloc(self.data, layout);
    }
}

/// Allocate an [`FfiBuffer`] of `capacity` bytes.
///
/// **Exported as:** `ffi_buffer_alloc`
#[no_mangle]
pub extern "C" fn ffi_buffer_alloc(capacity: usize) -> FfiBuffer {
    FfiBuffer::new(capacity)
}

/// Free an [`FfiBuffer`] previously allocated by this crate.
///
/// Safe to call on a zeroed / null buffer.
///
/// **Exported as:** `ffi_buffer_free`
#[no_mangle]
pub extern "C" fn ffi_buffer_free(buf: FfiBuffer) {
    // SAFETY: caller guarantees single-ownership.
    unsafe { buf.dealloc() };
}

// ─── FfiString ────────────────────────────────────────────────────────────────

/// FFI-safe UTF-8 string.
///
/// `data` points to a heap-allocated byte array of `len` bytes.
/// The bytes are valid UTF-8 but are **not** necessarily null-terminated
/// beyond `len`; always use `len` to determine the string length.
#[repr(C)]
pub struct FfiString {
    pub data: *mut u8,
    pub len: usize,
}

unsafe impl Send for FfiString {}
unsafe impl Sync for FfiString {}

impl FfiString {
    /// Returns an empty `FfiString` (null data pointer, len=0).
    #[inline]
    pub fn null() -> Self {
        FfiString { data: ptr::null_mut(), len: 0 }
    }

    /// Allocate and copy a UTF-8 string.
    pub fn new(s: &str) -> Self {
        let bytes = s.as_bytes();
        if bytes.is_empty() {
            return Self::null();
        }
        let layout = Layout::array::<u8>(bytes.len()).expect("string too large");
        let data = unsafe { alloc(layout) };
        if data.is_null() {
            panic!("ffi_string_alloc: out of memory");
        }
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len()) };
        FfiString { data, len: bytes.len() }
    }

    /// Convert to a `&str`.
    ///
    /// # Safety
    ///
    /// `self.data` must be valid for `self.len` bytes of valid UTF-8.
    pub unsafe fn as_str(&self) -> &str {
        if self.data.is_null() || self.len == 0 {
            return "";
        }
        let bytes = std::slice::from_raw_parts(self.data as *const u8, self.len);
        std::str::from_utf8_unchecked(bytes)
    }

    /// Deallocate the string's buffer.
    ///
    /// # Safety
    ///
    /// Must only be called once.
    pub unsafe fn dealloc(self) {
        if self.data.is_null() || self.len == 0 {
            return;
        }
        let layout = Layout::array::<u8>(self.len).expect("string too large");
        dealloc(self.data, layout);
    }
}

/// Allocate and copy a UTF-8 string of `len` bytes starting at `str`.
///
/// **Exported as:** `ffi_string_alloc`
///
/// # Safety
///
/// `str` must be valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ffi_string_alloc(str: *const u8, len: usize) -> FfiString {
    if str.is_null() || len == 0 {
        return FfiString::null();
    }
    let bytes = std::slice::from_raw_parts(str, len);
    let s = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return FfiString::null(), // Caller passed non-UTF-8 — return null
    };
    FfiString::new(s)
}

/// Free an [`FfiString`] previously allocated by this crate.
///
/// **Exported as:** `ffi_string_free`
#[no_mangle]
pub extern "C" fn ffi_string_free(str: FfiString) {
    // SAFETY: caller guarantees single-ownership.
    unsafe { str.dealloc() };
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_new_zero_capacity_is_null() {
        let buf = FfiBuffer::new(0);
        assert!(buf.data.is_null());
        assert_eq!(buf.len, 0);
        assert_eq!(buf.capacity, 0);
    }

    #[test]
    fn buffer_alloc_and_free() {
        let buf = FfiBuffer::new(64);
        assert!(!buf.data.is_null());
        assert_eq!(buf.capacity, 64);
        assert_eq!(buf.len, 0);
        ffi_buffer_free(buf);
    }

    #[test]
    fn buffer_from_vec_round_trip() {
        let data = b"hello ffi".to_vec();
        let buf = FfiBuffer::from_vec(data);
        assert_eq!(buf.len, 9);
        let slice = unsafe { buf.as_slice() };
        assert_eq!(slice, b"hello ffi");
        ffi_buffer_free(buf);
    }

    #[test]
    fn buffer_from_json_round_trip() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Msg { value: u32 }

        let msg = Msg { value: 42 };
        let buf = FfiBuffer::from_json(&msg).unwrap();
        let decoded: Msg = unsafe { buf.to_json() }.unwrap();
        assert_eq!(decoded, msg);
        ffi_buffer_free(buf);
    }

    #[test]
    fn string_null_on_zero_len() {
        let s = FfiString::new("");
        assert!(s.data.is_null());
    }

    #[test]
    fn string_roundtrip() {
        let s = FfiString::new("hello world");
        assert_eq!(s.len, 11);
        let back = unsafe { s.as_str() };
        assert_eq!(back, "hello world");
        ffi_string_free(s);
    }
}
