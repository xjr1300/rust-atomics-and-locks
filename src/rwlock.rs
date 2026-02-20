use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

use atomic_wait::{wait, wake_all, wake_one};

pub struct RwLock<T> {
    /// リーダーの数
    ///
    /// ライトロックされている場合はu32::MAXになる。
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
            if s < u32::MAX {
                assert!(s != u32::MAX - 1, "too many readers");
                match self.state.compare_exchange_weak(
                    s,
                    s + 1,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return ReadGuard { rwlock: self },
                    Err(e) => s = e,
                }
            } else {
                // self.stateがu32::MAXの場合、つまりライトロックされている場合は、
                // ライトロックが解除されるのを待つ。
                wait(&self.state, u32::MAX);

                // 解除されたらリーダの数を再読み込みして、ループの先頭でリーダロックを試みる。
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }

    pub fn write(&self) -> WriteGuard<'_, T> {
        while self
            .state
            .compare_exchange(0, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // ReaderGuardのdropと先行発生関係が発生しており、writer_wake_counterがインクリ
            // メントされていることを観測した場合、stateはデクリメントされている。
            // もし、ReaderGuardのdropで、writer_wake_counterが増えていた場合、stateが
            // 0になっていることを観測できる場合がある。このとき、ライターロックを待機せずに取得できる。
            let w = self.writer_wake_counter.load(Ordering::Acquire);
            if self.state.load(Ordering::Relaxed) != 0 {
                // RwLockがロックされている場合は待機する。
                // ただし、待機しているライターの数が、上で取得したライターの数と一致するときのみ待機する。
                wait(&self.writer_wake_counter, w);
            }
        }
        WriteGuard { rwlock: self }
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
        if self.rwlock.state.fetch_sub(1, Ordering::Release) == 1 {
            // 最後のリーダー（ここではstateが1から0になったとき）がロックを解除したとき、
            // 待機しているライターを起こす。
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
