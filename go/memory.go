package ffibridge

/*
#include "../shared/ffi.h"
#include <stdlib.h>
*/
import "C"

import (
	"unsafe"
)

// ─── Low-level memory helpers ─────────────────────────────────────────────────

// AllocBuffer allocates a raw FFI buffer via Rust's allocator.
// The returned buffer must be freed with FreeBuffer when done.
// Prefer NewBuffer (returns *Buffer with finalizer) for most use cases.
func AllocBuffer(capacity int) C.FfiBuffer {
	return C.ffi_buffer_alloc(C.size_t(capacity))
}

// FreeBuffer frees a raw C.FfiBuffer allocated by Rust.
// Safe to call on a zeroed buffer.
func FreeBuffer(buf C.FfiBuffer) {
	C.ffi_buffer_free(buf)
}

// ─── String helpers ───────────────────────────────────────────────────────────

// GoStringFromFfiString converts an FfiString (UTF-8, non-null-terminated)
// into a Go string by copying the bytes. The FfiString is not freed.
func GoStringFromFfiString(s C.FfiString) string {
	if s.data == nil || s.len == 0 {
		return ""
	}
	return C.GoStringN((*C.char)(unsafe.Pointer(s.data)), C.int(s.len))
}

// AllocFfiString copies a Go string into a Rust-allocated FfiString.
// The returned FfiString must be freed with FreeFFiString.
func AllocFfiString(s string) C.FfiString {
	if s == "" {
		return C.FfiString{}
	}
	cstr := C.CString(s)
	defer C.free(unsafe.Pointer(cstr))
	return C.ffi_string_alloc(cstr, C.size_t(len(s)))
}

// FreeFfiString frees a Rust-allocated FfiString.
func FreeFfiString(s C.FfiString) {
	C.ffi_string_free(s)
}

// ─── Internal buffer copy helper ─────────────────────────────────────────────

// copyBuffer makes a *Buffer that holds a copy of the raw C.FfiBuffer's data.
// The raw buffer is NOT freed — caller retains ownership of it.
func copyBuffer(raw C.FfiBuffer) *Buffer {
	if raw.data == nil || raw.len == 0 {
		return newBuffer(C.FfiBuffer{})
	}
	goBytes := C.GoBytes(unsafe.Pointer(raw.data), C.int(raw.len))
	return FromBytes(goBytes)
}
