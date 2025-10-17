use std::thread;

fn main() {
    let numbers = vec![1, 2, 3];
    // let mut numbers = vec![1, 2, 3];

    // `scope`関数内で起動したスレッドは、その関数のスコープを超えて長生きしない。
    // `scope`関数内で起動したすべてのスレッドは、その関数が終了すると、スレッドの終了を待機する。
    // したがって、明示的なジョインは必要ない。
    thread::scope(|s| {
        s.spawn(|| {
            println!("length: {}", numbers.len());
        });
        s.spawn(|| {
            for n in &numbers {
                println!("{n}");
            }
            // numbers.push(4);
        });
    });
}
