//! 条件変数`CondVar`は、スレッドが特定の条件を満たすまで待機するための同期プリミティブである。
//! 条件変数をウェイトした場合、そのスレッドは別のスレッドが`notify_one`や`notify_all`を呼び出すまで待機する。
//!
//! 条件変数は、シグナルが送られるまで待機中のスレッドをそのままにしておこうとするが、対応するシグナルが送られなくても、
//! スレッドを起こす場合がある。
//! これは、OSのプリミティブに依存する実際の実装では、`notify`が呼ばれなくても、待機中のスレッドを起こす仕様になっている。
//! つまり、「シグナルを受け取ったから起きたわけではない」場合がある。
//! これを、スプリアスウェイクアップ（spurious wakeup、偽りの起床）と呼ぶ。
//! スプリアスウェイクアップを考慮して、安定に条件変数を使用するためには、待機をループで囲む必要がある。
//!
//! 条件変数は、状態を持たず、条件が成立したことを保証しない。
//! 条件が成立したことを保証するためには、その条件を状態としてラップする`Mutex<T>`などと一緒に使用する。
//!
//! ```rust
//! // タプルの1つ目の要素が条件を表す`Mutex<bool>`、2つ目の要素が`Condvar`である。
//! // 条件が`true`になるまで、消費者は待機する。
//! let pair = Arc::new((Mutex::new(false), Condvar::new()));
//! let pair2 = Arc::clone(&pair);
//!
//! // プロデューサー側
//! let handle = std::thread::spawn(move || {
//!     let (lock, condvar) = &*pair;
//!     let mut started = lock.lock().unwrap();
//!     *started = true;         // 準備ができた
//!     condvar.notify_one();    // 通知
//! });
//!
//! // 消費者側
//! let (lock, condvar) = &*pair2;
//! let mut started = lock.lock().unwrap();
//!
//! // **条件変数の誤った使用方法**
//! // * notifyが来たとは限らない
//! // * 起きた時点で条件が満たされている保証はない
//! // * 他スレッドが先に条件を消費している可能性がある（別スレッドが`started`を`true`にした後、`false`にした）
//! if !started {
//!     started = condvar.wait(started).unwrap();
//! }
//! // アサーションに失敗することがある。
//! assert_eq!(*started, true);
//!
//! // **条件変数の正しい使用方法**
//! // 起こされたら再度条件を確認する。
//! while !started {
//!    started = condvar.wait(started).unwrap();
//! }
//!
//! handle.join().unwrap();
//!
//! // アサーションに成功することが保証される。
//! assert!(*started);
//! ```
//! 条件変数のウェイトは次の順番で動作する。
//! 1. `Mutex<T>`をアンロック
//! 2. スレッドをスリープ
//! 3. `notify`またはスプリアスウェイクアップにより起床
//! 4. `Mutex<T>`を再度ロック
//! 5. `wait`がリターン
//!
//! したがって、条件変数のウェイト操作から戻ってきたとき、必ず`Mutex<T>`をロックしていることが保証される。
use std::sync::atomic::{AtomicU32, Ordering};

use atomic_wait::{wait, wake_all, wake_one};

use rust_atomics_and_locks::mutex::MutexGuard;

#[derive(Default)]
pub struct Condvar {
    counter: AtomicU32,
}

impl Condvar {
    pub fn notify_one(&self) {
        self.counter.fetch_add(1, Ordering::Relaxed);
        wake_one(&self.counter);
    }

    pub fn notify_all(&self) {
        self.counter.fetch_add(1, Ordering::Relaxed);
        wake_all(&self.counter);
    }

    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        let counter_value = self.counter.load(Ordering::Relaxed);

        // ガードをドロップしてアンロックする。
        // ただし、後でロックするためにミューテックスを記憶する。
        let mutex = guard.mutex;
        drop(guard);

        // 上記dropでミューテックスをアンロックしてから、notify側でカウンタ値
        // がインクリメントされる場合がある。つまり、メソッド先頭のカウンタ値の
        // Relaxedな読み込みは、notify側のカウンタ値のインクリメントよりも前に
        // 発生する先行発生関係を形成する。

        // 上記dropでミューテックスをアンロックした後、notify側でカウンタ値が
        // インクリメントされた場合、「通知が来た」ことを示すため、waitは待機しない。
        // 「通知が来ていない」場合、つまりメソッドの先頭で読み込んだカウンタ値と
        // 同じ値をatomic_wait::waitが読み込んだときのみ待機する。
        wait(&self.counter, counter_value);

        // ミューテックスをロックして、ガードを返す。
        mutex.lock()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use rust_atomics_and_locks::mutex::Mutex;

    #[test]
    fn test_condvar() {
        let mutex = Mutex::new(0);
        let condvar = Condvar::default();

        let mut wakeups = 0;

        std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_secs(1));
                *mutex.lock() = 123;
                condvar.notify_one();
            });

            let mut m = mutex.lock();
            while *m < 100 {
                m = condvar.wait(m);
                wakeups += 1;
            }

            assert_eq!(*m, 123);
        });

        // メインスレッドがウェイトしたことを示す。
        // ただし、スプリアスウェイクアップの発生を許容するため、本来起こされる回数
        // は1回であるが、10回未満であることをアサートする。
        assert!(wakeups < 10);
    }
}

fn main() {}
