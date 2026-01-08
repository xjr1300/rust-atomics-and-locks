use std::cell::UnsafeCell;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering, fence};

pub struct Arc<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Arc<T> {}
unsafe impl<T: Send + Sync> Sync for Arc<T> {}

pub struct Weak<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Weak<T> {}
unsafe impl<T: Send + Sync> Sync for Weak<T> {}

/// データの生存と、メモリ領域の生存を分離して管理する制御ブロック
struct ArcData<T> {
    /// 強参照（`Arc<T>`）の数
    ///
    /// 0になった時点で`T`をドロップする。
    data_ref_count: AtomicUsize,

    /// 弱参照（`Weak<T>`）の数と、強参照が1つ以上存在することを表現する暗黙の弱参照を合算した参照カウント
    ///
    /// 0になった時点で強参照も弱参照も存在しないため、`ArcData<T>`のメモリを解放する。
    alloc_ref_count: AtomicUsize,

    /// 実データ
    ///
    /// `Arc<T>`の数が0になった時点でドロップされる。
    data: UnsafeCell<ManuallyDrop<T>>,
}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        // 強参照が1つ存在することになるため、`data_ref_count`を1で初期化する。
        // 強参照が存在することを示す暗黙的な弱参照も存在するため、`alloc_ref_count`も1で初期化する。
        // この時点で弱参照は存在しないが、`alloc_ref_count`は強参照と弱参照の合計数を表すため、1で初期化している。
        Self {
            ptr: NonNull::from(Box::leak(Box::new(ArcData {
                data_ref_count: AtomicUsize::new(1),
                alloc_ref_count: AtomicUsize::new(1),
                data: UnsafeCell::new(ManuallyDrop::new(data)),
            }))),
        }
    }

    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }

    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        // ラップしているデータの可変参照を取得するためには、強参照が1つのみ存在して、弱参照が存在しないことを
        // 確認する必要がある。
        // そこで、`alloc_ref_count == 1`（暗黙の弱参照のみ存在）を確認した上で、`alloc_ref_count`を
        // `usize::MAX`に設定し、他のスレッドが`Arc::downgrade`を使用して、弱参照を作成できないようにする。
        // `Arc::downgrade`では、`alloc_ref_count`が`usize::MAX`の場合、スピンするように
        // 実装されている。
        // したがって、次が実行されたとき、他のスレッドで実行中の`Arc::downgrade`はスピンロックで停止する。
        // `compare_exchange`で成功時に`Ordering::Acquire`を使用することで、`alloc_ref_count`が
        // 1である（弱参照が存在しない）ことを、`Weak::drop`のReleaseストアと同期することで、確実に観測できる。
        if arc
            .data()
            .alloc_ref_count
            .compare_exchange(1, usize::MAX, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // `alloc_ref_count`が1より大きいため弱参照が存在する。
            // したがって、ラップしているデータの可変参照を返せない。
            return None;
        }

        // 強参照が1つのみであることを確認する。
        // この時点で`alloc_ref_count == 1`が成立しているため、既存の弱参照は存在しない。
        // また、`alloc_ref_count`を`usize::MAX`に設定している間は、他のスレッドによる`Arc::downgrade`で
        // 新たな弱参照が作成されることを、一時的に防止している。
        let is_unique = arc.data().data_ref_count.load(Ordering::Relaxed) == 1;

        // `alloc_ref_count`を1に戻し、もし`Arc::downgrade`がスピンしていた場合に再開できるようにする。
        // Releaseストアを使用することで、`Arc::downgrade`の`compare_exchange_weak`のAcquireロードと
        // 同期し、`alloc_ref_count`が1であることを`Arc::downgrade`に保証する。
        // これ以降、他のスレッドで弱参照が作成されても、すでに取得済みの`data_ref_count`の値（`is_unique`）
        // には影響しない。
        arc.data().alloc_ref_count.store(1, Ordering::Release);

        // 強参照が複数あれば失敗させる。
        if !is_unique {
            return None;
        }

        // `fence(Ordering::Acquire)`は、このフェンスより前に他スレッドで行われたRelease操作
        // （特に`Arc::drop`におけるReleaseデクリメント）と同期し、それらに先行したデータアクセスが、
        // このフェンス以降に持ち越されないことを保証する。
        // これにより、「過去に」他スレッドが`Arc<T>`を通じてデータにアクセスしていた可能性を排除できる。
        fence(Ordering::Acquire);
        unsafe { Some(&mut *arc.data().data.get()) }
    }

    pub fn downgrade(arc: &Self) -> Weak<T> {
        let mut n = arc.data().alloc_ref_count.load(Ordering::Relaxed);
        loop {
            if n == usize::MAX {
                std::hint::spin_loop();
                n = arc.data().alloc_ref_count.load(Ordering::Relaxed);
                continue;
            }
            assert!(n < usize::MAX - 1);
            if let Err(e) = arc.data().alloc_ref_count.compare_exchange_weak(
                n,
                n + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                n = e;
                continue;
            }
            return Weak { ptr: arc.ptr };
        }
    }
}

impl<T> std::ops::Deref for Arc<T> {
    type Target = T;

    /// # Safety
    ///
    /// このデータに対する`Arc<T>`が存在するため、データは存在する。
    /// ただし、共有されている可能性がある。
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data().data.get() }
    }
}

impl<T> Weak<T> {
    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }

    pub fn upgrade(&self) -> Option<Arc<T>> {
        // 強参照が存在することを保証できれば良いため、Relaxedで十分である。
        // Acquireが必要になるのは、他のスレッドのReleaseより後に行われた書き込みを観測したいときである。
        let mut n = self.data().data_ref_count.load(Ordering::Relaxed);
        loop {
            if n == 0 {
                return None;
            }
            assert!(n < usize::MAX);
            if let Err(e) = self.data().data_ref_count.compare_exchange_weak(
                n,
                n + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                n = e;
                continue;
            }
            return Some(Arc { ptr: self.ptr });
        }
    }
}

impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        if self.data().alloc_ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Self { ptr: self.ptr }
    }
}

impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        if self.data().alloc_ref_count.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            unsafe {
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
        }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        if self.data().data_ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Self { ptr: self.ptr }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        if self.data().data_ref_count.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            // 安全性: データへの参照カウントは0であるため、誰もデータにアクセスできない
            unsafe {
                ManuallyDrop::drop(&mut *self.data().data.get());
            }
            // `Arc<T>`が残っていないため、すべての`Arc<T>`を代表していた暗黙のWeakポインタをドロップする
            drop(Weak { ptr: self.ptr });
        }
    }
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        static NUM_DROPS: AtomicUsize = AtomicUsize::new(0);

        struct DetectDrop;

        impl Drop for DetectDrop {
            fn drop(&mut self) {
                NUM_DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        let x = Arc::new(("hello", DetectDrop));
        let y = Arc::downgrade(&x);
        let z = Arc::downgrade(&x);

        let t = std::thread::spawn(move || {
            // この時点で、Weakポインタはアップグレード可能
            let y = y.upgrade().unwrap();
            assert_eq!(y.0, "hello");
        });
        assert_eq!(x.0, "hello");
        t.join().unwrap();

        // データはドロップされていないため、Weakポインタはアップグレード可能
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);
        assert!(z.upgrade().is_some());

        // Arcをドロップ
        drop(x);

        // Arcはすべてドロップされているため、Weakポインタはアップグレード不可能
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 1);
        assert!(z.upgrade().is_none());
    }
}
