//! # callback — Named callback registry
//!
//! Provides a thread-safe, global registry of named callbacks.
//! Callbacks can be registered either from Rust (typed closure) or from
//! the C/Go side via `ffi_register_callback` (C function pointer).
//!
//! ## Design
//!
//! The registry is a `Mutex<HashMap<String, CallbackFn>>` initialized lazily
//! via `once_cell`. All exported functions are panic-safe via [`catch_panic`].
//!
//! ## Thread safety
//!
//! The registry is protected by a `std::sync::Mutex`. If a thread panics while
//! holding the lock, the lock becomes poisoned and all subsequent operations
//! return [`FfiError::LockPoisoned`].

use std::collections::HashMap;
use std::os::raw::c_char;
use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::errors::{catch_panic, FfiError, FfiResult};
use crate::memory::FfiBuffer;
use crate::types::cstr_to_string;

// ─── Registry ─────────────────────────────────────────────────────────────────

type CallbackFn = Box<dyn Fn(FfiBuffer) -> FfiResult + Send + Sync>;

static CALLBACKS: Lazy<Mutex<HashMap<String, CallbackFn>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ─── Rust-native API ──────────────────────────────────────────────────────────

/// Register a named callback from Rust code.
///
/// The closure receives an [`FfiBuffer`] input and must return an [`FfiResult`].
///
/// # Thread safety
///
/// This function acquires the global callback registry lock.
/// Returns `Err` if the lock is poisoned.
pub fn register_callback<F>(name: &str, f: F) -> Result<(), FfiError>
where
    F: Fn(FfiBuffer) -> FfiResult + Send + Sync + 'static,
{
    let mut guard = CALLBACKS.lock().map_err(|_| FfiError::LockPoisoned)?;
    guard.insert(name.to_string(), Box::new(f));
    Ok(())
}

/// Remove a callback by name from Rust code.
///
/// Returns `Ok(true)` if the callback was removed, `Ok(false)` if not found.
pub fn unregister_callback(name: &str) -> Result<bool, FfiError> {
    let mut guard = CALLBACKS.lock().map_err(|_| FfiError::LockPoisoned)?;
    Ok(guard.remove(name).is_some())
}

/// Return the number of currently registered callbacks.
pub fn callback_count() -> usize {
    CALLBACKS.lock().map(|g| g.len()).unwrap_or(0)
}

// ─── FFI-exported API ─────────────────────────────────────────────────────────

/// Register a C function pointer as a named callback.
///
/// `name` must be a valid null-terminated UTF-8 string.
/// Returns `0` on success, `-1` if `name` is null or not valid UTF-8,
/// `-2` if the registry lock is poisoned.
///
/// **Exported as:** `ffi_register_callback`
///
/// # Safety
///
/// `name` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ffi_register_callback(
    name: *const c_char,
    cb: extern "C" fn(FfiBuffer) -> FfiResult,
) -> i32 {
    let name_str = match cstr_to_string(name) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let wrapped = move |buf: FfiBuffer| cb(buf);
    match CALLBACKS.lock() {
        Ok(mut guard) => {
            guard.insert(name_str, Box::new(wrapped));
            0
        }
        Err(_) => -2,
    }
}

/// Invoke a registered callback by name.
///
/// Returns `FFI_ERR_NOT_FOUND` if no callback with the given name is registered.
/// Returns `FFI_ERR_PANIC` if the callback panics.
///
/// **Exported as:** `ffi_invoke_callback`
///
/// # Safety
///
/// `name` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ffi_invoke_callback(name: *const c_char, input: FfiBuffer) -> FfiResult {
    // We must resolve the name before entering catch_panic (CStr isn't UnwindSafe).
    let name_str = match cstr_to_string(name) {
        Ok(s) => s,
        Err(e) => return FfiResult::err(e),
    };

    catch_panic(move || {
        let guard = CALLBACKS.lock().map_err(|_| FfiError::LockPoisoned)?;

        let cb = guard
            .get(&name_str)
            .ok_or_else(|| FfiError::NotFound(name_str.clone()))?;

        let result = cb(input);

        if result.is_ok() {
            // Extract the payload, leaving the (empty) error_message as-is.
            // FfiResult has no Drop, so the struct fields are just stack values;
            // we take ownership directly without mem::forget.
            let FfiResult { payload, .. } = result;
            Ok(payload)
        } else {
            Err(FfiError::Unknown(format!(
                "callback '{}' returned error code {:?}",
                name_str, result.error_code
            )))
        }
    })
}

/// Remove a registered callback by name.
///
/// Returns `0` if removed, `-1` if not found, `-2` if lock is poisoned.
///
/// **Exported as:** `ffi_unregister_callback`
///
/// # Safety
///
/// `name` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ffi_unregister_callback(name: *const c_char) -> i32 {
    let name_str = match cstr_to_string(name) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    match CALLBACKS.lock() {
        Ok(mut guard) => {
            if guard.remove(&name_str).is_some() {
                0
            } else {
                -1
            }
        }
        Err(_) => -2,
    }
}

/// Return the number of registered callbacks.
///
/// **Exported as:** `ffi_callback_count`
#[no_mangle]
pub extern "C" fn ffi_callback_count() -> usize {
    callback_count()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{ffi_result_free, FfiErrorCode};

    fn unique_name(prefix: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        format!("{prefix}_{}", COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn register_and_invoke_rust_callback() {
        let name = unique_name("test_echo");
        register_callback(&name, |buf| {
            let bytes = unsafe { buf.as_slice() }.to_vec();
            FfiResult::ok(FfiBuffer::from_vec(bytes))
        })
        .unwrap();

        let input = FfiBuffer::from_vec(b"test payload".to_vec());
        let result = unsafe {
            let c_name = std::ffi::CString::new(name.as_str()).unwrap();
            ffi_invoke_callback(c_name.as_ptr(), input)
        };
        assert!(result.is_ok());
        let slice = unsafe { result.payload.as_slice() };
        assert_eq!(slice, b"test payload");
        ffi_result_free(result);
    }

    #[test]
    fn invoke_unknown_callback_returns_not_found() {
        let result = unsafe {
            let c_name = std::ffi::CString::new("__nonexistent_callback__").unwrap();
            ffi_invoke_callback(c_name.as_ptr(), FfiBuffer::null())
        };
        assert_eq!(result.error_code, FfiErrorCode::NotFound);
        ffi_result_free(result);
    }

    #[test]
    fn unregister_removes_callback() {
        let name = unique_name("test_unregister");
        register_callback(&name, |buf| FfiResult::ok(buf)).unwrap();

        let removed = unregister_callback(&name).unwrap();
        assert!(removed);

        let not_removed = unregister_callback(&name).unwrap();
        assert!(!not_removed);
    }

    #[test]
    fn callback_count_tracks_registrations() {
        let name = unique_name("test_count");
        let before = callback_count();
        register_callback(&name, |buf| FfiResult::ok(buf)).unwrap();
        assert_eq!(callback_count(), before + 1);
        unregister_callback(&name).unwrap();
        assert_eq!(callback_count(), before);
    }
}
