//! Integration tests for ffi-bridge.
//!
//! These tests exercise the full FFI surface area end-to-end,
//! including buffer allocation, JSON round-trips, callback registration,
//! and error propagation.

use ffi_bridge::*;

// ─── Buffer round-trips ───────────────────────────────────────────────────────

#[test]
fn alloc_and_free_via_exported_fns() {
    let buf = ffi_buffer_alloc(128);
    assert!(!buf.data.is_null());
    assert_eq!(buf.capacity, 128);
    ffi_buffer_free(buf);
}

#[test]
fn buffer_null_is_safe_to_free() {
    let buf = FfiBuffer::null();
    ffi_buffer_free(buf); // Must not crash or leak
}

#[test]
fn buffer_from_vec_and_back() {
    let original = b"integration test data".to_vec();
    let buf = FfiBuffer::from_vec(original.clone());
    let recovered = unsafe { buf.as_slice() }.to_vec();
    assert_eq!(recovered, original);
    ffi_buffer_free(buf);
}

// ─── JSON round-trips ─────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct Request {
    method: String,
    params: Vec<i64>,
}

#[allow(dead_code)]
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct Response {
    result: i64,
    ok: bool,
}

#[test]
fn json_round_trip_through_ffi_buffer() {
    let req = Request {
        method: "sum".into(),
        params: vec![1, 2, 3, 4, 5],
    };
    let buf = FfiBuffer::from_json(&req).expect("serialize");
    let decoded: Request = unsafe { buf.to_json() }.expect("deserialize");
    assert_eq!(decoded, req);
    ffi_buffer_free(buf);
}

// ─── FfiResult ────────────────────────────────────────────────────────────────

#[test]
fn result_ok_flow() {
    let payload = FfiBuffer::from_vec(b"success".to_vec());
    let result = FfiResult::ok(payload);
    assert!(result.is_ok());
    assert_eq!(result.error_code, FfiErrorCode::Ok);
    ffi_result_free(result);
}

#[test]
fn result_err_flow() {
    let result = FfiResult::err(FfiError::Timeout);
    assert!(!result.is_ok());
    assert_eq!(result.error_code, FfiErrorCode::Timeout);
    let msg = unsafe { result.error_message.as_str() };
    assert!(msg.contains("timed out"));
    ffi_result_free(result);
}

// ─── catch_panic ──────────────────────────────────────────────────────────────

#[test]
fn catch_panic_prevents_unwind() {
    let result = catch_panic(|| panic!("do not let this escape!"));
    assert_eq!(result.error_code, FfiErrorCode::Panic);
    let msg = unsafe { result.error_message.as_str() };
    assert!(msg.contains("do not let this escape!"));
    ffi_result_free(result);
}

// ─── ffi_echo ─────────────────────────────────────────────────────────────────

#[test]
fn ffi_echo_copies_data() {
    let input = FfiBuffer::from_vec(b"round trip echo".to_vec());
    let result = ffi_echo(input);
    assert!(result.is_ok());
    let output = unsafe { result.payload.as_slice() };
    assert_eq!(output, b"round trip echo");
    ffi_result_free(result);
}

// ─── Callback registry ────────────────────────────────────────────────────────

/// Generate a unique callback name using an atomic counter.
/// Avoids timestamp collisions when tests run in parallel on CI.
fn unique_name(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!("{prefix}_{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[test]
fn callback_register_and_invoke() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let call_count = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&call_count);

    let name = unique_name("integ_cb");

    register_callback(&name, move |buf| {
        counter.fetch_add(1, Ordering::SeqCst);
        let bytes = unsafe { buf.as_slice() }.to_vec();
        FfiResult::ok(FfiBuffer::from_vec(bytes))
    })
    .expect("register");

    let input = FfiBuffer::from_vec(b"invoke me".to_vec());
    let result = unsafe {
        let c_name = std::ffi::CString::new(name.as_str()).unwrap();
        ffi_invoke_callback(c_name.as_ptr(), input)
    };

    assert!(result.is_ok(), "invoke should succeed");
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
    ffi_result_free(result);
}

#[test]
fn callback_not_found_returns_error() {
    let result = unsafe {
        let c_name = std::ffi::CString::new("__no_such_callback_xyz__").unwrap();
        ffi_invoke_callback(c_name.as_ptr(), FfiBuffer::null())
    };
    assert_eq!(result.error_code, FfiErrorCode::NotFound);
    ffi_result_free(result);
}

// ─── String round-trips ───────────────────────────────────────────────────────

#[test]
fn ffi_string_alloc_and_free() {
    let original = "hello from the integration test";
    let ffi_str = FfiString::new(original);
    assert_eq!(ffi_str.len, original.len());
    let recovered = unsafe { ffi_str.as_str() };
    assert_eq!(recovered, original);
    ffi_string_free(ffi_str);
}

#[test]
fn ffi_string_null_is_safe() {
    let s = FfiString::null();
    assert!(s.data.is_null());
    ffi_string_free(s); // Must not crash
}
