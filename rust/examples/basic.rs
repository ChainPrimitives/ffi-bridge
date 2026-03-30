//! Basic usage example for ffi-bridge.
//!
//! Demonstrates:
//! 1. Allocating an FfiBuffer from JSON data
//! 2. Registering a Rust callback
//! 3. Invoking the callback and reading the result
//! 4. Proper cleanup of all FFI resources

use ffi_bridge::{
    callback_count, ffi_buffer_free, ffi_invoke_callback, ffi_result_free, register_callback,
    FfiBuffer, FfiError, FfiResult,
};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct MathRequest {
    a: i64,
    b: i64,
    op: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct MathResponse {
    result: i64,
}

fn main() {
    println!("ffi-bridge basic example");
    println!("=========================================");

    // ── 1. Register a callback from Rust ──────────────────────────────────────
    register_callback("math.add", |input| {
        // Deserialize the request
        let req: MathRequest = match unsafe { input.to_json() } {
            Ok(r) => r,
            Err(e) => return FfiResult::err(e),
        };

        let result = match req.op.as_str() {
            "add" => req.a + req.b,
            "mul" => req.a * req.b,
            "sub" => req.a - req.b,
            unknown => {
                return FfiResult::err(FfiError::Unknown(format!("unknown op: {unknown}")))
            }
        };

        match FfiBuffer::from_json(&MathResponse { result }) {
            Ok(buf) => FfiResult::ok(buf),
            Err(e) => FfiResult::err(e),
        }
    })
    .expect("callback registration failed");

    println!("✓ Registered callback 'math.add' ({} total)", callback_count());

    // ── 2. Build an input buffer ───────────────────────────────────────────────
    let request = MathRequest { a: 21, b: 21, op: "add".into() };
    let input = FfiBuffer::from_json(&request).expect("failed to serialize request");
    println!("✓ Serialized request: {{ a: 21, b: 21, op: \"add\" }}");

    // ── 3. Invoke the callback ─────────────────────────────────────────────────
    let result = unsafe {
        let c_name = std::ffi::CString::new("math.add").unwrap();
        ffi_invoke_callback(c_name.as_ptr(), input)
    };

    if !result.is_ok() {
        let msg = unsafe { result.error_message.as_str() };
        eprintln!("✗ Callback failed: {msg}");
        ffi_result_free(result);
        std::process::exit(1);
    }

    // ── 4. Decode the response ─────────────────────────────────────────────────
    let response: MathResponse = unsafe { result.payload.to_json() }
        .expect("failed to decode response");

    println!("✓ Result: 21 + 21 = {}", response.result);
    assert_eq!(response.result, 42, "math is broken");

    // ── 5. Free the result ────────────────────────────────────────────────────
    ffi_result_free(result);
    println!("✓ All FFI resources freed");

    // ── 6. Use ffi_echo built-in ──────────────────────────────────────────────
    let echo_input = FfiBuffer::from_vec(b"hello, rust!".to_vec());
    let echo_result = ffi_bridge::ffi_echo(echo_input);
    assert!(echo_result.is_ok());
    let echoed = unsafe { echo_result.payload.as_slice() };
    println!("✓ ffi_echo: {:?}", std::str::from_utf8(echoed).unwrap());
    ffi_result_free(echo_result);

    // ── 7. Version check ──────────────────────────────────────────────────────
    let ver_buf = ffi_bridge::ffi_version();
    let ver = unsafe { ver_buf.as_slice() };
    println!("✓ ffi-bridge version: {}", std::str::from_utf8(ver).unwrap());
    ffi_buffer_free(ver_buf);

    println!("=========================================");
    println!("All assertions passed ✓");
}
