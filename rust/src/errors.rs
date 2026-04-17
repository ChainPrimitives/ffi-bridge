//! # errors — Error taxonomy and FFI result type
//!
//! Provides [`FfiErrorCode`] (C-ABI enum), [`FfiError`] (rich Rust error),
//! [`FfiResult`] (C-ABI result wrapper), and the
//! [`catch_panic`] utility that prevents panics from crossing the FFI boundary.
//!
//! ## Key principle
//!
//! **Panics must never cross the FFI boundary.** Undefined behaviour results if
//! a Rust panic unwinds into C or Go. Every `extern "C"` function in this crate
//! wraps its body in [`catch_panic`].

use crate::memory::{FfiBuffer, FfiString};

// ─── FfiErrorCode ─────────────────────────────────────────────────────────────

/// Numeric error codes that cross the FFI boundary.
///
/// Matches the `FfiErrorCode` enum in `shared/ffi.h`.
/// All values are stable — codes are never renumbered or removed.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiErrorCode {
    Ok = 0,
    NullPointer = 1,
    BufferTooSmall = 2,
    InvalidUtf8 = 3,
    Serialization = 4,
    Panic = 5,
    Timeout = 6,
    NotFound = 7,
    LockPoisoned = 8,
    Unknown = 99,
}

// ─── FfiError ─────────────────────────────────────────────────────────────────

/// Rich Rust-side error type that can be converted to an [`FfiErrorCode`] + message.
#[derive(Debug)]
pub enum FfiError {
    NullPointer,
    BufferTooSmall { needed: usize, available: usize },
    InvalidUtf8(String),
    Serialization(String),
    Panic(String),
    Timeout,
    NotFound(String),
    LockPoisoned,
    Unknown(String),
}

impl FfiError {
    /// Map to the corresponding [`FfiErrorCode`].
    pub fn code(&self) -> FfiErrorCode {
        match self {
            Self::NullPointer => FfiErrorCode::NullPointer,
            Self::BufferTooSmall { .. } => FfiErrorCode::BufferTooSmall,
            Self::InvalidUtf8(_) => FfiErrorCode::InvalidUtf8,
            Self::Serialization(_) => FfiErrorCode::Serialization,
            Self::Panic(_) => FfiErrorCode::Panic,
            Self::Timeout => FfiErrorCode::Timeout,
            Self::NotFound(_) => FfiErrorCode::NotFound,
            Self::LockPoisoned => FfiErrorCode::LockPoisoned,
            Self::Unknown(_) => FfiErrorCode::Unknown,
        }
    }

    /// Build a human-readable error message.
    pub fn message(&self) -> String {
        match self {
            Self::NullPointer => "null pointer received".into(),
            Self::BufferTooSmall { needed, available } => {
                format!("buffer too small: need {needed} bytes, have {available}")
            }
            Self::InvalidUtf8(ctx) => format!("invalid UTF-8 in {ctx}"),
            Self::Serialization(detail) => format!("serialization error: {detail}"),
            Self::Panic(msg) => format!("panic caught at FFI boundary: {msg}"),
            Self::Timeout => "operation timed out".into(),
            Self::NotFound(name) => format!("not found: {name}"),
            Self::LockPoisoned => "mutex lock is poisoned".into(),
            Self::Unknown(detail) => format!("unknown error: {detail}"),
        }
    }
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for FfiError {}

// ─── FfiResult ────────────────────────────────────────────────────────────────

/// C-ABI result type.
///
/// On success: `error_code == FfiErrorCode::Ok`, `error_message` is empty,
/// `payload` contains the result bytes.
///
/// On error: `error_code != 0`, `error_message` carries a UTF-8 string,
/// `payload` is zeroed.
///
/// **Always** call [`ffi_result_free`] when you are done with the result,
/// regardless of `error_code`.
#[repr(C)]
pub struct FfiResult {
    pub error_code: FfiErrorCode,
    pub error_message: FfiString,
    pub payload: FfiBuffer,
}

impl FfiResult {
    /// Construct a successful result carrying `payload`.
    pub fn ok(payload: FfiBuffer) -> Self {
        FfiResult {
            error_code: FfiErrorCode::Ok,
            error_message: FfiString::null(),
            payload,
        }
    }

