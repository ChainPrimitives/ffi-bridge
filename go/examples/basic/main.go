// Command basic demonstrates the ffi-bridge Go module.
//
// It shows how to:
//   - Create an FFI buffer from Go bytes
//   - Serialize/deserialize JSON through the FFI boundary
//   - Invoke a Rust-registered callback from Go
//   - Read the Rust library version
//
// Build requirements:
//
//	cd ../../rust && cargo build --release
//	cd ../.. && CGO_ENABLED=1 go run ./go/examples/basic/main.go
package main

import (
	"fmt"
	"log"

	ffibridge "github.com/Subaskar-S/ffi-bridge"
)

type MathRequest struct {
	A  int    `json:"a"`
	B  int    `json:"b"`
	Op string `json:"op"`
}

type MathResponse struct {
	Result int `json:"result"`
}

func main() {
	fmt.Println("ffi-bridge Go example")
	fmt.Println("==========================================")

	// ── 1. Check the linked Rust library version ───────────────────────────────
	ver := ffibridge.Version()
	fmt.Printf("✓ Linked Rust crate version: %s\n", ver)

	// ── 2. Buffer from bytes ───────────────────────────────────────────────────
	buf := ffibridge.FromBytes([]byte("hello from Go"))
	defer buf.Free()

	fmt.Printf("✓ Buffer: len=%d, bytes=%q\n", buf.Len(), buf.Bytes())

	// ── 3. JSON round-trip ─────────────────────────────────────────────────────
	req := MathRequest{A: 21, B: 21, Op: "add"}
	jsonBuf, err := ffibridge.FromJSON(req)
	if err != nil {
		log.Fatalf("FromJSON: %v", err)
	}
	defer jsonBuf.Free()

	var decoded MathRequest
	if err := jsonBuf.ToJSON(&decoded); err != nil {
		log.Fatalf("ToJSON: %v", err)
	}
	fmt.Printf("✓ JSON round-trip: %+v\n", decoded)

	// ── 4. Invoke Rust callback ────────────────────────────────────────────────
	// The "math.add" callback was registered in rust/examples/basic.rs.
	// In a real scenario you'd link against a shared library that registers it
	// at startup (e.g. via an init() or a dedicated setup function).
	//
	// Here we demonstrate the API; actual cross-process invocation requires
	// Rust to have initialized the callback registry first.
	fmt.Println()
	fmt.Println("Note: callback invocation requires the Rust library to have")
	fmt.Println("registered 'math.add' during its initialization.")
	fmt.Println()
	fmt.Println("If the Rust dylib is loaded and initialized, run:")
	fmt.Println("  var resp MathResponse")
	fmt.Println("  err := ffibridge.InvokeCallbackJSON(\"math.add\", req, &resp)")

	// ── 5. Error code display ──────────────────────────────────────────────────
	codes := []ffibridge.ErrorCode{
		ffibridge.ErrOK,
		ffibridge.ErrNullPointer,
		ffibridge.ErrPanic,
		ffibridge.ErrTimeout,
		ffibridge.ErrNotFound,
	}
	fmt.Println()
	fmt.Println("Error codes:")
	for _, c := range codes {
		fmt.Printf("  %3d → %s\n", int(c), c.String())
	}

	fmt.Println()
	fmt.Println("==========================================")
	fmt.Println("Example complete ✓")
}
