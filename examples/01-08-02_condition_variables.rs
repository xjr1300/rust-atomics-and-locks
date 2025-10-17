use std::collections::VecDeque;
// use std::mem::drop;
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
    let queue = Mutex::new(VecDeque::new());
    let not_empty = Condvar::new();

    thread::scope(|s| {
        s.spawn(|| {
            loop {
                // キューをロック
                let mut q = queue.lock().unwrap();
                let item = loop {
                    if let Some(item) = q.pop_front() {
                        break item;
                    } else {
                        // キューのロックを解放して、スリープ
                        q = not_empty.wait(q).unwrap();
                    }
                };
                // ミューテックスガードを解放
                // 明示的なミューテックスガードのドロップは必要ない。
                // ループの末尾でミューテックスガードはスコープ外になるため、自動的にドロップされる。
                // drop(q);
                dbg!(item);
            }
        });

        for i in 0.. {
            queue.lock().unwrap().push_back(i);
            not_empty.notify_one();
            thread::sleep(Duration::from_secs(1));
        }
    })
}
