use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

/// `Sync`の実装は、安全性の契約を満たすために`unsafe`である必要がある。
///
/// `Send`を実行するすべての型`T`に対して、`SpinLock<T>`は`Sync`を実装
/// していなければならないことを、コンパイラに伝える。
///
/// `T`は`SpinLock`によって保護されているため、`T`に対するアクセスは1つの
/// スレッドに限定される。したがって、`T`が`Sync`であることを要求せず、
/// `SpinLock<T>`が`Sync`であることを求めていることに注意すること。
/// リーダ・ライタロックのように、複数のスレッドが同時にアクセスすることを許可
/// する場合のみ、`T`に対して`Sync`を要求する必要がある。
unsafe impl<T> Sync for SpinLock<T> where T: Send {}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    /// 返却される可変参照のライフタイムは、このインスタンスと同じであるため、ライフタイムの
    /// 省略規則により、ライフタイム注釈は不要である。
    #[allow(clippy::mut_from_ref)]
    pub fn lock(&self) -> &mut T {
        while self.locked.swap(true, Ordering::Acquire) {
            std::hint::spin_loop();
        }
        // `UnsafeCell::get`は`*mut T`、つまり可変な`T`へのポインタを返す。
        // したがって、`*`を使用して参照外しをした後、その可変参照を返す。
        unsafe { &mut *self.value.get() }
    }

    /// # Safety
    ///
    /// `lock()`が返した`&mut T`が使用されておらず、なくなっていなくてならない。
    pub unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

fn main() {
    let lock = SpinLock::new(0);

    // ロックを獲得して、値を更新
    let value = lock.lock();
    *value += 1;

    // ロックを解放
    unsafe {
        lock.unlock();
    }

    // ここで値を読み取ろうとすると未定義動作になる。
    // println!("Value: {}", *value);

    // 再度ロックを獲得して、値を読み取り
    let value = lock.lock();
    println!("Value: {}", *value);

    // ロックを解放
    unsafe {
        lock.unlock();
    }
}
