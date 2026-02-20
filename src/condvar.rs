use std::sync::{Arc, Condvar, Mutex};

pub fn condvar() {
    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let pair2 = Arc::clone(&pair);

    // プロデューサー側
    let handle = std::thread::spawn(move || {
        let (lock, condvar) = &*pair;
        let mut started = lock.lock().unwrap();
        *started = true; // 準備ができた
        condvar.notify_one(); // 通知
    });

    // 消費者側
    let (lock, condvar) = &*pair2;
    let mut started = lock.lock().unwrap();
    while !*started {
        started = condvar.wait(started).unwrap();
    }

    handle.join().unwrap();

    assert!(*started);
}
