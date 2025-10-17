use std::collections::VecDeque;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

fn main() {
    let queue = Mutex::new(VecDeque::new());
    let main_thread_id = thread::current().id();
    println!("main thread id: {main_thread_id:?}");

    thread::scope(|s| {
        let producer_thread_id = thread::current().id();
        println!("producer thread id: {producer_thread_id:?}");

        // 消費スレッド
        let t = s.spawn(|| {
            let consumer_thread_id = thread::current().id();
            println!("consumer thread id: {consumer_thread_id:?}");

            loop {
                let item = queue.lock().unwrap().pop_front();
                if let Some(item) = item {
                    dbg!(item);
                } else {
                    thread::park();
                }
            }
        });

        // 生成スレッド（メインスレッドで実行）
        for i in 0.. {
            queue.lock().unwrap().push_back(i);
            t.thread().unpark();
            thread::sleep(Duration::from_secs(1));
        }
    });
}
