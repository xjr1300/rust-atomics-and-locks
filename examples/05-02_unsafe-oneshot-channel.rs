//! # アンセーフなワンショットチャネル
//!
//! ## MaybeUninit<T>
//!
//! `T`型の初期化されていないインスタンスを構築するためのラッパー型である。
//!
//! `MaybeUninit<T>`は、初期化されていないデータを扱うアンセーフなコードを書くことを可能にする。
//! これは、「この値が初期化されていない可能性がある」ことを、コンパイラーに明示するための方である。
//!
//! 一般的に、コンパイラーは、すべての変数がその型の不変条件を満たすように適切に初期化されていると仮定する。
//! たとえば、参照型は常に整列（参照が指すアドレスが、その型に要求されるアラインメント境界に揃っていること）
//! され、かつ非nullでなければならない。
//! これらの不変条件は、アンセーフなコードであっても常に維持されなければならない。
//!
//! このため、参照型の変数をゼロで初期化すると、その参照が実際に使用されるかどうかに関係なく、その
//! 時点で未定義動作が発生する。
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// `Channel<T>`は内部に`UnsafeCell<T>`を含むため、自動的に`Send`と`Sync`にならない。
/// しかし、本実装においては、以下の不変条件を呼び出し側が守る限りにおいて、スレッド間の移動は安全である
/// と判断して、`Send`と`Sync`を手動で実装している。
///
/// - `send`メソッドは1回だけ呼び出すこと。
/// - `is_ready`メソッドが`true`を返した後、`receive`メソッドは1回だけ呼び出すこと。
pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
}

unsafe impl<T> Send for Channel<T> where T: Send {}
unsafe impl<T> Sync for Channel<T> where T: Sync {}

impl<T> Channel<T> {
    // new -> default
    pub const fn default() -> Self {
        Self {
            // 初期化されていない状態で`MaybeUninit<T>`を構築
            message: UnsafeCell::new(MaybeUninit::uninit()),
            ready: AtomicBool::new(false),
        }
    }

    // 安全性: 他のスレッドが`receive`メソッドを呼び出し中に、メッセージを書き換える可能性がある。
    // この場合、競合状態が発生し未定義動作となる。したがって、`send`メソッドは1回だけ呼び出すこと。
    // #[allow(unsafe_op_in_unsafe_fn)]
    // pub unsafe fn send(&self, message: T) {
    //     (*self.message.get()).write(message);
    //     self.ready.store(true, Ordering::Release);
    // }
    pub fn send(&self, message: T) {
        unsafe {
            // `UnsafeCell<T>`を介して`MaybeUninit<T>`にアクセスし、値を書き込む
            (*self.message.get()).write(message);
        }
        // Releaseストアとすることで、`ready`が`true`であることを観測したスレッドは、
        // この書き込み以前のすべての操作が観測可能となる。
        self.ready.store(true, Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        // Acquireロードとすることで、`send`メソッド内のReleaseストアと、Release-Acquire関係
        // を形成し、`ready`が`true`であることを観測した場合に、`message`への書き込みも
        // 観測できることを保証する。
        self.ready.load(Ordering::Acquire)
    }

    // 安全性: `receive`メソッドは`is_ready`メソッドが`true`を返した後、1回だけ呼び出すこと。
    // `T`が`Copy`トレイトを実装していない場合、2回以上呼び出すと未定義動作となる。
    // #[allow(unsafe_op_in_unsafe_fn)]
    // pub unsafe fn receive(&self) -> T {
    //     (*self.message.get()).assume_init_read()
    // }
    pub fn receive(&self) -> T {
        // `UnsafeCell`を介して`MaybeUninit<T>`にアクセスし、メッセージを読み取り
        // `assume_init_read`は、`MaybeUninit<T>`が初期化されていると仮定して値を読み取る
        // ため、`MaybeUninit<T>`が初期化されていることを保証するのは、呼び出し側の責任である。
        unsafe { (*self.message.get()).assume_init_read() }
    }
}

/// `MaybeUninit<T>`は自動で`T`をドロップしない。
/// したがって、`Channel<T>`がドロップしたとき、値が初期化済みの場合のみ、値を明示的にドロップする。
impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if self.ready.load(Ordering::Acquire) {
            unsafe {
                self.message.get_mut().assume_init_drop();
            }
        }
    }
}

fn main() {
    let channel = Arc::new(Channel::default());
    let cloned_channel = Arc::clone(&channel);

    let sender = std::thread::spawn(move || {
        channel.send(42);
        println!("Sent message: 42");
    });

    let receiver = std::thread::spawn(move || {
        while !cloned_channel.is_ready() {
            std::hint::spin_loop();
        }
        let message = cloned_channel.receive();
        println!("Received message: {}", message);
    });

    sender.join().unwrap();
    receiver.join().unwrap();
}
