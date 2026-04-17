# ffi-bridge — Production Guide

## Overview

A Go↔Rust FFI boundary helper library providing memory-safe data passing, error propagation, and GC-isolation patterns. Ships as both a Go module and a Rust crate with matching interfaces.

**Why this package?** FFI between Go and Rust is notoriously error-prone — dangling pointers, GC interference, memory leaks. This library provides safe abstractions extracted from your hybrid-runtime-blockchain-engine project.

---

## Package Metadata

```
Go module:   github.com/ChainPrimitives/ffi-bridge
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
│   ├── bridge.go                # Core bridge API (Buffer, FromBytes, FromJSON)
│   ├── memory.go                # Low-level memory helpers (AllocBuffer, FfiString)
│   ├── errors.go                # Error propagation (ErrorCode, FfiError, CheckResult)
│   ├── types.go                 # Shared type mirrors (Version, InspectBuffer)
│   ├── callback.go              # Callback invocation (InvokeCallback, InvokeCallbackJSON)
│   ├── bridge_test.go
│   └── examples/
│       └── basic/main.go
├── rust/                        # Rust crate
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs               # Crate root & public API
│   │   ├── bridge.rs            # BridgeCall, ffi_echo, ffi_version
│   │   ├── memory.rs            # FfiBuffer, FfiString, alloc/free exports
│   │   ├── errors.rs            # FfiErrorCode, FfiError, FfiResult, catch_panic
│   │   ├── types.rs             # BridgeValue, check_not_null, cstr_to_string
│   │   └── callback.rs          # Named callback registry
│   ├── benches/
│   │   └── ffi_bench.rs         # Criterion benchmarks
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

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    FFI_OK                   = 0,
    FFI_ERR_NULL_POINTER     = 1,
    FFI_ERR_BUFFER_TOO_SMALL = 2,
    FFI_ERR_INVALID_UTF8     = 3,
    FFI_ERR_SERIALIZATION    = 4,
    FFI_ERR_PANIC            = 5,
    FFI_ERR_TIMEOUT          = 6,
    FFI_ERR_NOT_FOUND        = 7,
    FFI_ERR_LOCK_POISONED    = 8,
    FFI_ERR_UNKNOWN          = 99,
} FfiErrorCode;

typedef struct { uint8_t *data; size_t len; size_t capacity; } FfiBuffer;
typedef struct { char *data;    size_t len;                  } FfiString;

typedef struct {
    FfiErrorCode error_code;
    FfiString    error_message;   /* empty (data=NULL, len=0) on success */
    FfiBuffer    payload;         /* empty on error */
} FfiResult;

/* Buffer lifecycle */
FfiBuffer ffi_buffer_alloc(size_t capacity);
void      ffi_buffer_free(FfiBuffer buf);

/* String lifecycle */
FfiString ffi_string_alloc(const char *str, size_t len);
void      ffi_string_free(FfiString str);

/* Result lifecycle */
void ffi_result_free(FfiResult result);

/* Callback interface */
typedef FfiResult (*FfiCallback)(FfiBuffer input);
int32_t   ffi_register_callback(const char *name, FfiCallback cb);
FfiResult ffi_invoke_callback(const char *name, FfiBuffer input);
int32_t   ffi_unregister_callback(const char *name);
size_t    ffi_callback_count(void);

/* Bridge utilities */
FfiResult ffi_echo(FfiBuffer input);
FfiBuffer ffi_version(void);

#ifdef __cplusplus
}
#endif

#endif /* FFI_BRIDGE_H */
```

> Note: `FfiResult.error_message` is `FfiString` (not `*char`) — always use `len`, not `strlen`.

---

## Error Codes

