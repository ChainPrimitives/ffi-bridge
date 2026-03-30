#ifndef FFI_BRIDGE_H
#define FFI_BRIDGE_H

/*
 * ffi-bridge — Shared C Header
 * ============================================================
 * This header defines the ABI contract between the Go module and
 * the Rust crate. Keep in sync with:
 *   - rust/src/memory.rs   (FfiBuffer, FfiString)
 *   - rust/src/errors.rs   (FfiErrorCode, FfiResult)
 *   - rust/src/callback.rs (ffi_register_callback, ffi_invoke_callback)
 *
 * Ownership rules:
 *   - All buffers and strings allocated by Rust MUST be freed by calling
 *     the corresponding ffi_*_free function.
 *   - ffi_result_free frees both the error_message AND the payload.
 *   - If you extract payload from FfiResult, zero-out result.payload
 *     BEFORE calling ffi_result_free to avoid double-free.
 */

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ═══════════════════════════════════════════════════════════
 * Error Codes
 * ═══════════════════════════════════════════════════════════ */

typedef enum {
    FFI_OK                  = 0,
    FFI_ERR_NULL_POINTER    = 1,
    FFI_ERR_BUFFER_TOO_SMALL = 2,
    FFI_ERR_INVALID_UTF8    = 3,
    FFI_ERR_SERIALIZATION   = 4,
    FFI_ERR_PANIC           = 5,
    FFI_ERR_TIMEOUT         = 6,
    FFI_ERR_NOT_FOUND       = 7,
    FFI_ERR_LOCK_POISONED   = 8,
    FFI_ERR_UNKNOWN         = 99,
} FfiErrorCode;

/* ═══════════════════════════════════════════════════════════
 * Core Types
 * ═══════════════════════════════════════════════════════════ */

/**
 * FFI-safe byte buffer.
 * Ownership: the side that allocates (Rust) must free via ffi_buffer_free.
 * Never alias or copy without transferring ownership.
 */
typedef struct {
    uint8_t *data;
    size_t   len;
    size_t   capacity;
} FfiBuffer;

/**
 * FFI-safe UTF-8 string (NOT necessarily null-terminated beyond len).
 * Use len, not strlen, to determine the string length.
 */
typedef struct {
    char  *data;
    size_t len;
} FfiString;

/**
 * FFI result wrapper — carries either a success payload or an error.
 * Always call ffi_result_free when done, even on success.
 */
typedef struct {
    FfiErrorCode error_code;
    FfiString    error_message;  /* empty (data=NULL, len=0) on success */
    FfiBuffer    payload;        /* empty on error */
} FfiResult;

/* ═══════════════════════════════════════════════════════════
 * Buffer Lifecycle
 * ═══════════════════════════════════════════════════════════ */

/** Allocate a buffer of `capacity` bytes. Returns zeroed FfiBuffer on failure. */
FfiBuffer ffi_buffer_alloc(size_t capacity);

/** Free a buffer allocated by ffi_buffer_alloc. Safe to call on zeroed buffer. */
void ffi_buffer_free(FfiBuffer buf);

/* ═══════════════════════════════════════════════════════════
 * String Lifecycle
 * ═══════════════════════════════════════════════════════════ */

/** Allocate and copy a UTF-8 string of `len` bytes. */
FfiString ffi_string_alloc(const char *str, size_t len);

/** Free a string allocated by ffi_string_alloc. */
void ffi_string_free(FfiString str);

/* ═══════════════════════════════════════════════════════════
 * Result Lifecycle
 * ═══════════════════════════════════════════════════════════ */

/** Free an FfiResult and all resources it owns (message + payload). */
void ffi_result_free(FfiResult result);

/* ═══════════════════════════════════════════════════════════
 * Callback Interface
 * ═══════════════════════════════════════════════════════════ */

/** Callback function signature: takes an input buffer, returns an FfiResult. */
typedef FfiResult (*FfiCallback)(FfiBuffer input);

/**
 * Register a named callback.
 * Returns 0 on success, -1 if `name` is not valid UTF-8.
 * Thread-safe: uses a global Mutex-guarded registry.
 */
int32_t ffi_register_callback(const char *name, FfiCallback cb);

/**
 * Invoke a previously registered callback by name.
 * Returns FFI_ERR_NOT_FOUND if the name has not been registered.
 * Panic-safe: any Rust panic is caught and returned as FFI_ERR_PANIC.
 */
FfiResult ffi_invoke_callback(const char *name, FfiBuffer input);

/**
 * Remove a registered callback by name.
 * Returns 0 if removed, -1 if not found.
 */
int32_t ffi_unregister_callback(const char *name);

/**
 * Return the number of currently registered callbacks.
 */
size_t ffi_callback_count(void);

/* ═══════════════════════════════════════════════════════════
 * Bridge Utilities
 * ═══════════════════════════════════════════════════════════ */

/**
 * Echo the input buffer back as output. Useful for round-trip tests
 * and benchmarking FFI overhead.
 */
FfiResult ffi_echo(FfiBuffer input);

/**
 * Return the ffi-bridge crate version as a byte buffer.
 * The caller must free the returned buffer with ffi_buffer_free.
 */
FfiBuffer ffi_version(void);

#ifdef __cplusplus
}
#endif

#endif /* FFI_BRIDGE_H */
