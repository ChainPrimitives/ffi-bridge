package ffibridge

/*
#include "../shared/ffi.h"
#include <stdlib.h>
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"unsafe"
)

// ─── InvokeCallback ───────────────────────────────────────────────────────────

// InvokeCallback calls the Rust-registered callback named `name` with `input`.
//
// The callback must have been registered on the Rust side via
// register_callback() or ffi_register_callback(). The result is returned as a
// Go-managed *Buffer; call result.Free() (or defer result.Free()) when done.
//
// Returns ErrNotFound if no callback with that name exists.
// Any panic inside the Rust callback is caught and returned as an error.
func InvokeCallback(name string, input *Buffer) (*Buffer, error) {
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))

	var rawInput C.FfiBuffer
	if input != nil {
		rawInput = input.raw()
	}

	result := C.ffi_invoke_callback(cname, rawInput)
	return CheckResult(result)
}

// InvokeCallbackJSON is a convenience wrapper around InvokeCallback that
// serializes `req` to JSON before invoking the callback, and deserializes
// the response JSON into `resp`.
func InvokeCallbackJSON(name string, req any, resp any) error {
	buf, err := FromJSON(req)
	if err != nil {
		return fmt.Errorf("ffi: serialize request: %w", err)
	}
	defer buf.Free()

	result, err := InvokeCallback(name, buf)
	if err != nil {
		return err
	}
	defer result.Free()

	data := result.Bytes()
	if data == nil {
		return fmt.Errorf("ffi: empty response from callback %q", name)
	}
	if err := json.Unmarshal(data, resp); err != nil {
		return fmt.Errorf("ffi: deserialize response: %w", err)
	}
	return nil
}

// ─── RegisterFFICallback ──────────────────────────────────────────────────────
// Note: registering Go function pointers as C callbacks requires additional
// cgo export machinery (//export directives) which must be in a separate file
// to avoid duplicate symbol issues. The canonical approach for Go→Rust callbacks
// is to use InvokeCallbackJSON to invoke Rust-registered callbacks.
//
// If you need Rust to call back into Go, use a channel or shared state instead
// — crossing the FFI boundary in the Go→C→Rust→C→Go direction with callbacks
// is deeply unsafe due to goroutine/thread model mismatches.