| Code | Value | Meaning |
|------|-------|---------|
| `FFI_OK` | 0 | Success |
| `FFI_ERR_NULL_POINTER` | 1 | Required pointer was NULL |
| `FFI_ERR_BUFFER_TOO_SMALL` | 2 | Buffer smaller than required |
| `FFI_ERR_INVALID_UTF8` | 3 | Input is not valid UTF-8 |
| `FFI_ERR_SERIALIZATION` | 4 | JSON or other serialization failed |
| `FFI_ERR_PANIC` | 5 | Rust panic caught at FFI boundary |
| `FFI_ERR_TIMEOUT` | 6 | Operation exceeded time limit |
| `FFI_ERR_NOT_FOUND` | 7 | Named resource (e.g. callback) not found |
| `FFI_ERR_LOCK_POISONED` | 8 | Mutex poisoned by a previous panic |
| `FFI_ERR_UNKNOWN` | 99 | Catch-all for unclassified errors |

Codes are **stable** — never renumbered or removed across versions.

---

## Rust Implementation

### Cargo.toml

```toml
[package]
name = "ffi-bridge"
version = "1.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
once_cell  = "1"

[dev-dependencies]
tokio     = { version = "1", features = ["full"] }
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name    = "ffi_bench"
harness = false
```

### Memory model

```
Rust allocates → caller (Go) reads → Rust frees
```

- `FfiBuffer` and `FfiString` are `repr(C)` structs with **no `Drop` impl**.
- Go must call the matching `ffi_*_free` function; the GC never touches these allocations.
- Never copy these structs without transferring ownership — double-free will result.

### Key types

```rust
// FFI-safe byte buffer
#[repr(C)]
pub struct FfiBuffer { pub data: *mut u8, pub len: usize, pub capacity: usize }

// FFI-safe UTF-8 string (NOT null-terminated beyond len)
#[repr(C)]
pub struct FfiString { pub data: *mut u8, pub len: usize }

// C-ABI result — always call ffi_result_free when done
#[repr(C)]
pub struct FfiResult {
    pub error_code:    FfiErrorCode,
    pub error_message: FfiString,   // empty on success
    pub payload:       FfiBuffer,   // empty on error
}
```

### BridgeCall — recommended entry point

```rust
#[no_mangle]
pub extern "C" fn my_fn(input: FfiBuffer) -> FfiResult {
    BridgeCall::new(input).run(|buf| {
        let bytes = unsafe { buf.as_slice() }.to_vec();
        Ok(FfiBuffer::from_vec(bytes))
    })
}

// Or with automatic JSON de/serialization:
BridgeCall::new(input).run_json(|req: MyRequest| {
    Ok(MyResponse { result: req.value * 2 })
})
```

`BridgeCall::run` and `run_json` both wrap the closure in `catch_panic`, so panics are safely converted to `FfiErrorCode::Panic` and never cross the ABI.

### Callback registry

```rust
// Register from Rust
register_callback("math.add", |input| {
    let req: MathRequest = unsafe { input.to_json() }?;
    FfiResult::ok(FfiBuffer::from_json(&MathResponse { result: req.a + req.b })?)
}).expect("register failed");

// Register from C/Go (function pointer)
// int32_t ffi_register_callback(const char *name, FfiCallback cb);

// Invoke
// FfiResult ffi_invoke_callback(const char *name, FfiBuffer input);

// Remove
// int32_t ffi_unregister_callback(const char *name);
```

The registry is a `Mutex<HashMap<String, CallbackFn>>` guarded by `once_cell::sync::Lazy`. All exported functions are panic-safe.

---

## Go Implementation

### go.mod

```
module github.com/ChainPrimitives/ffi-bridge

go 1.21
```

No external Go dependencies — only the standard library and CGO.

### CGO link flags

```go
/*
#cgo LDFLAGS: -L${SRCDIR}/../rust/target/release -lffi_bridge
#cgo CFLAGS:  -I${SRCDIR}/../shared
#include "../shared/ffi.h"
#include <stdlib.h>
#include <string.h>
*/
import "C"
```

### Buffer API

```go
// Allocate / wrap
buf := ffibridge.NewBuffer(capacity)       // Rust-allocated, GC-finalizer registered
buf := ffibridge.FromBytes([]byte("data")) // copies Go bytes into Rust allocation
buf, err := ffibridge.FromJSON(myStruct)   // JSON-serializes into Rust allocation

// Read
buf.Bytes()          // copies bytes back to Go
buf.ToJSON(&target)  // deserializes JSON into target
buf.Len()
buf.IsEmpty()

// Release
buf.Free()           // idempotent; also called by GC finalizer
```

