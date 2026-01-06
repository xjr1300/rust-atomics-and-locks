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
        Self {
            ptr: NonNull::from(Box::leak(Box::new(ArcData {
                ref_count: AtomicUsize::new(1),
                data,
            }))),
        }
    }

    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }

    /// 参照カウンタが1のときだけ、内部の`T`への可変参照（`&mut T`）を返す。
    /// 参照カウンタが1より大きい場合は`None`を返す。
    ///
    /// `get_mut`メソッドは、`self`を引数として取らず、`arc`として自身を受け取り、メソッド呼び出し構文を使用できない。
    /// したがって、`a.get_mut()`のように呼び出すことができず、`Arc::get_mut(&mut a)`のように呼び出す必要がある。
    ///
    /// Rustは`Deref`を実装した`Arc<T>`のような構造体のインスタンス`a`に対して`a.method()`を呼び出した場合、
    /// 次の探索が行われる。
    ///
    /// 1. `Arc<T>`型に`method`という名前のメソッドが存在するか確認する。
    /// 2. 上記が存在しない場合、`Deref`で得られる型（この場合は`T`）に`method`という名前のメソッドが存在するか確認する。
    /// 3. 上記が存在しない場合、`T`型の`Deref`で得られる型に`method`という名前のメソッドが存在するか確認する。
    /// 4. 以降、同様に`Deref`で得られる型に対して探索を続ける。
    ///
    /// したがって、`a.get_mut()`とした場合、次が候補に上がる。
    ///
    /// - `Arc<T>`型の`get_mut`メソッド
    /// - `T`型の`get_mut`メソッド（`Deref`で得られる型）
    /// - `&T`/`&mut T`型の`get_mut`メソッド
    /// - `T`型の`Deref`で得られる型の`get_mut`メソッド
    ///
    /// もし、`T`が`get_mut`という名前のメソッドを持っていた場合、`a.get_mut()`は、`Arc<T>`型の`get_mut`メソッドを
    /// 呼び出したいのか、`T`型の`get_mut`メソッドを呼び出したいのか曖昧になってしまう。
    /// したがって、関連関数として実装して、`Arc::get_mut(&mut a)`のように呼び出す形にして、`Deref`実装をメソッド探索に
    /// 巻き込まないようにしている。
    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        if arc.data().ref_count.load(Ordering::Relaxed) == 1 {
            // `ref_count == 1`が観測された場合、他のスレッドで行われた`Drop`におけるRelease操作と同期する必要がある。
            // つまり、他スレッドが`ref_count`を1にして、ドロップするまでに行った書き込みと同期する。
            // このAcquireフェンスにより、他スレッドが`Arc<T>`を解放するまでに行ったすべての書き込みを
            // `fence`以降の操作から確実に観測できるようにする。
            fence(Ordering::Acquire);
            unsafe { Some(&mut arc.ptr.as_mut().data) }
        } else {
            None
        }
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

fn main() {
    let mut a = Arc::new("hello".to_string());
    {
        let b = Arc::clone(&a);
        println!("b: {}", b.data().data)
    }
    if let Some(s) = Arc::get_mut(&mut a) {
        s.push_str(", world");
    }
    println!("a: {}", a.data().data);
}
