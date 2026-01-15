use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};

static A: AtomicU64 = AtomicU64::new(0);

// fn main() {
//     black_box(&A);
//     let start = std::time::Instant::now();
//     for _ in 0..1_000_000_000 {
//         black_box(A.load(Ordering::Relaxed));
//     }
//     println!("Elapsed: {:?}", start.elapsed());
// }

fn main() {
    black_box(&A);

    std::thread::spawn(|| {
        loop {
            // black_box(A.load(Ordering::Relaxed));
            A.store(0, Ordering::Relaxed);
        }
    });

    let start = std::time::Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(A.load(Ordering::Relaxed));
    }

    println!("Elapsed: {:?}", start.elapsed());
}
