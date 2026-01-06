use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering, fence};

/// `Aec<T>`と`Weak<T>`の共有制御ブロック
///
/// 本実装では寿命管理を2段階に分離しているｌ．
///
/// - `data_ref_count`
///   - 生存している`Arc<T>`の数
///   - 0になった時点で`T`をドロップ
/// - `alloc_ref_count`
///   - `Arc<T>`と`Weak<T>`をあわせた総数
///   - 0になった時点で`ArcData<T>`のメモリを解放
///
/// したがって、`T`は最後の`Arc<T>`がドロップされた時点でドロップされるが、`Weak<T>`が残っている場合、
/// この制御ブロックのメモリは解放されない。
/// この構造は、`std::sync::Arc`の内部設計と本質的に同じである。
struct ArcData<T> {
    /// `Arc`の参照カウンタ
    ///
    /// 0になったら`T`をドロップする。
    data_ref_count: AtomicUsize,

    /// `Arc`と`Weak`の参照カウンタ
    ///
    /// `ArcData<T>`のメモリを指している`Arc`と`Weak`の総数をカウントする。
    /// 0になったら`ArcData<T>`のメモリを解放する。
    alloc_ref_count: AtomicUsize,

    /// データ本体
    ///
    /// `data_ref_count`が0になったときに`None`に設定され、それ以降は`Weak::upgrade`できなくなる。
    data: UnsafeCell<Option<T>>,
}

pub struct Arc<T> {
    weak: Weak<T>,
}

pub struct Weak<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Weak<T> {}
unsafe impl<T: Send + Sync> Sync for Weak<T> {}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        Self {
            weak: Weak {
                ptr: NonNull::from(Box::leak(Box::new(ArcData {
                    data_ref_count: AtomicUsize::new(1),
                    alloc_ref_count: AtomicUsize::new(1),
                    data: UnsafeCell::new(Some(data)),
                }))),
            },
        }
    }

    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        if arc.weak.data().alloc_ref_count.load(Ordering::Relaxed) == 1 {
            fence(Ordering::Acquire);
            // 安全性: `alloc_ref_count == 1`は、`Arc`が1つしか存在しないことを意味する。
            // このとき、他のスレッドからこの`T`に到達する手段は存在しない。
            // さらに`&mut Arc<T>`を受け取っているため、このスレッドは`Arc`に対する排他アクセスを保持している。
            let arc_data = unsafe { arc.weak.ptr.as_mut() };
            let option = arc_data.data.get_mut();
            // Arcがあるためデータは`Some`であることが保証されている。
            let data = option.as_mut().unwrap();
            Some(data)
        } else {
            None
        }
    }

    pub fn downgrade(arc: &Self) -> Weak<T> {
        arc.weak.clone()
    }
}

impl<T> Weak<T> {
    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }

    pub fn upgrade(&self) -> Option<Arc<T>> {
        // `upgrade`は`data_ref_count`の数を確認できればよく、`T`の初期化や破棄の同期は、`Arc<T>::drop`
        // のRelease-Acquireによって保証される。
        // このため、ここではRelaxedを使用しても問題ない。
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
                // `data_ref_count`の更新に失敗した場合、`n`を新しい値で更新して再試行する。
                n = e;
                continue;
            }
            return Some(Arc { weak: self.clone() });
        }
    }
}

impl<T> std::ops::Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let ptr = self.weak.data().data.get();
        unsafe { (*ptr).as_ref().unwrap() }
    }
}

impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        if self.data().alloc_ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Weak { ptr: self.ptr }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        let weak = self.weak.clone();
        if weak.data().data_ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Self { weak }
    }
}

impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        if self.data().alloc_ref_count.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            unsafe {
                // ArcとWeakがすべてドロップされた場合、`ArcData<T>`のメモリを解放する。
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
        }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        if self
            .weak
            .data()
            .data_ref_count
            .fetch_sub(1, Ordering::Release)
            == 1
        {
            fence(Ordering::Acquire);
            let ptr = self.weak.data().data.get();
            unsafe {
                // `data_ref_count`が0になったため、この`Arc<T>`が`T`の最後の保有者である。
                // したがって、`T`をドロップする責任がある。
                // `T`のドロップは、`ArcData<T>`の`data`フィールドを内部可変性を利用して`None`に設定することで実現する。
                // ただし、`ArcData<T>`自体は`Weak<T>`が存在する可能性があるため解放しない。
                // `Weak<T>`は、`ptr`で`ArcData<T>`を確保したメモリ領域を指し示している。
                (*ptr) = None;
            }
        }
        // `Arc<T>`は内部に`Weak<T>`を1つ保持しているため、この`drop`の終了時に、その`Weak<T>`がドロップされる。
        // しかし、他に`Weak`が存在する場合、`ArcData<T>`のメモリは解放されない。
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
