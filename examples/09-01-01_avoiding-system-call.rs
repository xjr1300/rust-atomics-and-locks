use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};

use atomic_wait::{wait, wake_one};

pub struct Mutex<T> {
    /// 0: ロックされていない状態
    /// 1: ロックされており、待機中のスレッドがない状態
    /// 2: ロックされており、待機中のスレッドがある状態
    state: AtomicU32,
    value: UnsafeCell<T>,
}

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

    /// 待機はstateが2の場合のみする。
    pub fn lock(&self) -> MutexGuard<T> {
        // stateが0（ロックされていない）の場合のみ1（ロックされている、待機中のスレッドがない状態）
        // に変更する。
        // 成功した場合、ロックを獲得できるため、MutexGuardを返す。
        //
        // ここのAcquire操作は、unlockメソッドのRelease操作と、先行発生関係を形成しており、
        // unlockメソッドがstateを設定した後のメモリ操作を、このスレッドが見ることを保証する。
        // つまり、stageが0、1、2の場合がある。
        if self
            .state
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // compare_exchangeに失敗した場合、既にロックされている（stateは1または2）。
            // 以降は待機処理に入る。

            // 次のswap操作で0が返されるまでループする。
            // 0が返されるのは、別スレッドがunlockメソッドでstateを0に変更した直後に、このスレッドで
            // swapが実行された場合で、この場合はこのスレッドがロックを取得する。
            //
            // stateが1の場合は、このスレッドは上記compare_exchangeでstateを0から1に変更できな
            // かったため、ロックは他のスレッドが取得していることを意味する。
            // したがって、stateを2に設定し、待機中のスレッドが存在することを表明する。
            while self.state.swap(2, Ordering::Acquire) != 0 {
                // swapはstateを2（ロックされている、待機中スレッドがある状態）に設定し、
                // 以前の値を返す。
                //
                // swapの結果が0の場合、別スレッドがちょうどロックを解放した直後にswapしたことを意味する。
                // この場合、自分がロックを獲得したためループを抜ける。
                //
                // swapの戻り値が1の場合は、別スレッドがunlockによってロックを解放し、stateを0に変更した後、
                // さらに別のスレッドが、compare_exchangeでstateを0から1に変更し、ロックを取得した場合である。
                //
                // swapの戻り値が2の場合は、すでに他のスレッドが待機していることを示す。
                //
                // swapでstateを2に設定しても、次のwait呼び出しまでに、別スレッドによってstateが0または1に
                // 変更される可能性があることに注意すること。
                //
                // wait呼び出しでstateが0と評価されるのは、他のスレッドがunlockメソッドでstateを0に変更した場合である。
                //
                // wait呼び出しでstateが1と評価されるのは、別のスレッドがunlockメソッドでstateを0に変更した後、
                // さらに別のスレッドがcompare_exchangeでstateを0から1に変更した場合である。
                // この場合、waitで待機しないが、ループの先頭に戻り、swapで再度stateを2に設定し直す。
                //
                // wait呼び出しでstateが2と評価されるのは、既に待機スレッドが存在する場合である。
                // この場合、waitで待機する。
                //
                // waitによる待機から復帰するのは、別スレッドがunlockメソッドでstateを0に変更した後、wake_one
                // によって、偶然的にこのスレッドに再開を通知された場合である。
                wait(&self.state, 2);
            }
        }
        MutexGuard { mutex: self }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    /// stateが2の場合のみ、他のスレッドを起こす。
    fn drop(&mut self) {
        // stateを0（ロックされていない）にセット
        if self.mutex.state.swap(0, Ordering::Release) == 2 {
            wake_one(&self.mutex.state);
        }
    }
}
