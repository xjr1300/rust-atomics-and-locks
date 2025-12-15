use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

/// Guard
///
/// GuardはSpinLockよりも長生きできない。
/// Guardは`Deref`と`DerefMut`を実装しているため、ロック保持中に`T`への不変参照および可変参照を提供する。
/// Guard自体をスレッド間で送受信・共有できるようにするため、 別途`Send`および`Sync`のunsafe実装により`T`への制約を課している。
pub struct Guard<'a, T> {
    lock: &'a SpinLock<T>,
}

/// `UnsafeCell<T>`は`Sync`でないため、コンパイラは`SpinLock<T>`を動的に`Sync`であることを判断できない。
/// しかし、`SpinLock<T>`は内部可変性がスピンロックによって適切に同期されており、`T: Send`である限り、
/// 複数スレッドから`SpinLock<T>`にアクセスしても安全である。
/// その安全性をプログラマが保証して、それをコンパイラーに伝えるために、`unsafe impl`を使用して`Sync`を実装する。
unsafe impl<T> Sync for SpinLock<T> where T: Send {}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> Guard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {
            std::hint::spin_loop();
        }
        Guard { lock: self }
    }
}

/// `'_`は、この実装がGuardのライフタイム引数に依存せず、`'static`を含めてすべてのライフタイムに
/// 対して同一に成立することを示す。
/// これは `impl<'a, T> Deref for Guard<'a, T>` と等価である。
impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // `UnsafeCell::get`は`*mut T`、つまり可変な`T`へのポインタを返す。
        // しかし、`Deref`トレイトの`deref`メソッドは不変参照を返す必要があるため、
        // 不変参照に変換する。
        unsafe { &*self.lock.value.get() }
    }
}

/// `DerefMut`は`Deref`を継承するトレイトであり、`Target`関連型は`Deref`側で定義されたものをそのまま使用する。
/// そのため、`DerefMut`を実装する型は必ず`Deref`も実装している必要がある。
impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.value.get() }
    }
}

unsafe impl<T> Send for Guard<'_, T> where T: Send {}
unsafe impl<T> Sync for Guard<'_, T> where T: Sync {}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

fn main() {
    let x = SpinLock::new(Vec::new());
    std::thread::scope(|s| {
        s.spawn(|| x.lock().push(1));
        s.spawn(|| {
            let mut guard = x.lock();
            // DerefMutが実装されているため、`guard`は`&mut Vec<T>`（可変参照）に自動的に変換される。
            guard.push(2);

            // 次の`drop(guard)`のコメントアウトを外すと、`guard`がドロップされているため、次の`guard.push()`で
            // コンパイルエラーが発生する。
            //
            // ```text
            // drop(guard);
            //      ----- value moved here
            // guard.push(3);
            // ^^^^^ value borrowed here after move
            // drop(guard);
            // ```
            //
            // drop(guard);

            guard.push(3);
        });
    });
    let guard = x.lock();
    // Derefが実装されているため、`guard`は`&Vec<T>`（不変参照）に自動的に変換される。
    assert!(guard.as_slice().contains(&1));
    assert!(guard.as_slice().contains(&2));
    assert!(guard.as_slice().contains(&3));
}
