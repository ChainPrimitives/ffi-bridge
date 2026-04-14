// Benchmarks for ffi-bridge core operations.
//
// Run with:
//   cargo bench
//
// Or for a specific benchmark:
//   cargo bench -- buffer_alloc

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ffi_bridge::*;

// ─── Buffer benchmarks ────────────────────────────────────────────────────────

fn bench_buffer_alloc_free(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_alloc_free");
    for size in [64usize, 256, 1024, 4096, 65536] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let buf = ffi_buffer_alloc(black_box(size));
                ffi_buffer_free(buf);
            });
        });
    }
    group.finish();
}

fn bench_buffer_from_vec(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_from_vec");
    for size in [64usize, 1024, 65536] {
        let data = vec![0u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| {
                let buf = FfiBuffer::from_vec(black_box(data.clone()));
                ffi_buffer_free(buf);
            });
        });
    }
    group.finish();
}

// ─── JSON round-trip benchmarks ───────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct BenchPayload {
    id: u64,
    name: String,
    values: Vec<i32>,
}

fn bench_json_round_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_round_trip");

    let small = BenchPayload { id: 1, name: "small".into(), values: vec![1, 2, 3] };
    let large = BenchPayload {
        id: 999,
        name: "large_payload_with_longer_name".into(),
        values: (0..100).collect(),
    };

    group.bench_function("small", |b| {
        b.iter(|| {
            let buf = FfiBuffer::from_json(black_box(&small)).unwrap();
            let _: BenchPayload = unsafe { buf.to_json() }.unwrap();
            ffi_buffer_free(buf);
        });
    });

    group.bench_function("large", |b| {
        b.iter(|| {
            let buf = FfiBuffer::from_json(black_box(&large)).unwrap();
            let _: BenchPayload = unsafe { buf.to_json() }.unwrap();
            ffi_buffer_free(buf);
        });
    });

    group.finish();
}

// ─── ffi_echo benchmark ───────────────────────────────────────────────────────

fn bench_ffi_echo(c: &mut Criterion) {
    let mut group = c.benchmark_group("ffi_echo");
    for size in [64usize, 1024, 65536] {
        let data = vec![0xABu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| {
                let input = FfiBuffer::from_vec(black_box(data.clone()));
                let result = ffi_echo(input);
                ffi_result_free(result);
            });
        });
    }
    group.finish();
}

// ─── catch_panic overhead ─────────────────────────────────────────────────────

fn bench_catch_panic_ok(c: &mut Criterion) {
    c.bench_function("catch_panic_ok_path", |b| {
        b.iter(|| {
            let result = catch_panic(|| Ok(FfiBuffer::from_vec(b"ok".to_vec())));
            ffi_result_free(result);
        });
    });
}

// ─── Callback invoke benchmark ────────────────────────────────────────────────

fn bench_callback_invoke(c: &mut Criterion) {
    // Register once before benchmarking
    register_callback("bench.echo", |buf| {
        let bytes = unsafe { buf.as_slice() }.to_vec();
        FfiResult::ok(FfiBuffer::from_vec(bytes))
    })
    .ok(); // ignore if already registered from a previous run

    let input_data = b"benchmark input payload".to_vec();

    c.bench_function("callback_invoke", |b| {
        b.iter(|| {
            let input = FfiBuffer::from_vec(black_box(input_data.clone()));
            let result = unsafe {
                let name = std::ffi::CString::new("bench.echo").unwrap();
                ffi_invoke_callback(name.as_ptr(), input)
            };
            ffi_result_free(result);
        });
    });
}

// ─── String benchmarks ────────────────────────────────────────────────────────

fn bench_ffi_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("ffi_string");

    group.bench_function("alloc_free_short", |b| {
        b.iter(|| {
            let s = FfiString::new(black_box("hello world"));
            ffi_string_free(s);
        });
    });

    let long_str = "a".repeat(256);
    group.bench_function("alloc_free_long", |b| {
        b.iter(|| {
            let s = FfiString::new(black_box(long_str.as_str()));
            ffi_string_free(s);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_buffer_alloc_free,
    bench_buffer_from_vec,
    bench_json_round_trip,
    bench_ffi_echo,
    bench_catch_panic_ok,
    bench_callback_invoke,
    bench_ffi_string,
);
criterion_main!(benches);
