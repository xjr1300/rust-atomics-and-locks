#[cfg(not(target_os = "linux"))]
compile_error!("Linux only. Sorry!");

use std::sync::atomic::{AtomicU32, Ordering};

pub fn wait(a: &AtomicU32, expected: u32) {
    unsafe {
        // Futexシステムコールを呼び出し、aがexpectedと等しい場合、時間制限無しで待機する。
        libc::syscall(
            libc::SYS_futex,
            a as *const AtomicU32,
            libc::FUTEX_WAIT,
            expected,
            std::ptr::null::<libc::timespec>(),
        );
    }
}

pub fn wake_one(a: &AtomicU32) {
    unsafe {
        // Futexシステムコールを呼び出し、aを待機しているスレッドのうち1つを起こす。
        libc::syscall(libc::SYS_futex, a as *const AtomicU32, libc::FUTEX_WAKE, 1);
    }
}

fn main() {
    // 待機条件となるアトミック変数
    // 初期値は0で、0の場合はメインスレッドを停止し、0以外になったら起こす。
    let a = AtomicU32::new(0);

    std::thread::scope(|s| {
        s.spawn(|| {
            // 3秒待ってから、aを1に設定し、メインスレッドを起こす。
            std::thread::sleep(std::time::Duration::from_secs(3));
            a.store(1, Ordering::Relaxed);
            wake_one(&a);
        });

        println!("Waiting...");
        // aが0の場合、ループする。
        while a.load(Ordering::Relaxed) == 0 {
            // aが0の場合、待機する。
            // ただし、Relaxedロードでaが0であっても、wait関数でaが0と評価されなかった場合、
            // wait関数で実行されるFutexシステムコールで待機しない。
            wait(&a, 0);
        }
        println!("Done!");
    });
}
