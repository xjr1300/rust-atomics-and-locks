use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
};
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    println!("Inaccurate statistics is started...");
    inaccurate_statistics();

    println!();

    println!("Accurate statistics is started...");
    accurate_statistics();
}

fn inaccurate_statistics() {
    let num_done = &AtomicUsize::new(0);
    let total_time = &AtomicU64::new(0);
    let max_time = &AtomicU64::new(0);

    thread::scope(|s| {
        // 4つのバックグラウンドスレッドが、それぞれ25アイテムを処理し、合計100アイテム処理
        for t in 0..4 {
            s.spawn(move || {
                for i in 0..25 {
                    let start = Instant::now();
                    process_item(t * 25 + i);
                    let time_taken = start.elapsed().as_micros() as u64;
                    // この4つのスレッドのあるスレッドがnum_doneを更新して、total_timeやmax_timeを
                    // 更新していない瞬間に、メインスレッドが次の3つのアトミック変数をロードする瞬間がある。
                    // また、Ordering::Relaxedで更新しているため、次の3つのアトミック変数を更新する
                    // 順番は保証されない。したがって、max_timeが更新されて、num_doneが更新されていない
                    // 瞬間がある。
                    num_done.fetch_add(1, Relaxed);
                    total_time.fetch_add(time_taken, Relaxed);
                    max_time.fetch_max(time_taken, Relaxed);
                }
            });
        }

        // メインスレッドは統計値を毎秒更新
        loop {
            let total_time = Duration::from_micros(total_time.load(Relaxed));
            let max_time = Duration::from_micros(max_time.load(Relaxed));
            let n = num_done.load(Relaxed);

            if n == 100 {
                break;
            }

            print_statistics(n, total_time, max_time);

            thread::sleep(Duration::from_secs(1));
        }
    });

    println!("Done!");
}

fn print_statistics(num_done: usize, total_time: Duration, max_time: Duration) {
    if num_done == 0 {
        println!("Working.. nothing done yet.");
    } else {
        println!(
            "Working.. {num_done}/100 done, {:?} average, {:?} peak",
            total_time / num_done as u32,
            max_time,
        );
    }
}

#[derive(Debug, Default)]
struct Statistics {
    num_done: usize,
    total_time: u64,
    max_time: u64,
}

fn accurate_statistics() {
    let statistics = Arc::new(Mutex::new(Statistics::default()));
    let statistics_in_scope = Arc::clone(&statistics);

    thread::scope(|s| {
        for t in 0..4 {
            let statistics_in_thread = Arc::clone(&statistics_in_scope);
            s.spawn(move || {
                for i in 0..25 {
                    let start = Instant::now();
                    process_item(t * 25 + i);
                    let time_taken = start.elapsed().as_micros() as u64;
                    // Mutexで保護することで、3つの変数に対する操作をアトミックに処理
                    let mut guard = statistics_in_thread.lock().unwrap();
                    guard.num_done += 1;
                    guard.total_time += time_taken;
                    guard.max_time = guard.max_time.max(time_taken);
                }
            });
        }

        loop {
            // 統計値を取得するためにロック
            let guard = statistics.lock().unwrap();
            let total_time = Duration::from_micros(guard.total_time);
            let max_time = Duration::from_micros(guard.max_time);
            let n = guard.num_done;
            // 上記ロックを解除
            std::mem::drop(guard);

            if n == 100 {
                break;
            }

            print_statistics(n, total_time, max_time);

            thread::sleep(Duration::from_secs(1));
        }
    })
}

fn process_item(_: usize) {
    thread::sleep(Duration::from_millis(500));
}
