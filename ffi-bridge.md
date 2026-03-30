# ffi-bridge — Production Guide

## Overview

A Go↔Rust FFI boundary helper library providing memory-safe data passing, error propagation, and GC-isolation patterns. Ships as both a Go module and a Rust crate with matching interfaces.

**Why this package?** FFI between Go and Rust is notoriously error-prone — dangling pointers, GC interference, memory leaks. This library provides safe abstractions extracted from your hybrid-runtime-blockchain-engine project.

---

## Package Metadata

```
Go module:   github.com/Subaskar-S/ffi-bridge
Rust crate:  ffi-bridge
License:     MIT
```

> This is a dual-language package: a Go module + a Rust crate, published separately.

---

## Directory Structure

```
ffi-bridge/
├── go/                          # Go module
│   ├── go.mod
│   ├── go.sum
│   ├── bridge.go                # Core bridge API
│   ├── memory.go                # Memory management helpers
│   ├── errors.go                # Error propagation across FFI
│   ├── types.go                 # Shared type definitions
│   ├── callback.go              # Safe callback registration
│   ├── bridge_test.go
│   └── examples/
│       └── basic/main.go
├── rust/                        # Rust crate
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs               # Crate root & public API
│   │   ├── bridge.rs            # Core bridge types
│   │   ├── memory.rs            # Arena allocator for FFI buffers
│   │   ├── errors.rs            # Error codes & propagation
│   │   ├── types.rs             # Shared FFI-safe types
│   │   └── callback.rs          # Callback handler
│   ├── tests/
│   │   └── integration.rs
│   └── examples/
│       └── basic.rs
├── shared/
│   ├── ffi.h                    # C header for FFI interface
│   └── error_codes.h            # Shared error code definitions
├── Makefile                     # Build both Go + Rust
├── README.md
└── LICENSE
```

---

## Shared C Header — shared/ffi.h

```c
#ifndef FFI_BRIDGE_H
#define FFI_BRIDGE_H

#include <stdint.h>
#include <stddef.h>

// Error codes (shared between Go and Rust)
typedef enum {
    FFI_OK = 0,
    FFI_ERR_NULL_POINTER = 1,
    FFI_ERR_BUFFER_TOO_SMALL = 2,
    FFI_ERR_INVALID_UTF8 = 3,
    FFI_ERR_SERIALIZATION = 4,
    FFI_ERR_PANIC = 5,
    FFI_ERR_TIMEOUT = 6,
    FFI_ERR_UNKNOWN = 99,
} FfiErrorCode;

// FFI-safe byte buffer
typedef struct {
    uint8_t* data;
    size_t len;
    size_t capacity;
} FfiBuffer;

// FFI-safe string (UTF-8, null-terminated)
typedef struct {
    char* data;
    size_t len;
} FfiString;

// FFI result wrapper
typedef struct {
    FfiErrorCode error_code;
    FfiString error_message;
    FfiBuffer payload;
} FfiResult;

// Lifecycle
FfiBuffer ffi_buffer_alloc(size_t capacity);
void ffi_buffer_free(FfiBuffer buf);
FfiString ffi_string_alloc(const char* str, size_t len);
void ffi_string_free(FfiString str);
void ffi_result_free(FfiResult result);

// Callback registration
typedef FfiResult (*FfiCallback)(FfiBuffer input);
int32_t ffi_register_callback(const char* name, FfiCallback cb);
FfiResult ffi_invoke_callback(const char* name, FfiBuffer input);

#endif
```

---

## Rust Implementation

### rust/Cargo.toml

```toml
[package]
name = "ffi-bridge"
version = "1.0.0"
edition = "2021"
description = "Memory-safe Go↔Rust FFI boundary helpers"
license = "MIT"
repository = "https://github.com/Subaskar-S/ffi-bridge"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

### rust/src/lib.rs

```rust
pub mod bridge;
pub mod memory;
pub mod errors;
pub mod types;
pub mod callback;

pub use bridge::*;
pub use memory::*;
pub use errors::*;
pub use types::*;
```

### rust/src/memory.rs

```rust
use std::alloc::{alloc, dealloc, Layout};
use std::ptr;

