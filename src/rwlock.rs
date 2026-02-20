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

    /// データ
    value: UnsafeCell<T>,
}

/// `T`が複数のスレッドから安全できるように`Sync`を要求している。
unsafe impl<T> Sync for RwLock<T> where T: Send + Sync {}

impl<T> RwLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
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
        while let Err(s) =
            self.state
                .compare_exchange(0, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
        {
            // self.stateが0でない場合、つまりリーダーがいるかライトロックされている場合は、
            // ロックが解除されるのを待つ。
            wait(&self.state, s);
        }
        // ループを抜けたら、self.stateが0、つまりロックされていない状態からu32::MAXに交換
        // できたため、ライターロックを返す。
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
            // 最後のリーダーがロックを解除した場合、つまりself.stateが1から0になった場合は、
            // 待機中のリーダーが存在しないことを示すため、ライターがロックできる。
            // したがって、待機中のライターが存在する場合、ライトロックが解除されたことを通知する。
            wake_one(&self.rwlock.state);
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
        // 待機しているリーダーとライターをすべて起こす。
        wake_all(&self.rwlock.state);
    }
}
