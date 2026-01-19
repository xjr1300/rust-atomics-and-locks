use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering, compiler_fence};

fn main() {
    let locked = AtomicBool::new(false);
    let counter = AtomicUsize::new(0);

    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                for _ in 0..1_000_000 {
                    // ロックを取得する。
                    // ただし、メモリオーダリングが間違っている。
                    while locked.swap(true, Ordering::Relaxed) {}
                    // コンパイラのメモリオーダリングのみを抑制しており、CPUレベルのメモリオーダリングは抑制していない。
                    compiler_fence(Ordering::Acquire);

                    // ロックを保持したまま、非アトミックにカウンタをインクリメントする。
                    let old = counter.load(Ordering::Relaxed);
                    let new = old + 1;
                    counter.store(new, Ordering::Relaxed);

                    // ロックを解放する。
                    // ただし、メモリオーダリングが間違っている。
                    // コンパイラのメモリオーダリングのみを抑制しており、CPUレベルのメモリオーダリングは抑制していない。
                    compiler_fence(Ordering::Release);
                    locked.store(false, Ordering::Relaxed);
                }
            });
        }
    });

    println!("{}", counter.into_inner());

    // 正しく計算するためには、以下のようにメモリオーダリングを修正する必要がある。
    let locked = AtomicBool::new(false);
    let counter = AtomicUsize::new(0);

    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                for _ in 0..1_000_000 {
                    // ロックを取得する。
                    // Acquireオーダーリングにより、lockedがfalseであることを観測した場合、
                    // 対応するReleaseストアより前のメモリ操作が**この**スレッドの後続のコードから観測可能になる。
                    // Acquire: 受信する（このスレッドから見えるようにする）
                    // Acquire操作より後のメモリ操作が、このコードより前に発生しない。
                    while locked.swap(true, Ordering::Acquire) {}

                    // ロックを保持したまま、非アトミックにカウンタをインクリメントする。
                    let old = counter.load(Ordering::Relaxed);
                    let new = old + 1;
                    counter.store(new, Ordering::Relaxed);

                    // ロックを解放する。
                    // これより前のメモリ操作が**他の**スレッドから観測可能になる。
                    // Release: 公開する（他のスレッドから見えるようにする）
                    // Release操作より前のメモリ操作が、このコードより後に発生しない。
                    locked.store(false, Ordering::Release);
                }
            });
        }
    });

    println!("{}", counter.into_inner());
}
