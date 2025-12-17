//! `Condvar`: 条件変数
//!
//! 条件変数は、特定の条件（イベント）が満たされるまでにスレッドを待機させるための同期プリミティブである。
//! 待機中のスレッドは、CPU時間を消費せずにブロックされる。
//!
//! 条件変数は通常、論理述語（待機条件）とミューテックスと組み合わせて使用される。
//! 待機条件となる述語は、常にミューテックスをロックした状態で評価されなければならない。
//!
//! また、条件変数の待機は「スプリアスウェイクアップ（spurious wakeup、偽の目覚め）」が発生する可能性
//! があるため、待機は必ずループ内で行い、起床後に条件を再評価する必要がある。
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

/// 型パラメーター`T`に対して`Send`と`Sync`のトレイト境界を明示していない理由は、`T`が`Mutex`によって
/// 保護されていることをRustコンパイラが認識しているためである。
///
/// `Mutex<T>`は、`T: Send`である場合に`Sync`となる。
/// このため、`T`が`Send`であれば、`Mutex<VecDeque<T>>`と`Condvar`のみをフィールドに持つ
/// `Channel<T>`も`Send`かつ`Sync`となる。
#[derive(Default)]
pub struct Channel<T> {
    queue: Mutex<VecDeque<T>>,
    item_ready: Condvar,
}

impl<T> Channel<T> {
    // pub fn new() -> Self {
    //     Self {
    //         queue: Mutex::new(VecDeque::new()),
    //         item_ready: Condvar::new(),
    //     }
    // }

    pub fn send(&self, message: T) {
        self.queue.lock().unwrap().push_back(message);
        // 同じ`Condvar`に対して待機しているスレッドのうち、いずれか1つを起床させる。
        // ただし、`Condvar`は待機中のスレッドを起床させるだけで、スプリアスウェイクアップ
        // を考慮して、起床後条件が成立しているかを確認する必要がある。
        self.item_ready.notify_one();
    }

    pub fn receive(&self) -> T {
        let mut queue = self.queue.lock().unwrap();
        // スプリアスウェイクアップに備えて、ループ内で条件を再評価する。
        // このプログラムにおける条件は、キューが空でないことである。
        // したがって、キューからメッセージを取り出せるまでループしている。
        loop {
            if let Some(message) = queue.pop_front() {
                return message;
            }
            // `Condvar::wait()`は、待機するときに`Mutex`のロックを解放し、
            // 通知を受けて起床した後に、再びロックを取得してから制御を返す。
            // したがって、ロックを取得できるまで処理が進まないことがある。
            //
            // このため、待機中は`Mutex`がロックされたままになることはない。
            // また、スプリアスウェイクアップが発生する場合に備え、必ずループ内で
            // 条件を再評価する。
            queue = self.item_ready.wait(queue).unwrap();
        }
    }
}

fn main() {
    // let channel = Arc::new(Channel::new());
    let channel = Arc::new(Channel::default());
    let cloned_channel = Arc::clone(&channel);

    let sender = std::thread::spawn(move || {
        for i in 0..5 {
            println!("Sending: {i}");
            channel.send(i);
            std::thread::sleep(std::time::Duration::from_nanos(1));
        }
    });

    let receiver = std::thread::spawn(move || {
        for _ in 0..5 {
            let message = cloned_channel.receive();
            println!("Received: {message}");
        }
    });

    receiver.join().unwrap();
    sender.join().unwrap();
}
