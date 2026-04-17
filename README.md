# ffi-bridge

> Memory-safe Go↔Rust FFI boundary helpers: buffer management, error propagation, panic safety, and named callback registration.

[![Crates.io](https://img.shields.io/crates/v/ffi-bridge.svg)](https://crates.io/crates/ffi-bridge)
[![Go Reference](https://pkg.go.dev/badge/github.com/ChainPrimitives/ffi-bridge.svg)](https://pkg.go.dev/github.com/ChainPrimitives/ffi-bridge)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/ChainPrimitives/ffi-bridge)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/ChainPrimitives/ffi-bridge/pulls)

A dual-language package — a **Go module** and a **Rust crate** — that provides safe abstractions for crossing the Go↔Rust FFI boundary. Extracted from a hybrid-runtime blockchain engine where correctness at the FFI layer is critical.

---

## Why ffi-bridge?

FFI between Go and Rust is notoriously error-prone:

| Problem | ffi-bridge solution |
|---------|-------------------|
| Dangling pointers | Explicit ownership: Rust allocates, Rust frees via exported functions |
| GC interference | `runtime.SetFinalizer` isolates Go GC from Rust heap allocations |
| Panic unwinds crossing ABI | Every `extern "C"` fn wraps its body in `catch_panic` |
| Error propagation loss | `FfiResult` carries typed error codes + messages over the boundary |
| Callback lifetime bugs | Named callback registry with `Mutex`-guard and `once_cell` |

---

## Repository Layout

```
ffi-bridge/
├── go/                          # Go module (github.com/ChainPrimitives/ffi-bridge)
│   ├── go.mod
│   ├── bridge.go                # Buffer type + FromBytes / FromJSON
│   ├── memory.go                # Low-level alloc helpers, FfiString conversion
│   ├── errors.go                # ErrorCode, FfiError, CheckResult
│   ├── types.go                 # Type mirrors + Version()
│   ├── callback.go              # InvokeCallback / InvokeCallbackJSON
│   ├── bridge_test.go           # Unit tests + benchmarks
│   └── examples/
│       └── basic/main.go
├── rust/                        # Rust crate (ffi-bridge on crates.io)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── memory.rs            # FfiBuffer, FfiString
│   │   ├── errors.rs            # FfiErrorCode, FfiError, FfiResult, catch_panic
│   │   ├── types.rs             # BridgeValue, utilities
│   │   ├── bridge.rs            # BridgeCall, ffi_echo, ffi_version
│   │   └── callback.rs          # Named callback registry
│   ├── tests/
│   │   └── integration.rs
│   └── examples/
│       └── basic.rs
├── shared/
│   ├── ffi.h                    # C header — the ABI contract
│   └── error_codes.h            # Standalone error code definitions
├── Makefile
├── LICENSE
└── README.md
```

---

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | ≥ 1.70 | Build the Rust crate |
| Go | ≥ 1.21 | Build the Go module |
| C compiler | Any (clang/gcc) | CGo linkage |
| `cargo` | Latest stable | Rust build system |

---

## Getting Started

### 1. Clone and build

```bash
git clone https://github.com/ChainPrimitives/ffi-bridge
cd ffi-bridge

make          # Builds Rust (release) then Go
make test     # Runs all tests
make example  # Runs the Rust example
```

### 2. Use the Rust crate

Add to `Cargo.toml`:

```toml
[dependencies]
ffi-bridge = "1.0"
```

### 3. Use the Go module

```bash
go get github.com/ChainPrimitives/ffi-bridge/go
```

> **Note:** CGO_ENABLED=1 is required. The Rust shared library must be built
> and accessible via `LD_LIBRARY_PATH` (Linux) or `DYLD_LIBRARY_PATH` (macOS)
> at link time.

---

## Rust API

### Buffer allocation

```rust
use ffi_bridge::{FfiBuffer, ffi_buffer_free};

// Allocate a buffer
let buf = FfiBuffer::new(1024);

// From a Vec<u8> (zero-copy, transfers ownership)
let buf = FfiBuffer::from_vec(vec![1, 2, 3]);

// From a JSON-serializable value
let buf = FfiBuffer::from_json(&my_struct)?;

// Read as a slice
let slice = unsafe { buf.as_slice() };

// Deserialize
let value: MyType = unsafe { buf.to_json() }?;

// Free (must be called when done — no Drop)
ffi_buffer_free(buf);
```

### FfiResult

```rust
use ffi_bridge::{FfiResult, FfiError, catch_panic};

// Constructors
let ok_result = FfiResult::ok(some_buffer);
let err_result = FfiResult::err(FfiError::Timeout);

// Panic-safe wrapper
let result = catch_panic(|| {
    // ... potentially panicking code
    Ok(FfiBuffer::from_vec(b"output".to_vec()))
});
```

### Callbacks

```rust
use ffi_bridge::{register_callback, FfiResult, FfiBuffer};

// Register from Rust
register_callback("my.handler", |input| {
    let req: MyRequest = unsafe { input.to_json() }?;
    let resp = process(req);
    match FfiBuffer::from_json(&resp) {
        Ok(buf) => FfiResult::ok(buf),
        Err(e)  => FfiResult::err(e),
    }
})?;
```

### BridgeCall

```rust
use ffi_bridge::BridgeCall;

#[no_mangle]
pub extern "C" fn my_fn(input: FfiBuffer) -> FfiResult {
    BridgeCall::new(input).run_json(|req: MyRequest| {
        Ok(process(req))   // MyResponse implements Serialize
    })
}
```

---

## Go API

### Buffer operations

```go
import ffibridge "github.com/ChainPrimitives/ffi-bridge/go"

// From bytes
buf := ffibridge.FromBytes([]byte("hello rust"))
defer buf.Free()

// From a JSON-serializable value
buf, err := ffibridge.FromJSON(myStruct)
if err != nil { ... }
defer buf.Free()

// Read back
bytes := buf.Bytes()

// Decode JSON
var resp MyResponse
if err := buf.ToJSON(&resp); err != nil { ... }
```

### Invoking Rust callbacks

```go
// Raw buffer
result, err := ffibridge.InvokeCallback("my.handler", inputBuf)
if err != nil { ... }
defer result.Free()

// JSON convenience (serialize in → deserialize out)
var resp MyResponse
err := ffibridge.InvokeCallbackJSON("my.handler", myRequest, &resp)
```

### Errors

```go
// FfiError carries a typed error code + message
result, err := ffibridge.InvokeCallback("unknown.callback", buf)
if err != nil {
    ffiErr := err.(*ffibridge.FfiError)
    fmt.Println(ffiErr.Code)    // ffibridge.ErrNotFound
    fmt.Println(ffiErr.Message) // "not found: unknown.callback"
}
```

### Version check

```go
ver := ffibridge.Version() // e.g. "1.0.0"
```

---

## C Header

The shared C ABI contract lives in `shared/ffi.h`. Include it in any C, C++, or CGo consumer:

```c
#include "shared/ffi.h"

FfiBuffer buf = ffi_buffer_alloc(128);
// ... write data into buf.data, set buf.len ...
ffi_buffer_free(buf);

FfiResult result = ffi_invoke_callback("my.handler", buf);
if (result.error_code != FFI_OK) {
    // handle error
}
ffi_result_free(result);
```

---

## Safety Guarantees

### Memory safety

- **Rust allocates, Rust frees.** Go never calls `malloc`/`free` for FFI buffers.
  All allocation goes through Rust's global allocator.
- **GC isolation.** Go's garbage collector cannot see Rust heap memory.
  `runtime.SetFinalizer` on the Go `*Buffer` wrapper ensures cleanup if
  `Free()` is not called explicitly.
- **No double-free.** `Buffer.Free()` is idempotent. `CheckResult` zeroes the
  payload field of `FfiResult` before calling `ffi_result_free` to prevent
  the Rust side from freeing bytes that have already been copied to Go.

### Panic safety

- **Panics never cross the FFI boundary.** Every `extern "C"` function in the
  Rust crate wraps its body in `catch_panic`, which uses `std::panic::catch_unwind`.
  A caught panic is converted to `FfiResult { error_code: FFI_ERR_PANIC, ... }`.

### Thread safety

- **Callback registry** is protected by `std::sync::Mutex<HashMap>` initialized
  with `once_cell::sync::Lazy`. Registrations from any thread are safe.
- Poisoned locks are detected and reported as `FFI_ERR_LOCK_POISONED`.

---

## Error Codes

| Code | Value | Description |
|------|-------|-------------|
| `FFI_OK` | 0 | Success |
| `FFI_ERR_NULL_POINTER` | 1 | Required pointer was null |
| `FFI_ERR_BUFFER_TOO_SMALL` | 2 | Buffer smaller than required |
| `FFI_ERR_INVALID_UTF8` | 3 | Input is not valid UTF-8 |
| `FFI_ERR_SERIALIZATION` | 4 | JSON serialization failed |
| `FFI_ERR_PANIC` | 5 | Rust panic caught at boundary |
| `FFI_ERR_TIMEOUT` | 6 | Operation timed out |
| `FFI_ERR_NOT_FOUND` | 7 | Named resource not found |
| `FFI_ERR_LOCK_POISONED` | 8 | Mutex lock poisoned |
| `FFI_ERR_UNKNOWN` | 99 | Unclassified error |

Error codes are **stable** — values are never renumbered or removed.

---

## Build Details

### Makefile targets

```bash
make             # Build Rust (release) + Go
make rust        # Rust only
make go          # Go only (builds Rust first)
make test        # All tests
make test-rust   # Rust tests only
make test-go     # Go tests only
make bench       # Benchmarks (both sides)
make example     # Run Rust basic example
make lint        # cargo clippy + go vet
make fmt         # cargo fmt + gofmt
make clean       # Remove all build artifacts
make publish     # cargo publish + git tag go/v*
```

### CGo link flags

The Go module uses `#cgo LDFLAGS` to locate the Rust shared library:

```
-L${SRCDIR}/../rust/target/release -lffi_bridge
```

For production deployments, install the library system-wide or set
`CGO_LDFLAGS` / `LD_LIBRARY_PATH` appropriately.

### Platform support

| Platform | Rust target | Library extension |
|----------|-------------|-------------------|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `.so` |
| macOS arm64 | `aarch64-apple-darwin` | `.dylib` |
| macOS x86_64 | `x86_64-apple-darwin` | `.dylib` |

---

## Testing Strategy

- **Unit tests** in each Rust module (`#[cfg(test)]`) — buffer alloc/free, JSON round-trips, error enum mapping, panic catching.
- **Integration tests** in `rust/tests/integration.rs` — full FFI surface area end-to-end.
- **Go tests** in `go/bridge_test.go` — buffer lifecycle, JSON helpers, error types, benchmarks.
- **Memory safety** — run tests under AddressSanitizer (Linux):
  ```bash
  cd rust && RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --target x86_64-unknown-linux-gnu
  ```
- **Valgrind** (Linux):
  ```bash
  valgrind --leak-check=full cd go && CGO_ENABLED=1 go test ./...
  ```

---

## Publishing

```bash
# Rust crate
cd rust && cargo publish

# Go module — tag follows the go/ prefix convention for multi-module repos
git tag go/v1.0.0
git push origin go/v1.0.0
```

Or use `make publish` which runs tests first.

---

## Development

```bash
git clone https://github.com/ChainPrimitives/ffi-bridge
cd ffi-bridge

make              # Build everything
make test         # Run all tests
make lint         # Clippy + go vet
make fmt          # Auto-format both sides
make example      # Run the Rust basic example
```

---

## Contributing

Contributions are welcome! This project follows standard open-source practices.

### Getting Started

1. **Fork** the repository
2. **Clone** your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/ffi-bridge
   cd ffi-bridge
   make
   ```
3. **Branch** for your change:
   ```bash
   git checkout -b feat/my-feature
   ```
4. **Test** your changes:
   ```bash
   make lint && make test
   ```
5. **Commit** using [Conventional Commits](https://www.conventionalcommits.org/):
   ```
   feat: add FfiString length validation
   fix: prevent double-free in CheckResult on empty payload
   docs: expand safety guarantee documentation
   ```
6. Open a **Pull Request** against `main`.

### Guidelines

- **Tests required.** New functionality must include unit tests.
- **No unsafe without justification.** All `unsafe` blocks must have a `// SAFETY:` comment.
- **No breaking changes** to `FfiErrorCode` values or exported C symbols.
- **Keep FFI boundary minimal.** Only `repr(C)` POD types may cross the boundary.
- For security vulnerabilities, email **subaskar.sr@gmail.com** directly.

---

## Changelog

### v1.0.0

- 🚀 Initial release
- `FfiBuffer` — explicit-ownership heap buffer with alloc/free
- `FfiString` — UTF-8 string allocation
- `FfiResult` — C-ABI result type with error code + message
- `catch_panic` — panic boundary guard
- `BridgeCall` — high-level FFI call builder with JSON helpers
- Named callback registry with thread-safe global state
- Go module: `Buffer`, `FromBytes`, `FromJSON`, `InvokeCallback`, `InvokeCallbackJSON`, `CheckResult`
- Shared C header (`shared/ffi.h`, `shared/error_codes.h`)
- Full test suite: Rust unit + integration tests, Go unit tests + benchmarks
- Makefile for build orchestration

---

## License

MIT © 2026 [Subaskar Sivakumar](https://github.com/Subaskar-S)
