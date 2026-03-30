package ffibridge

/*
#include "../shared/ffi.h"
*/
import "C"

import "unsafe"

// ─── Shared Go-side type mirrors ──────────────────────────────────────────────

// BufferRaw holds the field values of a C.FfiBuffer without the C type.
// Useful when serializing buffer metadata (e.g. for logging or debugging).
type BufferRaw struct {
	Len      uint64
	Capacity uint64
}

// InspectBuffer returns the raw fields of a *Buffer for introspection.
// Returns a zero-value BufferRaw if b is nil or already freed.
func InspectBuffer(b *Buffer) BufferRaw {
	if b == nil || b.freed {
		return BufferRaw{}
	}
	return BufferRaw{
		Len:      uint64(b.inner.len),
		Capacity: uint64(b.inner.capacity),
	}
}

// Version returns the semantic version string of the linked Rust crate.
// Useful for asserting ABI compatibility between the Go module and the
// linked Rust library at startup.
func Version() string {
	// ffi_version returns an FfiBuffer containing the version bytes.
	raw := C.ffi_version()
	defer C.ffi_buffer_free(raw)
	if raw.data == nil || raw.len == 0 {
		return ""
	}
	return C.GoStringN((*C.char)(unsafe.Pointer(raw.data)), C.int(raw.len))
}