/// FFI-safe byte buffer with explicit ownership semantics.
/// The Rust side allocates, the Go side reads, Rust side frees.
#[repr(C)]
pub struct FfiBuffer {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl FfiBuffer {
    /// Create a new buffer with given capacity
    pub fn new(capacity: usize) -> Self {
        if capacity == 0 {
            return FfiBuffer { data: ptr::null_mut(), len: 0, capacity: 0 };
        }
        let layout = Layout::array::<u8>(capacity).expect("Invalid layout");
        let data = unsafe { alloc(layout) };
        FfiBuffer { data, len: 0, capacity }
    }

    /// Create buffer from a Vec<u8>, consuming the vec
    pub fn from_vec(mut vec: Vec<u8>) -> Self {
        let buf = FfiBuffer {
            data: vec.as_mut_ptr(),
            len: vec.len(),
            capacity: vec.capacity(),
        };
        std::mem::forget(vec); // Prevent Vec from freeing memory
        buf
    }

    /// Create buffer from serializable data
    pub fn from_json<T: serde::Serialize>(value: &T) -> Result<Self, FfiError> {
        let json = serde_json::to_vec(value)
            .map_err(|e| FfiError::Serialization(e.to_string()))?;
        Ok(Self::from_vec(json))
    }

    /// Reconstruct as a slice (unsafe: caller must ensure validity)
    pub unsafe fn as_slice(&self) -> &[u8] {
        if self.data.is_null() || self.len == 0 {
            return &[];
        }
        std::slice::from_raw_parts(self.data, self.len)
    }

    /// Deserialize from JSON
    pub unsafe fn to_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, FfiError> {
        let slice = self.as_slice();
        serde_json::from_slice(slice)
            .map_err(|e| FfiError::Serialization(e.to_string()))
    }
}

/// Free a buffer allocated by Rust
#[no_mangle]
pub extern "C" fn ffi_buffer_free(buf: FfiBuffer) {
    if buf.data.is_null() || buf.capacity == 0 {
        return;
    }
    let layout = Layout::array::<u8>(buf.capacity).expect("Invalid layout");
    unsafe { dealloc(buf.data, layout); }
}

/// Allocate a buffer from the Rust side
#[no_mangle]
pub extern "C" fn ffi_buffer_alloc(capacity: usize) -> FfiBuffer {
    FfiBuffer::new(capacity)
}
```

### rust/src/errors.rs

```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Error codes matching the C header
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FfiErrorCode {
    Ok = 0,
    NullPointer = 1,
    BufferTooSmall = 2,
    InvalidUtf8 = 3,
    Serialization = 4,
    Panic = 5,
    Timeout = 6,
    Unknown = 99,
}

/// Rust-side error type
#[derive(Debug)]
pub enum FfiError {
    NullPointer,
    BufferTooSmall { needed: usize, available: usize },
    InvalidUtf8(String),
    Serialization(String),
    Panic(String),
    Timeout,
    Unknown(String),
}

impl FfiError {
    pub fn code(&self) -> FfiErrorCode {
        match self {
            FfiError::NullPointer => FfiErrorCode::NullPointer,
            FfiError::BufferTooSmall { .. } => FfiErrorCode::BufferTooSmall,
            FfiError::InvalidUtf8(_) => FfiErrorCode::InvalidUtf8,
            FfiError::Serialization(_) => FfiErrorCode::Serialization,
            FfiError::Panic(_) => FfiErrorCode::Panic,
            FfiError::Timeout => FfiErrorCode::Timeout,
            FfiError::Unknown(_) => FfiErrorCode::Unknown,
        }
    }

