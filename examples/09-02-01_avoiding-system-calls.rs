use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use atomic_wait::{wait, wake_all, wake_one};

use rust_atomics_and_locks::mutex::MutexGuard;

#[derive(Default)]
pub struct Condvar {
    counter: AtomicU32,
    num_waiters: AtomicUsize,
}

impl Condvar {
    pub fn notify_one(&self) {
        if self.num_waiters.load(Ordering::Relaxed) > 0 {
            self.counter.fetch_add(1, Ordering::Relaxed);
            wake_one(&self.counter);
        }
    }

    pub fn notify_all(&self) {
        if self.num_waiters.load(Ordering::Relaxed) > 0 {
            self.counter.fetch_add(1, Ordering::Relaxed);
            wake_all(&self.counter);
        }
    }

    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        self.num_waiters.fetch_add(1, Ordering::Relaxed);

        let counter_value = self.counter.load(Ordering::Relaxed);

        let mutex = guard.mutex;
        drop(guard);

        wait(&self.counter, counter_value);

        self.num_waiters.fetch_sub(1, Ordering::Relaxed);

        mutex.lock()
    }
}

#[cfg(test)]
mod tests {
    use rust_atomics_and_locks::mutex::Mutex;
    #[test]
    fn test_condvar() {
        let mutex = Mutex::new(0);
        let condvar = Condvar::default();

        let mut wakeups = 0;

        std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_secs(1));
                *mutex.lock() = 123;
                condvar.notify_one();
            });

            let mut m = mutex.lock();
            while *m < 100 {
                m = condvar.wait(m);
                wakeups += 1;
            }

            assert_eq!(*m, 123);
        });

        // メインスレッドがウェイトしたことを示す。
        // ただし、スプリアスウェイクアップの発生を許容するため、本来起こされる回数
        // は1回であるが、10回未満であることをアサートする。
        assert!(wakeups < 10);
    }
}

fn main() {}
