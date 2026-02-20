use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

use atomic_wait::{wait, wake_all, wake_one};

pub struct RwLock<T> {
    /// リーダーが待機している場合のリーダの数の2倍の値と、ライターが待機している
    /// 場合は1を加えた値を保持する。
    /// また、ライトロックされている場合はu32::MAXになる。
    ///
    /// つまり、リーダーが存在する場合は偶数、ライターが存在する場合は奇数になる。
    /// なお、u32::MAXは奇数である。
    state: AtomicU32,

    /// ライターを起こす際にインクリメント
    writer_wake_counter: AtomicU32,

    /// データ
    value: UnsafeCell<T>,
}

/// `T`が複数のスレッドから安全できるように`Sync`を要求している。
unsafe impl<T> Sync for RwLock<T> where T: Send + Sync {}

impl<T> RwLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            writer_wake_counter: AtomicU32::new(0),
            value: UnsafeCell::new(value),
        }
    }

    pub fn read(&self) -> ReadGuard<'_, T> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            if s.is_multiple_of(2) {
                // リーダーしか待機していない
                assert!(s != u32::MAX - 2, "too many readers");
                match self.state.compare_exchange_weak(
                    s,
                    s + 2,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return ReadGuard { rwlock: self },
                    Err(e) => s = e,
                }
            } else {
                // ライターが待機している
                wait(&self.state, u32::MAX);
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }

    pub fn write(&self) -> WriteGuard<'_, T> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            // ライターのみ待機している場合は、アンロックされていたらロックを試みる。
            if s <= 1 {
                match self
                    .state
                    .compare_exchange(s, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
                {
                    Ok(_) => return WriteGuard { rwlock: self },
                    Err(e) => {
                        // ループの先頭でロックの状態を確認するために、比較交換で失敗したときにえられたstateの値
                        // をsに保存する。
                        s = e;
                        continue;
                    }
                }
            }

            // ロックに失敗した場合、ライタースタベーションを回避するため、新しいリーダーをreadメソッド
            // で待機させるために、stateを奇数にする。
            if s.is_multiple_of(2) {
                match self
                    .state
                    .compare_exchange(s, s + 1, Ordering::Relaxed, Ordering::Relaxed)
                {
                    Ok(_) => {}
                    Err(e) => {
                        s = e;
                        continue;
                    }
                }
            }

            // まだロックされていたら待機する。
            let w = self.writer_wake_counter.load(Ordering::Acquire);
            s = self.state.load(Ordering::Relaxed);
            if s >= 2 {
                // リーダーまたはライターが待機している場合は待機する。
                wait(&self.writer_wake_counter, w);
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }
}

pub struct ReadGuard<'a, T> {
    rwlock: &'a RwLock<T>,
}

impl<T> std::ops::Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.value.get() }
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        // stateを2減らすことで、リーダーの待機数が1つ減る。
        if self.rwlock.state.fetch_sub(2, Ordering::Release) == 3 {
            // state3から１になった場合、RwLockがアンロックされ、かつ待機中のライターが存在する。
            // このライターを起こす。
            self.rwlock
                .writer_wake_counter
                .fetch_add(1, Ordering::Release);
            wake_one(&self.rwlock.writer_wake_counter);
        }
    }
}

pub struct WriteGuard<'a, T> {
    rwlock: &'a RwLock<T>,
}

impl<T> std::ops::Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.value.get() }
    }
}

impl<T> std::ops::DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.rwlock.value.get() }
    }
}

impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.rwlock.state.store(0, Ordering::Release);
        self.rwlock
            .writer_wake_counter
            .fetch_add(1, Ordering::Release);
        wake_one(&self.rwlock.writer_wake_counter);
        wake_all(&self.rwlock.state);
    }
}
