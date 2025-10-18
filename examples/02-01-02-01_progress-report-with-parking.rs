use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::thread;
use std::time::Duration;

fn main() {
    let num_done = AtomicUsize::new(0);

    let main_thread = thread::current();

    thread::scope(|s| {
        s.spawn(|| {
            for i in 0..100 {
                process_item(i);
                num_done.store(i + 1, Relaxed);
                // メインスレッドを起こす
                main_thread.unpark();
            }
        });

        loop {
            let n = num_done.load(Relaxed);
            if n == 100 {
                break;
            }
            println!("Working.. {n}/100 done");
            // バックグラウンドスレッドにメインスレッドが起こされるか、1秒経過するまでパーキング
            thread::park_timeout(Duration::from_secs(1));
        }
    });

    println!("Done!");
}

fn process_item(_: usize) {
    thread::sleep(Duration::from_millis(500));
}
