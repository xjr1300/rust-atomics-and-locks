use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::thread;
use std::time::Duration;

fn main() {
    // 前のストップフラグの例と異なり、num_doneはスコープ付きスレッドで実行され、
    // そのスレッドはメイン関数より長生きしないため、静的変数である必要がない。
    let num_done = AtomicUsize::new(0);

    thread::scope(|s| {
        // このバックグラウンドスレッドで100個のアイタムすべてを処理
        s.spawn(|| {
            for i in 0..100 {
                // process_itemの処理は少し時間がかかる
                process_item(i);
                num_done.store(i + 1, Relaxed)
            }
        });

        // メインスレッドは毎秒1回状態を更新
        loop {
            let n = num_done.load(Relaxed);
            if n == 100 {
                break;
            }
            println!("Working.. {n}/100 done");
            thread::sleep(Duration::from_secs(1));
        }
    });

    println!("Done!");
}

fn process_item(_: usize) {
    thread::sleep(Duration::from_millis(200));
}
