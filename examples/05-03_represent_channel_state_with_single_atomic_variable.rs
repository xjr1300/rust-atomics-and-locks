use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU8, Ordering};

const EMPTY: u8 = 0;
const WRITING: u8 = 1;
const READY: u8 = 2;
const READING: u8 = 3;

pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    state: AtomicU8,
}

unsafe impl<T: Send> Sync for Channel<T> {}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Self {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            state: AtomicU8::new(EMPTY),
        }
    }
}

impl<T> Channel<T> {
    //pub const fn new() -> Self {
    //    Self {
    //        message: UnsafeCell::new(MaybeUninit::uninit()),
    //        state: AtomicU8::new(EMPTY),
    //    }
    //}

    pub fn send(&self, message: T) {
        if self
            .state
            .compare_exchange(EMPTY, WRITING, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            panic!("can't send more than cone message!");
        }
        unsafe {
            (*self.message.get()).write(message);
        }
        // 次のReleaseストアと、`receive()`メソッドのAcquireロードがbefore-after関係を形成
        self.state.store(READY, Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        self.state.load(Ordering::Relaxed) == READY
    }

    pub fn receive(&self) -> T {
        if self
            .state
            .compare_exchange(READY, READING, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            panic!("no message available!");
        }
        unsafe { (*self.message.get()).assume_init_read() }
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if *self.state.get_mut() == READY {
            unsafe { self.message.get_mut().assume_init_drop() }
        }
    }
}

fn main() {
    let channel = Channel::default();
    let t = std::thread::current();
    std::thread::scope(|s| {
        s.spawn(|| {
            channel.send("hello world!");
            t.unpark();
        });
        while !channel.is_ready() {
            std::thread::park();
        }
        assert_eq!(channel.receive(), "hello world!");
    });
}