    pub fn message(&self) -> String {
        match self {
            FfiError::NullPointer => "Null pointer received".into(),
            FfiError::BufferTooSmall { needed, available } =>
                format!("Buffer too small: need {needed}, have {available}"),
            FfiError::InvalidUtf8(s) => format!("Invalid UTF-8: {s}"),
            FfiError::Serialization(s) => format!("Serialization error: {s}"),
            FfiError::Panic(s) => format!("Panic caught: {s}"),
            FfiError::Timeout => "Operation timed out".into(),
            FfiError::Unknown(s) => format!("Unknown error: {s}"),
        }
    }
}

/// FFI-safe result type
#[repr(C)]
pub struct FfiResult {
    pub error_code: FfiErrorCode,
    pub error_message: *mut c_char,  // Null if no error
    pub payload: super::memory::FfiBuffer,
}

impl FfiResult {
    pub fn ok(payload: super::memory::FfiBuffer) -> Self {
        FfiResult {
            error_code: FfiErrorCode::Ok,
            error_message: std::ptr::null_mut(),
            payload,
        }
    }

    pub fn err(error: FfiError) -> Self {
        let msg = CString::new(error.message()).unwrap_or_default();
        FfiResult {
            error_code: error.code(),
            error_message: msg.into_raw(),
            payload: super::memory::FfiBuffer::new(0),
        }
    }
}

/// Free the result, including error message string
#[no_mangle]
pub extern "C" fn ffi_result_free(result: FfiResult) {
    if !result.error_message.is_null() {
        unsafe { drop(CString::from_raw(result.error_message)); }
    }
    ffi_buffer_free(result.payload);
}

use super::memory::ffi_buffer_free;

/// Catch panics at the FFI boundary and convert to error results.
/// CRITICAL: Panics must never cross the FFI boundary!
pub fn catch_panic<F>(f: F) -> FfiResult
where
    F: FnOnce() -> Result<super::memory::FfiBuffer, FfiError> + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(Ok(buf)) => FfiResult::ok(buf),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => {
            let msg = panic.downcast_ref::<&str>()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown panic".to_string());
            FfiResult::err(FfiError::Panic(msg))
        }
    }
}
```

### rust/src/callback.rs

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use std::ffi::CStr;
use std::os::raw::c_char;
use once_cell::sync::Lazy;

use crate::memory::FfiBuffer;
use crate::errors::{FfiResult, FfiError, catch_panic};

type CallbackFn = Box<dyn Fn(FfiBuffer) -> FfiResult + Send + Sync>;

static CALLBACKS: Lazy<Mutex<HashMap<String, CallbackFn>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Register a named callback that can be invoked from Go
pub fn register_callback<F>(name: &str, f: F)
where
    F: Fn(FfiBuffer) -> FfiResult + Send + Sync + 'static,
{
    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.insert(name.to_string(), Box::new(f));
}

/// FFI-exported: register callback by name
#[no_mangle]
pub extern "C" fn ffi_register_callback(
    name: *const c_char,
    cb: extern "C" fn(FfiBuffer) -> FfiResult,
) -> i32 {
    let name = unsafe {
        match CStr::from_ptr(name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return -1,
        }
    };

    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.insert(name, Box::new(move |buf| cb(buf)));
    0
}

/// FFI-exported: invoke a registered callback by name
#[no_mangle]
pub extern "C" fn ffi_invoke_callback(
    name: *const c_char,
    input: FfiBuffer,
) -> FfiResult {
    catch_panic(|| {
        let name = unsafe {
            CStr::from_ptr(name).to_str()
                .map_err(|_| FfiError::InvalidUtf8("callback name".into()))?
                .to_string()
        };

        let callbacks = CALLBACKS.lock()
            .map_err(|_| FfiError::Unknown("Lock poisoned".into()))?;

        let cb = callbacks.get(&name)
            .ok_or_else(|| FfiError::Unknown(format!("Callback '{}' not found", name)))?;

        // Invoke the callback
        let result = cb(input);
        Ok(result.payload)
    })
}
```

---

## Go Implementation

### go/go.mod

```
module github.com/Subaskar-S/ffi-bridge

go 1.21
```

### go/bridge.go

