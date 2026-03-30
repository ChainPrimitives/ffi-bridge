package ffibridge_test

import (
	"encoding/json"
	"testing"

	ffibridge "github.com/Subaskar-S/ffi-bridge"
)

// ─── Buffer tests ─────────────────────────────────────────────────────────────

func TestNewBuffer(t *testing.T) {
	buf := ffibridge.NewBuffer(64)
	defer buf.Free()

	if buf.IsEmpty() == false && buf.Len() != 0 {
		t.Fatalf("new buffer should have len=0, got %d", buf.Len())
	}
}

func TestFromBytes(t *testing.T) {
	data := []byte("hello ffi bridge")
	buf := ffibridge.FromBytes(data)
	defer buf.Free()

	if buf.Len() != len(data) {
		t.Fatalf("expected len=%d, got %d", len(data), buf.Len())
	}

	got := buf.Bytes()
	if string(got) != string(data) {
		t.Fatalf("expected %q, got %q", data, got)
	}
}

func TestFromBytes_Empty(t *testing.T) {
	buf := ffibridge.FromBytes([]byte{})
	defer buf.Free()

	if !buf.IsEmpty() {
		t.Fatal("buffer from empty slice should be empty")
	}
}

func TestFromJSON_AndToJSON(t *testing.T) {
	type Payload struct {
		Name  string `json:"name"`
		Value int    `json:"value"`
	}

	original := Payload{Name: "bridge", Value: 42}

	buf, err := ffibridge.FromJSON(original)
	if err != nil {
		t.Fatalf("FromJSON: %v", err)
	}
	defer buf.Free()

	var decoded Payload
	if err := buf.ToJSON(&decoded); err != nil {
		t.Fatalf("ToJSON: %v", err)
	}

	if decoded.Name != original.Name || decoded.Value != original.Value {
		t.Fatalf("round-trip mismatch: got %+v, want %+v", decoded, original)
	}
}

func TestBuffer_Free_Idempotent(t *testing.T) {
	buf := ffibridge.FromBytes([]byte("test"))
	buf.Free()
	buf.Free() // Must not panic or crash
}

func TestBuffer_Bytes_AfterFree(t *testing.T) {
	buf := ffibridge.FromBytes([]byte("test"))
	buf.Free()
	bytes := buf.Bytes()
	if bytes != nil {
		t.Fatal("Bytes() should return nil after Free()")
	}
}

// ─── Error code tests ─────────────────────────────────────────────────────────

func TestErrorCode_String(t *testing.T) {
	cases := []struct {
		code ffibridge.ErrorCode
		want string
	}{
		{ffibridge.ErrOK, "ok"},
		{ffibridge.ErrNullPointer, "null_pointer"},
		{ffibridge.ErrBufferTooSmall, "buffer_too_small"},
		{ffibridge.ErrInvalidUTF8, "invalid_utf8"},
		{ffibridge.ErrSerialization, "serialization"},
		{ffibridge.ErrPanic, "panic"},
		{ffibridge.ErrTimeout, "timeout"},
		{ffibridge.ErrNotFound, "not_found"},
		{ffibridge.ErrLockPoisoned, "lock_poisoned"},
		{ffibridge.ErrUnknown, "error_99"},
	}

	for _, tc := range cases {
		if tc.code.String() != tc.want {
			t.Errorf("ErrorCode(%d).String() = %q, want %q", tc.code, tc.code.String(), tc.want)
		}
	}
}

func TestFfiError_Error(t *testing.T) {
	err := &ffibridge.FfiError{
		Code:    ffibridge.ErrPanic,
		Message: "test panic message",
	}
	s := err.Error()
	if s == "" {
		t.Fatal("Error() should return non-empty string")
	}
	if len(s) < 10 {
		t.Fatalf("Error() string too short: %q", s)
	}
}

// ─── JSON round-trip helpers ──────────────────────────────────────────────────

func TestFromJSON_InvalidInput(t *testing.T) {
	ch := make(chan int) // channels are not JSON-serializable
	_, err := ffibridge.FromJSON(ch)
	if err == nil {
		t.Fatal("expected error marshaling channel, got nil")
	}
}

func TestBuffer_ToJSON_EmptyBuffer(t *testing.T) {
	buf := ffibridge.NewBuffer(0)
	defer buf.Free()

	var v map[string]any
	err := buf.ToJSON(&v)
	if err == nil {
		t.Fatal("expected error deserializing empty buffer")
	}
}

// ─── Benchmark ────────────────────────────────────────────────────────────────

func BenchmarkFromBytes_1K(b *testing.B) {
	data := make([]byte, 1024)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		buf := ffibridge.FromBytes(data)
		buf.Free()
	}
}

func BenchmarkFromJSON(b *testing.B) {
	type Msg struct {
		ID    int    `json:"id"`
		Value string `json:"value"`
	}
	msg := Msg{ID: 1, Value: "benchmark"}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		buf, _ := ffibridge.FromJSON(msg)
		buf.Free()
	}
}

// Helper to ensure JSON round-trip is used in benchmark context.
var _ = json.Marshal // avoid import errors
