use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

static mut DATA: String = String::new();
static LOCKED: AtomicBool = AtomicBool::new(false);

fn f() {
    // LOCKED変数の値をcompare_exchangeで読み取り、falseであればtrueに変更する。
    // 成功時はAcquireオーダリング、失敗時はRelaxedオーダリングを使用する。
    //
    // Acquireオーダリングにより、compare_exchange成功時に以下が保証される。
    // - このcompare_exchangeより前に他のスレッドがLOCKEDへReleaseオーダリング
    //   でfalseを書き込んだ場合、そのReleaseより前のメモリ操作（DATAへの変更など）が
    //   このスレッドで観測可能になる。
    //
    // LOCKED変数の書き換えに失敗した場合、DATA変数へのアクセスはない。
    //
    // したがって、このスレッドがLOCKEDをtrueに設定した後は、他のスレッドが同時にDATAに
    // アクセスすることはない（ロックが取得された状態）。
    //
    // ReleaseストアとAcquireロード間で確立する「先行発生関係（happens-before）は、
    // アトミックでない通常の変数にも影響することに注意すること。
    if LOCKED
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        unsafe {
            #[allow(static_mut_refs)]
            DATA.push('!');
        }
        // ロックを解放する。Releaseオーダリングにより、このstoreより前に
        // 行われたメモリ操作（DATAへの書き込み）が、次にこのLOCKED変数を
        // Acquireオーダリングで読み取るスレッドから観測可能になる。
        LOCKED.store(false, Ordering::Release);
    }
}

fn main() {
    thread::scope(|s| {
        for _ in 0..100 {
            s.spawn(f);
        }
    })
}