    /// Construct an error result.
    pub fn err(error: FfiError) -> Self {
        let msg = FfiString::new(&error.message());
        FfiResult {
            error_code: error.code(),
            error_message: msg,
            payload: FfiBuffer::null(),
        }
    }

    /// Returns `true` if this result represents success.
    #[inline]
    pub fn is_ok(&self) -> bool {
        self.error_code == FfiErrorCode::Ok
    }
}

/// Free an [`FfiResult`] and all heap memory it owns.
///
/// **Must** be called exactly once per `FfiResult` received from this crate.
///
/// If you take ownership of `payload` before this call, zero `result.payload`
/// first to prevent a double-free.
///
/// **Exported as:** `ffi_result_free`
#[no_mangle]
pub extern "C" fn ffi_result_free(result: FfiResult) {
    // SAFETY: caller guarantees single-ownership.
    unsafe {
        result.error_message.dealloc();
        result.payload.dealloc();
    }
}

// ─── catch_panic ──────────────────────────────────────────────────────────────

/// Run `f` and convert any Rust panic into an [`FfiResult`] error.
///
/// This is the **critical safety wrapper** used by every `extern "C"` function
/// in this crate. A panic unwind crossing the FFI boundary is undefined behaviour;
/// `catch_panic` ensures it never happens.
///
/// # Example
///
/// ```rust,ignore
/// #[no_mangle]
/// pub extern "C" fn my_ffi_fn(buf: FfiBuffer) -> FfiResult {
///     catch_panic(|| {
///         // ... do work, return Result<FfiBuffer, FfiError>
///         Ok(buf)
///     })
/// }
/// ```
pub fn catch_panic<F>(f: F) -> FfiResult
where
    F: FnOnce() -> Result<FfiBuffer, FfiError> + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(Ok(buf)) => FfiResult::ok(buf),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic_payload) => {
            let msg = panic_payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| panic_payload.downcast_ref::<String>().map(|s| s.as_str()))
                .unwrap_or("unknown panic payload");
            FfiResult::err(FfiError::Panic(msg.to_string()))
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_result_ok_is_ok() {
        let buf = FfiBuffer::new(8);
        let result = FfiResult::ok(buf);
        assert!(result.is_ok());
        assert_eq!(result.error_code, FfiErrorCode::Ok);
        ffi_result_free(result);
    }

    #[test]
    fn ffi_result_err_carries_code_and_message() {
        let result = FfiResult::err(FfiError::NullPointer);
        assert!(!result.is_ok());
        assert_eq!(result.error_code, FfiErrorCode::NullPointer);
        let msg = unsafe { result.error_message.as_str() };
        assert!(msg.contains("null pointer"));
        ffi_result_free(result);
    }

    #[test]
    fn catch_panic_ok_path() {
        let result = catch_panic(|| Ok(FfiBuffer::new(4)));
        assert!(result.is_ok());
        ffi_result_free(result);
    }

    #[test]
    fn catch_panic_err_path() {
        let result = catch_panic(|| Err::<FfiBuffer, _>(FfiError::Timeout));
        assert_eq!(result.error_code, FfiErrorCode::Timeout);
        ffi_result_free(result);
    }

    #[test]
    fn catch_panic_catches_panic() {
        let result = catch_panic(|| {
            panic!("intentional test panic");
        });
        assert_eq!(result.error_code, FfiErrorCode::Panic);
        let msg = unsafe { result.error_message.as_str() };
        assert!(msg.contains("intentional test panic"));
        ffi_result_free(result);
    }

    #[test]
    fn ffi_error_buffer_too_small_message() {
        let e = FfiError::BufferTooSmall {
            needed: 100,
            available: 10,
        };
        assert!(e.message().contains("100"));
        assert!(e.message().contains("10"));
    }
}