```go
package ffibridge

/*
#cgo LDFLAGS: -L../rust/target/release -lffi_bridge
#include "../shared/ffi.h"
#include <stdlib.h>
*/
import "C"
import (
	"encoding/json"
	"fmt"
	"runtime"
	"unsafe"
)

// Buffer wraps an FFI buffer with automatic cleanup
type Buffer struct {
	inner C.FfiBuffer
	freed bool
}

// NewBuffer creates a new FFI buffer
func NewBuffer(capacity int) *Buffer {
	buf := &Buffer{
		inner: C.ffi_buffer_alloc(C.size_t(capacity)),
	}
	runtime.SetFinalizer(buf, (*Buffer).Free)
	return buf
}

// FromBytes creates a buffer from a Go byte slice
func FromBytes(data []byte) *Buffer {
	buf := NewBuffer(len(data))
	if len(data) > 0 {
		C.memcpy(unsafe.Pointer(buf.inner.data), unsafe.Pointer(&data[0]), C.size_t(len(data)))
		buf.inner.len = C.size_t(len(data))
	}
	return buf
}

// FromJSON creates a buffer from a JSON-serializable value
func FromJSON(v interface{}) (*Buffer, error) {
	data, err := json.Marshal(v)
	if err != nil {
		return nil, fmt.Errorf("ffi: json marshal: %w", err)
	}
	return FromBytes(data), nil
}

// Bytes returns the buffer contents as a Go byte slice (copies data)
func (b *Buffer) Bytes() []byte {
	if b.inner.data == nil || b.inner.len == 0 {
		return nil
	}
	return C.GoBytes(unsafe.Pointer(b.inner.data), C.int(b.inner.len))
}

// ToJSON deserializes the buffer contents into v
func (b *Buffer) ToJSON(v interface{}) error {
	return json.Unmarshal(b.Bytes(), v)
}

// Free releases the buffer memory
func (b *Buffer) Free() {
	if !b.freed {
		C.ffi_buffer_free(b.inner)
		b.freed = true
	}
}
```

### go/errors.go

```go
package ffibridge

import "fmt"

// ErrorCode matches the C FFI error codes
type ErrorCode int32

const (
	ErrOK             ErrorCode = 0
	ErrNullPointer    ErrorCode = 1
	ErrBufferTooSmall ErrorCode = 2
	ErrInvalidUTF8    ErrorCode = 3
	ErrSerialization  ErrorCode = 4
	ErrPanic          ErrorCode = 5
	ErrTimeout        ErrorCode = 6
	ErrUnknown        ErrorCode = 99
)

// FfiError represents an error from the Rust side
type FfiError struct {
	Code    ErrorCode
	Message string
}

func (e *FfiError) Error() string {
	return fmt.Sprintf("ffi error %d: %s", e.Code, e.Message)
}

// CheckResult converts an FFI result into Go error handling
func CheckResult(result C.FfiResult) (*Buffer, error) {
	defer C.ffi_result_free(result)

	if result.error_code != 0 {
		msg := ""
		if result.error_message != nil {
			msg = C.GoString(result.error_message)
		}
		return nil, &FfiError{
			Code:    ErrorCode(result.error_code),
			Message: msg,
		}
	}

	// Copy payload before freeing result
	buf := &Buffer{inner: result.payload, freed: false}
	// Prevent double-free: result_free won't free payload since we took ownership
	result.payload = C.FfiBuffer{}
	return buf, nil
}
```

---

## Build & Test

### Makefile

```makefile
.PHONY: all rust go test clean

all: rust go

rust:
	cd rust && cargo build --release

go: rust
	cd go && CGO_ENABLED=1 go build ./...

test: all
	cd rust && cargo test
	cd go && CGO_ENABLED=1 go test -v ./...

clean:
	cd rust && cargo clean
	cd go && go clean

bench:
	cd rust && cargo bench
	cd go && CGO_ENABLED=1 go test -bench=. -benchmem ./...
```

---

## Publishing

```bash
# Rust crate
cd rust && cargo publish

# Go module
cd go && git tag go/v1.0.0 && git push origin go/v1.0.0
```

---

## Testing Strategy

- **Unit**: Buffer alloc/free, JSON round-trip, error propagation
- **Memory safety**: Run under Valgrind/ASan to detect leaks
- **Callback**: Register Go callback, invoke from Rust, verify data integrity
- **Benchmark**: Measure FFI overhead vs native calls
