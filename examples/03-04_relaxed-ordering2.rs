use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
use std::thread;
use std::time::Duration;

static X: AtomicI32 = AtomicI32::new(0);

fn a1() {
    X.fetch_add(5, Relaxed);
    thread::sleep(Duration::from_nanos(1));
}

fn a2() {
    X.fetch_add(10, Relaxed);
    thread::sleep(Duration::from_nanos(1));
}

fn b() {
    let a = X.load(Relaxed);
    thread::sleep(Duration::from_nanos(1));
    let b = X.load(Relaxed);
    thread::sleep(Duration::from_nanos(1));
    let c = X.load(Relaxed);
    thread::sleep(Duration::from_nanos(1));
    let d = X.load(Relaxed);
    println!("{a}, {b}, {c}, {d}");
}

fn main() {
    let handle_b = thread::spawn(b);
    let handle_a2 = thread::spawn(a2);
    let handle_a1 = thread::spawn(a1);
    handle_a1.join().unwrap();
    handle_a2.join().unwrap();
    handle_b.join().unwrap();
}
