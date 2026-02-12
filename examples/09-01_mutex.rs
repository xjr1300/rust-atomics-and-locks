use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};

use atomic_wait::{wait, wake_one};

pub struct Mutex<T> {
    /// 0: ロックされていない状態
    /// 1: ロックされている状態
    state: AtomicU32,
    value: UnsafeCell<T>,
}

/// `Mutex`は複数スレッドから共有参照（`&Mutex`）で同時に使われることを前提としている。
///
/// `Send`は、値を別スレッドにムーブできることを示すトレイトである。
/// `Sync`は、共有参照（`&T`）を複数スレッドで同時に使えることを示すトレイトである。
///
/// `Mutex<T>`は、次のように使用されることを想定している。
///
/// ```rust
/// let m = Mutex::new(0);
/// let m_ref = &m; // 共有参照を取得
///
/// std::thread::scope(|s| {
///     s.spawn(|| {
///         let mut guard = m_ref.lock();
///         *guard += 1;
///     });
///     s.spawn(|| {
///         let mut guard = m_ref.lock();
///     });
/// });
/// ```
///
/// 上記コードで起きていることは次の通り。
///
/// * それぞれのスレッドは`&Mutex<T>`の共有参照を持っている。
/// * `Mutex<T>`を使用して排他的に`T`にアクセスしている。
///
/// したがって、`Mutex<T>`は、`&Mutex<T>`を複数スレッドに渡す型である必要がある。
/// このため、`&Mutex<T>`が複数スレッドで同時にアクセスできることを示す`Sync`を`Mutex<T>`に実装する必要がある。
///
/// ロック取得後、`Mutex<T>`は、そのロックを取得したスレッドだけが、実質的に`T`を所有している状態になる。
/// これは、ガードを通じてこのスレッドに対し、`T`への排他的な可変アクセス権を与えている状態であり、スレッド間の観点では
/// `T`を移動させたのと同等の制約が課される。
/// したがって、`T`が別スレッドにムーブ可能であることを示す`Send`を`T`に実装する必要がある。
///
/// また、`T`が`Sync`を要求していない理由は、`Mutex<T>`が`T`として`Sync`でない`Cell<T>`のような型を
/// ラップできるようにするためである。
/// これは、`T`へのアクセスはロックを通じて排他的に行われるため、同時アクセスが発生しないから許される。
///
/// `Cell<T>`や`RefCell<T>`は`Sync`を実装していない内部可変性を提供する型である。
/// `Cell<T>`はコピーによる値の取得や設定を許可し、`RefCell<T>`は実行時に借用規則を検査する。
/// 上記の通り、`Mutex<T>`は`T`へのアクセスを排他的に行うため、`T`が`Sync`でなくても問題ない。
unsafe impl<T> Sync for Mutex<T> where T: Send {}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

/// ライフタイムパラメータ`'a`でなく`'_`となっている理由は、`impl`では
/// ライフタイム名ではなく、ライフタイムが存在することを示すことが本質だからである。
/// したがって、どのようなライフタイムであっても問題ないことを示すために`'_`を使う。
unsafe impl<T> Sync for MutexGuard<'_, T> where T: Sync {}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.value.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.value.get() }
    }
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0), // ロックされていない状態で初期化
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        // stateを1（ロックされている）にセット
        while self.state.swap(1, Ordering::Acquire) == 1 {
            // すでにロックされていたら、stateが1でなくなるまで待機
            wait(&self.state, 1);
        }
        MutexGuard { mutex: self }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // stateを0（ロックされていない）にセット
        self.mutex.state.swap(0, Ordering::Release);
        // 待機中のスレッドがあれば、1つだけ起こす
        wake_one(&self.mutex.state);
    }
}
