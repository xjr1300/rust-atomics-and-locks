//! このプログラムのメッセージの出力順序は、プログラムを実行するたびに変化する。
//!
//! `join`でスレッドの終了を待たないと、`main`関数が`f`関数の終了を待たないため、
//! 関数`f`が出力するメッセージは出力されたり、されなかったりする。
//! 関数`f`が出力するメッセージを確実に出力するためには、`join`メソッドでスレッドの終了を待つ。
use std::thread;

fn main() {
    let t1 = thread::spawn(f);
    let t2 = thread::spawn(f);

    println!("Hello from the main thread.");

    t1.join().unwrap();
    t2.join().unwrap();
}

fn f() {
    println!("Hello from another thread!");

    let id = thread::current().id();
    println!("This is my thread id: {id:?}");
}
