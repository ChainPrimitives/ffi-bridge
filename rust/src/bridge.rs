//! # bridge — High-level bridge call helpers
//!
//! Provides [`BridgeCall`], a convenience struct that encapsulates an input
//! buffer, executes a Rust handler, and returns an `FfiResult` — all panic-safe.
//!
//! This is the recommended entry point for implementing new `extern "C"`
//! functions that process an input buffer and return an output buffer.

use crate::errors::{catch_panic, FfiError, FfiResult};
use crate::memory::FfiBuffer;

// ─── BridgeCall ───────────────────────────────────────────────────────────────

/// Convenience builder for a single FFI call.
///
/// # Example
///
/// ```rust,ignore
/// #[no_mangle]
/// pub extern "C" fn my_fn(input: FfiBuffer) -> FfiResult {
///     BridgeCall::new(input).run(|buf| {
///         // buf is the input, return Result<FfiBuffer, FfiError>
///         let val: u32 = unsafe { buf.to_json()? };
///         FfiBuffer::from_json(&(val * 2))
///     })
/// }
/// ```
pub struct BridgeCall {
    input: FfiBuffer,
}

impl BridgeCall {
    /// Create a `BridgeCall` from an input buffer received over FFI.
    pub fn new(input: FfiBuffer) -> Self {
        BridgeCall { input }
    }

    /// Execute `handler` with the input buffer and return an [`FfiResult`].
    ///
    /// The closure is run inside [`catch_panic`], so panics are safely
    /// converted to `FfiErrorCode::Panic` results.
    pub fn run<F>(self, handler: F) -> FfiResult
    where
        F: FnOnce(&FfiBuffer) -> Result<FfiBuffer, FfiError> + std::panic::UnwindSafe,
    {
        let input = self.input;
        catch_panic(move || handler(&input))
    }

    /// Execute `handler` with a JSON-deserialized input value.
    ///
    /// Deserializes `I` from the input buffer's JSON content, then calls
    /// `handler`. The output is serialized back to JSON and returned.
    pub fn run_json<I, O, F>(self, handler: F) -> FfiResult
    where
        I: serde::de::DeserializeOwned + std::panic::UnwindSafe,
        O: serde::Serialize,
        F: FnOnce(I) -> Result<O, FfiError> + std::panic::UnwindSafe,
    {
        let input = self.input;
        catch_panic(move || {
            let value: I = unsafe { input.to_json() }?;
            let output = handler(value)?;
            FfiBuffer::from_json(&output)
        })
    }

    /// Return the raw input buffer (consumes self).
    pub fn into_buffer(self) -> FfiBuffer {
        self.input
    }
}

// ─── FFI-exported health check ────────────────────────────────────────────────

/// Echo the input buffer back as the output — useful for round-trip tests.
///
/// **Exported as:** `ffi_echo`
#[no_mangle]
pub extern "C" fn ffi_echo(input: FfiBuffer) -> FfiResult {
    BridgeCall::new(input).run(|buf| {
        // Copy the slice into a new allocation so the input can be freed independently.
        let bytes = unsafe { buf.as_slice() }.to_vec();
        Ok(FfiBuffer::from_vec(bytes))
    })
}

/// Return a version string buffer.
///
/// **Exported as:** `ffi_version`
#[no_mangle]
pub extern "C" fn ffi_version() -> FfiBuffer {
    FfiBuffer::from_vec(env!("CARGO_PKG_VERSION").as_bytes().to_vec())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ffi_result_free;
    use crate::memory::ffi_buffer_free;

    #[test]
    fn bridge_call_run_ok() {
        let input = FfiBuffer::from_vec(b"ping".to_vec());
        let result = BridgeCall::new(input).run(|buf| {
            let bytes = unsafe { buf.as_slice() }.to_vec();
            assert_eq!(&bytes, b"ping");
            Ok(FfiBuffer::from_vec(b"pong".to_vec()))
        });
        assert!(result.is_ok());
        let payload = unsafe { result.payload.as_slice() };
        assert_eq!(payload, b"pong");
        ffi_result_free(result);
    }

    #[test]
    fn bridge_call_run_json() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Req { n: u32 }
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Res { doubled: u32 }

        let input = FfiBuffer::from_json(&Req { n: 21 }).unwrap();
        let result = BridgeCall::new(input).run_json(|req: Req| {
            Ok(Res { doubled: req.n * 2 })
        });
        assert!(result.is_ok());
        let decoded: Res = unsafe { result.payload.to_json() }.unwrap();
        assert_eq!(decoded, Res { doubled: 42 });
        ffi_result_free(result);
    }

    #[test]
    fn ffi_echo_round_trip() {
        let input = FfiBuffer::from_vec(b"hello echo".to_vec());
        let result = ffi_echo(input);
        assert!(result.is_ok());
        let output = unsafe { result.payload.as_slice() };
        assert_eq!(output, b"hello echo");
        ffi_result_free(result);
    }

    #[test]
    fn ffi_version_returns_version_string() {
        let buf = ffi_version();
        let s = unsafe { buf.as_slice() };
        assert!(!s.is_empty());
        ffi_buffer_free(buf);
    }
}