### Error handling

```go
// CheckResult converts a raw C.FfiResult into (*Buffer, error).
// It copies the payload into Go memory and frees all Rust allocations.
// Never call ffi_result_free separately if you use CheckResult.
buf, err := ffibridge.CheckResult(result)
if err != nil {
    var ffiErr *ffibridge.FfiError
    errors.As(err, &ffiErr)
    fmt.Println(ffiErr.Code, ffiErr.Message)
}
```

Error codes mirror the C enum:

```go
ffibridge.ErrOK             // 0
ffibridge.ErrNullPointer    // 1
ffibridge.ErrBufferTooSmall // 2
ffibridge.ErrInvalidUTF8    // 3
ffibridge.ErrSerialization  // 4
ffibridge.ErrPanic          // 5
ffibridge.ErrTimeout        // 6
ffibridge.ErrNotFound       // 7
ffibridge.ErrLockPoisoned   // 8
ffibridge.ErrUnknown        // 99
```

### Invoking callbacks

```go
// Raw buffer
result, err := ffibridge.InvokeCallback("math.add", inputBuf)
defer result.Free()

// JSON convenience wrapper
var resp MathResponse
err := ffibridge.InvokeCallbackJSON("math.add", MathRequest{A: 21, B: 21, Op: "add"}, &resp)
```

> Go→Rust→Go callbacks (registering Go function pointers for Rust to call back) are intentionally not supported. The goroutine/thread model mismatch makes this deeply unsafe. Use channels or shared state for Go-side callbacks instead.

### Version check

```go
ver := ffibridge.Version() // reads ffi_version() from the linked Rust dylib
```

---

## Build & Test

### Makefile targets

```
make              → build Rust (release) + Go
make rust         → Rust only
make go           → Go only (builds Rust first)
make test         → Rust unit/integration + Go tests
make test-rust    → Rust tests only
make test-go      → Go tests only
make bench        → Criterion (Rust) + go test -bench (Go)
make example      → run rust/examples/basic.rs
make lint         → cargo clippy + go vet
make fmt          → cargo fmt + gofmt
make clean        → remove all build artifacts
make publish      → cargo publish + git tag go/vX.Y.Z
```

### Quick start

```bash
# 1. Build the Rust shared library
cd rust && cargo build --release

# 2. Build and test Go
cd go && CGO_ENABLED=1 go test -v ./...

# 3. Run Rust tests
cd rust && cargo test

# 4. Run benchmarks
cd rust && cargo bench
cd go  && CGO_ENABLED=1 go test -bench=. -benchmem -run='^$' ./...
```

---

## Testing Strategy

| Layer | What's tested |
|-------|--------------|
| Rust unit tests | Each module has inline `#[cfg(test)]` tests |
| Rust integration (`tests/integration.rs`) | Full FFI surface: alloc, JSON, result, echo, callbacks, strings |
| Rust benchmarks (`benches/ffi_bench.rs`) | Criterion benchmarks for buffer, JSON, echo, callback, string ops |
| Go unit tests (`bridge_test.go`) | Buffer lifecycle, JSON round-trip, error codes, free idempotency |
| Go benchmarks | `BenchmarkFromBytes_1K`, `BenchmarkFromJSON` |

Run under AddressSanitizer for memory safety validation:

```bash
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test
```

---

## Publishing

```bash
# Rust crate
cd rust && cargo publish

# Go module
git tag go/v1.0.0 && git push origin go/v1.0.0
```

---

## Safety Contract

1. All buffers/strings allocated by this crate **must** be freed by the corresponding `ffi_*_free` function.
2. `ffi_result_free` frees **both** `error_message` and `payload` — if you extract `payload`, zero it in the struct before calling `ffi_result_free`.
3. Panics are caught at every `extern "C"` boundary via `catch_panic` — they never cross the ABI.
4. No Rust type with a `Drop` impl crosses the FFI boundary as a value — only `repr(C)` POD structs do.
5. The callback registry is `Mutex`-guarded and fully thread-safe.
