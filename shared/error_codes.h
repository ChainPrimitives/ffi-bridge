#ifndef FFI_BRIDGE_ERROR_CODES_H
#define FFI_BRIDGE_ERROR_CODES_H

/*
 * ffi-bridge — Shared Error Code Definitions
 * ============================================================
 * Standalone header: can be included independently of ffi.h
 * when only the error codes are needed (e.g., in error-handling
 * utilities that don't deal with buffers or callbacks).
 *
 * These values are stable across versions. New codes will only
 * be added — never removed or renumbered.
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    /** No error. */
    FFI_OK                   = 0,

    /** A required pointer argument was NULL. */
    FFI_ERR_NULL_POINTER     = 1,

    /** The provided buffer is smaller than required. */
    FFI_ERR_BUFFER_TOO_SMALL = 2,

    /** Input bytes are not valid UTF-8. */
    FFI_ERR_INVALID_UTF8     = 3,

    /** JSON or other serialization failed. */
    FFI_ERR_SERIALIZATION    = 4,

    /** A Rust panic was caught at the FFI boundary. */
    FFI_ERR_PANIC            = 5,

    /** The operation exceeded its time limit. */
    FFI_ERR_TIMEOUT          = 6,

    /** A requested resource (e.g. callback name) was not found. */
    FFI_ERR_NOT_FOUND        = 7,

    /** A Mutex was poisoned by a previous panic. */
    FFI_ERR_LOCK_POISONED    = 8,

    /** Catch-all for unclassified errors. */
    FFI_ERR_UNKNOWN          = 99,
} FfiErrorCode;

/** Returns a static human-readable description for an error code. */
static inline const char *ffi_error_code_str(FfiErrorCode code) {
    switch (code) {
        case FFI_OK:                   return "ok";
        case FFI_ERR_NULL_POINTER:     return "null pointer";
        case FFI_ERR_BUFFER_TOO_SMALL: return "buffer too small";
        case FFI_ERR_INVALID_UTF8:     return "invalid utf-8";
        case FFI_ERR_SERIALIZATION:    return "serialization error";
        case FFI_ERR_PANIC:            return "panic at ffi boundary";
        case FFI_ERR_TIMEOUT:          return "timeout";
        case FFI_ERR_NOT_FOUND:        return "not found";
        case FFI_ERR_LOCK_POISONED:    return "lock poisoned";
        case FFI_ERR_UNKNOWN:          return "unknown error";
        default:                       return "unrecognized error code";
    }
}

#ifdef __cplusplus
}
#endif

#endif /* FFI_BRIDGE_ERROR_CODES_H */
