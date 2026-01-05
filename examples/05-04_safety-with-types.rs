use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
};

struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
}

pub struct Sender<T> {
    channel: Arc<Channel<T>>,
}

pub struct Receiver<T> {
    channel: Arc<Channel<T>>,
}

unsafe impl<T: Send> Sync for Channel<T> {}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let channel = Arc::new(Channel {
        message: UnsafeCell::new(MaybeUninit::uninit()),
        ready: AtomicBool::new(false),
    });
    (
        Sender {
            channel: channel.clone(),
        },
        Receiver { channel },
    )
}

impl<T> Sender<T> {
    /// # Safety
    ///
    /// このメソッドはパニックしない。
    /// また、`send()`メソッドを呼び出すと、メソッド内にインスタンスがムーブするため、
    /// 1回だけ呼び出し可能であることを型システムによって保証する。
    pub fn send(self, message: T) {
        unsafe {
            (*self.channel.message.get()).write(message);
        }
        self.channel.ready.store(true, Ordering::Release);
    }
}

impl<T> Receiver<T> {
    pub fn is_ready(&self) -> bool {
        self.channel.ready.load(Ordering::Relaxed)
    }

    pub fn receive(self) -> T {
        if !self.channel.ready.swap(false, Ordering::Acquire) {
            panic!("no message available!");
        }
        unsafe { (*self.channel.message.get()).assume_init_read() }
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        // `ready`が`true`の場合、読み込まれていないメッセージがチャネルに存在するため
        // ドロップする必要がある。
        if *self.ready.get_mut() {
            unsafe {
                self.message.get_mut().assume_init_drop();
            }
        }
    }
}

fn main() {
    std::thread::scope(|s| {
        let (sender, receiver) = channel();
        let t = std::thread::current();
        s.spawn(move || {
            sender.send("hello world!");
            // 次は`sender`がむーぶしているため、コンパイルエラーになる。
            // sender.send("second message");
            t.unpark();
        });
        while !receiver.is_ready() {
            std::thread::park();
        }
        assert_eq!(receiver.receive(), "hello world!");
    })
}
