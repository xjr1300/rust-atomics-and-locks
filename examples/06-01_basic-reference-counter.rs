//! `NonNull<T>`は決してnullにならないことを型レベルで保証されたポインタである。
//! サイズ及びABI(Application Binary Interface)は、`*mut T`と同じであるが、非ヌルであるという制約をコンパイラに伝えている点が異なる。
//!
//! したがって、`Arc<T>`が保持する`ptr`は常に非ヌルで、ヌルというビットパターンは使用されない。
//!
//! > ABI(Application Binary Interface)とは、コンパイラやプログラミング言語の違いを超えて、バイナリレベルで関数やデータ構造がどのように
//! > 表現され、相互作用するかを定義する規約や仕様を示す。
//! > ABIは、アプリケーションの移植性や互換性にも重要な役割を果たす。
//! > 特に、新しいバージョンのOSやハードウェアが登場しても、ABIが維持されていれば、既存のアプリケーションは新しい環境で修正することなく動作できる。
//! > これにより、開発者は異なる環境でもアプリケーションを開発でき、ユーザーも新しい環境に移行する際の負担が軽減される。
//!
//! 次に、`Option<T>`は概念的に次のように表現される。
//!
//! ```rust
//! enum Option<T> {
//!     None,
//!     Some(T),
//! }
//! ```
//!
//! 一般には、`None`と`Some`を区別するための判別子（タグ）と`T`の値が必要になるため、`Option<T>`のサイズは`T`のサイズより大きくなる。
//!
//! しかし、Rustには**ヌルポインタ最適化（nullable pointer optimization)**があり、**ある型`T`にコンパイラが「決して現れない」と
//! 保証できる値（無効値）が存在する場合、`Option<T>`はその無効値を`None`の表現として再利用できる**。
//!
//! `Arc<T>`は本質的に非ヌルなポインタ1つを保持する型である。
//! したがって、`Option<Arc<T>>`は次のように表現できる。
//!
//! - `None`: `ptr == null`
//! - `Some(arc)`: `ptr == arc.ptr`（`arc.ptr`は必ず非ヌル）
//!
//! このため、`None`用に新しいタグを持つ必要がなく、`Arc<T>`では使用されない`null`というビットパターンを`None`の表現として割り当てることができる。
//!
//! この結果、`size_of::<Option<Arc<T>>>() == size_of::<Arc<T>>()`が成立するため、`Option<Arc<T>>`は`Arc<T>`と同じサイズになる。
//!
//! `NonNull<T>`を使用する理由の1つに、`Option<Arc<T>>`をゼロコストで表現できるようにすることが挙げられる。
//!
//! ちなみに`Option<*mut T>`とした場合、`Some(null)`が存在しうるため、`None`と区別するためのタグが必要になり、ヌルポインタ最適化がなされず、
//! `size_of::<Option<*mut T>>() == size_of::<*mut T>() + size_of::<usize>()`となる。
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering, fence};

struct ArcData<T> {
    ref_count: AtomicUsize,
    data: T,
}

pub struct Arc<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Arc<T> {}
unsafe impl<T: Send + Sync> Sync for Arc<T> {}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        Arc {
            ptr: NonNull::from(Box::leak(Box::new(ArcData {
                ref_count: AtomicUsize::new(1),
                data,
            }))),
        }
    }

    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> std::ops::Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data().data
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        if self.data().ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Arc { ptr: self.ptr }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        if self.data().ref_count.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            unsafe {
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
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

        // 文字列と`DetectDrop`をタプルにまとめて`Arc`で包む。
        let x = Arc::new(("hello", DetectDrop));
        let y = Arc::clone(&x);

        // `x`を別スレッドにムーブして消費する。
        let t = std::thread::spawn(move || {
            assert_eq!(x.0, "hello");
        });

        // `y`は利用可能なはず。
        assert_eq!(y.0, "hello");

        // 起動したスレッドが終了するまで待機する。
        t.join().unwrap();

        // `x`はドロップされているはず。
        // しかし、`y`はまだ生きているので、ドロップされていないはず。
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);

        // `y`をドロップする。
        drop(y);

        // すべての`Arc`インスタンスがドロップされたので、`DetectDrop`もドロップされているはず。
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 1);
    }
}
