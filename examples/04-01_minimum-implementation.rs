use std::sync::atomic::{AtomicBool, Ordering};

// #[derive(Default)]
pub struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    // pub const fn new() -> Self {
    //     Self {
    //         locked: AtomicBool::new(false),
    //     }
    // }
    pub const fn default() -> Self {
        Self {
            // locked: AtomicBool::new(true),
            locked: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) {
        // `swap`は、現在の値を返すので、もし`locked`が`true`ならループを続ける。
        // while self.locked.swap(true, Ordering::Acquire) {
        //     std::hint::spin_loop();
        // }

        // `compare_exchange_weak`は、現在の値が引数で与えた`current`（現在値、期待値）と等しい場合、
        // 引数で与えた`new`に値を更新して`Ok(current)`を返す。
        // 逆に等しくない場合は`Err(actual)`を返し、`actual`は現在の値を示す。
        //
        // `compare_exchange_weak`は、`compare_exchange`と異なり、比較に成功したときでも
        // 偽の失敗（spuriously fail: スプリアスフェイス）をすることが許される。
        // したがって、LL/SC(Load-Linked/Store-Conditional)命令を持つ一部のプラットフォーム
        // では、余分な内部ループを持たずに実装でき、より効率的なコードが生成される可能性がある。
        //
        // `compare_exchange_weak`は、期待値、現在値と等しくても失敗する可能性があるため、
        // ループ内で使用することが前提となっている。
        // ループ内で使用することで、失敗しても他のスレッドに負けたのか、それとも偽の失敗なのかを区別
        // する必要がなく、単に再試行すれば良いだけである。
        //
        // ```rust
        // use std::sync::atomic::{AtomicBool, Ordering};
        //
        // let val = AtomicBool::new(false);
        // let new = true;
        // let mut old = val.load(Ordering::Relaxed);
        // loop {
        //     match val.compare_exchange_weak(old, new, Ordering::SeqCst, Ordering::Relaxed) {
        //         Ok(_) => break,
        //         Err(x) => old = x,
        //     }
        // }
        // ```
        //
        // 2つの`Ordering`引数は、この操作が他のメモリ操作とどのような順序関係を持つかを指定する。
        // `success`引数は、比較に成功し、値の更新が行われた場合に、この操作が前後のメモリ操作と
        // どのように順序付けられるかを指定する。
        // `failure`引数は、比較が失敗し、値が更新されなかった場合に行われる読み取り操作の順序付け
        // を指定する。
        //
        // 次の`compare_exchange_weak`は、成功時に`Acquire`を失敗しに`Relaxed`を指定している。
        // これにより、ロック取得後の読み書き操作が、ロック解放前の書き込みを確実に観測できるようになる。
        // 一方、失敗時は値の更新がされないため、`Relaxed`で十分である。
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
    }

    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

fn main() {
    let lock = SpinLock::default();
    lock.lock();
    // クリティカルセクション
    std::thread::sleep(std::time::Duration::from_secs(1));
    println!("1 second passed in critical section");
    lock.unlock();
}
