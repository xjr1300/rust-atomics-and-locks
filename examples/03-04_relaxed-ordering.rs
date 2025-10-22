use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
use std::thread;
use std::time::Duration;

static X: AtomicI32 = AtomicI32::new(0);

fn a() {
    thread::sleep(Duration::from_nanos(1));
    X.fetch_add(5, Relaxed);
    thread::sleep(Duration::from_nanos(1));
    X.fetch_add(10, Relaxed);
}

fn b() {
    let a = X.load(Relaxed);
    thread::sleep(Duration::from_nanos(1));
    let b = X.load(Relaxed);
    thread::sleep(Duration::from_nanos(1));
    let c = X.load(Relaxed);
    let d = X.load(Relaxed);
    println!("{a}, {b}, {c}, {d}");
}

fn main() {
    let handle_a = thread::spawn(a);
    let handle_b = thread::spawn(b);
    handle_a.join().unwrap();
    handle_b.join().unwrap();
}
