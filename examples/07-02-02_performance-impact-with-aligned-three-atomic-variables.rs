use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(align(64))]
struct Aligned(AtomicU64);

static A: [Aligned; 3] = [
    Aligned(AtomicU64::new(0)),
    Aligned(AtomicU64::new(0)),
    Aligned(AtomicU64::new(0)),
];

fn main() {
    black_box(&A);

    std::thread::spawn(|| {
        loop {
            A[0].0.store(1, Ordering::Relaxed);
            A[2].0.store(1, Ordering::Relaxed);
        }
    });

    let start = std::time::Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(A[1].0.load(Ordering::Relaxed));
    }

    println!("Elapsed: {:?}", start.elapsed());
}
