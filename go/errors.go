//go:build cgo
// +build cgo

package ffibridge

/*
#include "../shared/ffi.h"
*/
import "C"

import "fmt"

// ─── ErrorCode ────────────────────────────────────────────────────────────────

// ErrorCode mirrors the FfiErrorCode enum from shared/ffi.h.
// Values are stable across versions.
type ErrorCode int32

const (
	ErrOK             ErrorCode = 0
	ErrNullPointer    ErrorCode = 1
	ErrBufferTooSmall ErrorCode = 2
	ErrInvalidUTF8    ErrorCode = 3
	ErrSerialization  ErrorCode = 4
	ErrPanic          ErrorCode = 5
	ErrTimeout        ErrorCode = 6
	ErrNotFound       ErrorCode = 7
	ErrLockPoisoned   ErrorCode = 8
	ErrUnknown        ErrorCode = 99
)

func (c ErrorCode) String() string {
	switch c {
	case ErrOK:
		return "ok"
	case ErrNullPointer:
		return "null_pointer"
	case ErrBufferTooSmall:
		return "buffer_too_small"
	case ErrInvalidUTF8:
		return "invalid_utf8"
	case ErrSerialization:
		return "serialization"
	case ErrPanic:
		return "panic"
	case ErrTimeout:
		return "timeout"
	case ErrNotFound:
		return "not_found"
	case ErrLockPoisoned:
		return "lock_poisoned"
	default:
		return fmt.Sprintf("error_%d", int(c))
	}
}

// ─── FfiError ─────────────────────────────────────────────────────────────────

// FfiError represents an error returned from the Rust side over FFI.
type FfiError struct {
	Code    ErrorCode
	Message string
}

func (e *FfiError) Error() string {
	return fmt.Sprintf("ffi error [%s]: %s", e.Code, e.Message)
}

// ─── CheckResult ──────────────────────────────────────────────────────────────

// CheckResult inspects an FfiResult from Rust, frees all Rust-owned memory,
// and returns either a Go-owned *Buffer (on success) or an error.
//
// Ownership transfer:
//   - On success: the payload bytes are copied into a new Go-allocated buffer,
//     and the original Rust allocation is freed.
//   - On error: the error message is copied into the FfiError, and all Rust
//     memory is freed.
//
// Always call this function with the result returned from a Rust extern "C"
// function — never call ffi_result_free separately if you use CheckResult.
func CheckResult(result C.FfiResult) (*Buffer, error) {
	if result.error_code != C.FFI_OK {
		msg := GoStringFromFfiString(result.error_message)
		// Free both the error message and the (empty) payload.
		C.ffi_result_free(result)
		return nil, &FfiError{
			Code:    ErrorCode(result.error_code),
			Message: msg,
		}
	}

	// Copy payload bytes into a Go-managed buffer, then free the Rust allocation.
	buf := copyBuffer(result.payload)

	// Zero out payload in result before ffi_result_free to prevent double-free.
	result.payload = C.FfiBuffer{}
	C.ffi_result_free(result)

	return buf, nil
}
