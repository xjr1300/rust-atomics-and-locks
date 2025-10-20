use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
use std::thread;

static X: AtomicI32 = AtomicI32::new(0);

fn main() {
    // --- 先行関係の開始 ---
    // `spawn`によるスレッドの開始と、スレッド開始前までに発生したことは先行関係を持つ。
    X.store(1, Relaxed);
    let t = thread::spawn(f);
    // --- 先行関係の終わり ---
    // スレッドが起動した時点で先行関係が発生し、Xは必ず1になっている。

    // 次のコードは先行関係を持たないため、`f`関数内のアサートで、Xは1の場合と2の場合があり得る。
    X.store(2, Relaxed);

    // --- 先行関係の開始 ---
    // `join`によるスレッドの大気と、スレッド待機後に発生したことは先行関係を持つ。
    t.join().unwrap();
    X.store(3, Relaxed);
    // --- 先行関係の終わり ---
}

fn f() {
    let x = X.load(Relaxed);
    assert!(x == 1 || x == 2);
}
