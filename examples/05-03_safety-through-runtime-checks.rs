use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    in_use: AtomicBool,
    ready: AtomicBool,
}

unsafe impl<T> Sync for Channel<T> where T: Send {}

impl<T> Channel<T> {
    pub const fn default() -> Self {
        Self {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            in_use: AtomicBool::new(false),
            ready: AtomicBool::new(false),
        }
    }

    /// # Safety
    ///
    /// 1回だけよびだすこと！
    /// 同時に複数スレッドから呼び出してはダメ！
    pub fn send(&self, message: T) {
        if self.in_use.swap(true, Ordering::Relaxed) {
            panic!("can't send more than one message!");
        }
        unsafe {
            (*self.message.get()).write(message);
        }
        // `message`への書き込みを公開するReleaseストア
        self.ready.store(true, Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        // `Relaxed`は、他のメモリアクセスとのhappens-before関係を形成しない。
        // このメソッドは、メッセージが準備できている「可能性」を確認するための
        // ヒントとしてのみ使用される。
        //
        // したがって、**同じスレッド**で`is_ready()`メソッドが`true`を返した後、`receive()`
        // メソッド内の`ready.load()`呼び出しが`true`になることは保証されていない。
        //
        // 実際に`message`が初期化済みであることの保証は、`receive()`内のAcquire
        // オーダリングの`swap()`操作によってのみ与えられる。
        self.ready.load(Ordering::Relaxed)
    }

    pub fn receive(&self) -> T {
        // `Atomic*::swap`メソッドは、アトミック変数の値を新しい値に置き換え、
        // 置き換え前の古い値を返す。
        // したがって、`ready`が`false`のときに、つまり`message`に値が与えられて
        // いないときに、`receive()`メソッドを呼び出すとパニックする。
        //
        // このAcquireロードが、`send()`メソッドのReleaseストアと同期して、
        // `message`への書き込みが観測可能になる。
        if !self.ready.swap(false, Ordering::Acquire) {
            panic!("no message available!");
        }
        // `ready == true`をAcquireロードで観測しているため、`message`は
        // 初期化されていることが保証される。
        unsafe { (*self.message.get()).assume_init_read() }
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if *self.ready.get_mut() {
            unsafe {
                self.message.get_mut().assume_init_drop();
            }
        }
    }
}

fn main() {
    let channel = Arc::new(Channel::default());
    let t = std::thread::current();
    std::thread::scope(|s| {
        s.spawn(|| {
            channel.send("hello world!");
            // `unpark()`は、対応するスレッドを起床させるための通知を送信する。
            // 正確には、対象スレッドに許可トークンを1つ与える操作である。
            //
            // `park()`は、許可トークンが存在する場合は停止せずに即座に復帰し、存在しない場合のみ
            // スレッドを停止する。
            //
            // 下の`while`ループでは、mainスレッドが`park()`によって停止している可能性がある。
            // そのため、メッセージ送信後に`unpark()`を呼び出し、mainスレッドが待機状態であれば
            // 起床させる。
            t.unpark();
        });
        while !channel.is_ready() {
            // メッセージが準備できていない可能性があるため、mainスレッドを一時的に停止する。
            // ただし、すでに許可トークンが与えられている場合、`park()`は停止せずに即座に復帰する。
            std::thread::park();
        }
        assert_eq!(channel.receive(), "hello world!");
    });
}
