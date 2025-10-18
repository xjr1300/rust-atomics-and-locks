use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::thread;

fn main() {
    // STOPフラグは静的変数でないと、下のバックグラウンドスレッドと、メインスレッドで利用できない。
    // Arc::Mutex<AtomicBool>は、アトミックなAtomicBoolでアトミックを実現するMutexでラップ
    // することになるため過剰である。
    // let STOP: AtomicBool = AtomicBool::new(false);
    static STOP: AtomicBool = AtomicBool::new(false);

    // 何か仕事をするためにスレッドを起動
    let background_thread = thread::spawn(|| {
        // STOPがfalseの場合にループを継続
        while !STOP.load(Relaxed) {
            some_work();
        }
    });

    // メインスレッドでユーザーからの入力を受け付け
    for line in std::io::stdin().lines() {
        match line.unwrap().as_str() {
            "help" => println!("commands: help, stop"),
            "stop" => break,
            cmd => println!("unknown command: {cmd}"),
        }
    }

    // バックグラウンドスレッドを停止を通知するために、STOPをtrueに設定
    STOP.store(true, Relaxed);

    // バックグラウンドスレッドが終了するまで待機
    background_thread.join().unwrap();
}

fn some_work() {
    // この処理に時間がかからなければ、標準入力に"stop"を入力したとき、すぐにバックグラウンドスレッドが終了して
    // プログラムが終了する。逆に時間がかかる場合は、バックグラウンドスレッドの終了に時間がかかる。
    // std::thread::sleep(std::time::Duration::from_secs(5));
    std::thread::sleep(std::time::Duration::from_millis(1));
}
