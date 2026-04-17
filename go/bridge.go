//go:build cgo
// +build cgo

// Package ffibridge provides memory-safe Go↔Rust FFI abstractions.
//
// The package links against the Rust ffi-bridge shared library and exposes
// idiomatic Go types that wrap the raw C ABI. All heap memory that crosses the
// FFI boundary is allocated by Rust and freed by the matching ffi_*_free
// exported function — the Go garbage collector never touches it.
//
// # Quick start
//
//	// Allocate a buffer from Go data
//	buf := ffibridge.FromBytes([]byte("hello rust"))
//	defer buf.Free()
//
//	// OR from a JSON-serializable value:
//	buf, err := ffibridge.FromJSON(myStruct)
//
//	// Invoke a Rust callback
//	result, err := ffibridge.InvokeCallback("my.callback", buf)
//	if err != nil { ... }
//	defer result.Free()
//
//	// Decode the result
//	var response MyResponse
//	if err := result.ToJSON(&response); err != nil { ... }
//
// # Build requirements
//
// CGO_ENABLED=1 is required. The Rust library must be built first:
//
//	cd ../rust && cargo build --release
package ffibridge

/*
#cgo LDFLAGS: -L${SRCDIR}/../rust/target/release -lffi_bridge
#cgo CFLAGS: -I${SRCDIR}/../shared

#include "../shared/ffi.h"
#include <stdlib.h>
#include <string.h>
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"runtime"
	"unsafe"
)

// ─── Buffer ───────────────────────────────────────────────────────────────────

// Buffer wraps an FfiBuffer allocated by the Rust side.
//
// When a Buffer is no longer reachable, Go's garbage collector will call
// runtime.SetFinalizer which triggers Free — but you should call Free
// explicitly (e.g. via defer) for predictable resource management.
type Buffer struct {
	inner C.FfiBuffer
	freed bool
}

// newBuffer wraps a raw C.FfiBuffer, registering a GC finalizer.
func newBuffer(raw C.FfiBuffer) *Buffer {
	b := &Buffer{inner: raw}
	runtime.SetFinalizer(b, (*Buffer).Free)
	return b
}

// NewBuffer allocates a new FFI buffer of the given capacity via Rust.
func NewBuffer(capacity int) *Buffer {
	return newBuffer(C.ffi_buffer_alloc(C.size_t(capacity)))
}

// FromBytes creates an FFI buffer containing a copy of data.
func FromBytes(data []byte) *Buffer {
	buf := NewBuffer(len(data))
	if len(data) > 0 {
		C.memcpy(
			unsafe.Pointer(buf.inner.data),
			unsafe.Pointer(&data[0]),
			C.size_t(len(data)),
		)
		buf.inner.len = C.size_t(len(data))
	}
	return buf
}

// FromJSON serializes v to JSON and stores it in an FFI buffer.
func FromJSON(v any) (*Buffer, error) {
	data, err := json.Marshal(v)
	if err != nil {
		return nil, fmt.Errorf("ffi: json marshal: %w", err)
	}
	return FromBytes(data), nil
}

// Bytes returns a copy of the buffer's contents as a Go byte slice.
// Returns nil if the buffer is empty or freed.
func (b *Buffer) Bytes() []byte {
	if b.freed || b.inner.data == nil || b.inner.len == 0 {
		return nil
	}
	return C.GoBytes(unsafe.Pointer(b.inner.data), C.int(b.inner.len))
}

// ToJSON deserializes the buffer's JSON contents into v.
func (b *Buffer) ToJSON(v any) error {
	data := b.Bytes()
	if data == nil {
		return fmt.Errorf("ffi: buffer is empty or freed")
	}
	return json.Unmarshal(data, v)
}

// Len returns the number of initialized bytes in the buffer.
func (b *Buffer) Len() int {
	if b.freed {
		return 0
	}
	return int(b.inner.len)
}

// IsEmpty reports whether the buffer has no initialized data.
func (b *Buffer) IsEmpty() bool {
	return b.Len() == 0
}

// Free releases the Rust-allocated memory for this buffer.
// Safe to call multiple times (idempotent).
func (b *Buffer) Free() {
	if !b.freed {
		C.ffi_buffer_free(b.inner)
		b.freed = true
		runtime.SetFinalizer(b, nil)
	}
}

// raw returns the inner C.FfiBuffer for passing to other C functions.
// Caller must not free the returned value independently.
func (b *Buffer) raw() C.FfiBuffer {
	return b.inner
}
