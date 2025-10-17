use std::thread;

fn main() {
    f();
    g();
}

fn f() {
    let numbers = vec![1, 2, 3];

    // `numbers`の所有権は`spawn`関数で実行するクロージャーに移動
    thread::spawn(move || {
        for n in numbers {
            println!("{n}");
        }
    })
    .join()
    .unwrap();
}

fn g() {
    let numbers = Vec::from_iter(0..=1000);

    let t = thread::spawn(move || {
        let len = numbers.len();
        let sum = numbers.into_iter().sum::<usize>();
        sum / len
    });

    let average = t.join().unwrap();

    println!("average: {average}");
}
