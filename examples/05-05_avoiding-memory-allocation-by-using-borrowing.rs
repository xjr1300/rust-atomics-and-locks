use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
}

unsafe impl<T: Send> Sync for Channel<T> {}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Channel {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            ready: AtomicBool::new(false),
        }
    }
}

impl<T> Channel<T> {
    //pub fn split<'a>(&'a mut self) -> (Sender<'a, T>, Receiver<'a, T>) {
    pub fn split(&'_ mut self) -> (Sender<'_, T>, Receiver<'_, T>) {
        // 自身の可変参照を受け取り、自身の可変参照を介して新しいインスタンスで初期化することで、
        // 上書き前の`*self`がドロップされるようにする。
        *self = Self::default();
        (Sender { channel: self }, Receiver { channel: self })
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

pub struct Sender<'a, T> {
    channel: &'a Channel<T>,
}

pub struct Receiver<'a, T> {
    channel: &'a Channel<T>,
}

impl<T> Sender<'_, T> {
    pub fn send(self, message: T) {
        unsafe {
            (*self.channel.message.get()).write(message);
        }
        self.channel.ready.store(true, Ordering::Release);
    }
}

impl<T> Receiver<'_, T> {
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

fn main() {
    let mut channel = Channel::default();
    std::thread::scope(|s| {
        let (sender, receiver) = channel.split();
        let t = std::thread::current();
        s.spawn(move || {
            sender.send("hello world!");
            t.unpark();
        });
        while !receiver.is_ready() {
            std::thread::park();
        }
        assert_eq!(receiver.receive(), "hello world!");
    })
}
