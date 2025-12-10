//! # 3.8 Fence
//!
//! フェンス（fence）とは、メモリ操作の順序を強制的に制御する命令である。
//! CPUやコンパイラは、高速化のために命令を並び替える（リオーダリング）ことがある。
//! しかし、フェンスを入れることで、このリオーダリングを制限できる。
//!
//! ## Releaseフェンス
//!
//! Releaseフェンスは、Releaseフェンスよりも前に行った書き込みが、Releaseフェンス
//! より後に行われる公開操作（たとえば、読み手への合図となるストア）より後に、他のスレッド
//! から観測されることを防ぐ働きをする。
//!
//! つまり、Releaseフェンスより前の書き込みは、Releaseフェンスより後の公開操作より
//! も先に観測されるという順序関係を確立する。
//!
//! ```rust
//! use std::sync::atomic::{AtomicBool, fence, Ordering};
//!
//! static READY: AtomicBool = AtomicBool::new(false);
//! static mut DATA: i32 = 0;
//!
//! fn producer() {
//!     unsafe { DATA = 42; }                   // 共有データの書き換え
//!     fence(Ordering::Release);               // これより前の書き込みを公開可能にする
//!     READY.store(true, Ordering::Relaxed);   // 読み手に合図
//! }
//! ```
//!
//! 他のスレッドである読み手が`READY == true`を観測し、対応するAcquire操作を行った場合、
//! `DATA == 42`を観測することが保証される。
//!
//! ## Acquireフェンス
//!
//! Acquireフェンスは、Acquireフェンスより後に行われる読み込みが、Acquireフェンスよりも
//! 前に行われた操作より先に実行されることを防ぐ働きを持つ。
//!
//! つまり、Acquireフェンスより後の読み込みは、必ずAcquireフェンスより前の公開操作
//! （たとえば、読み手が観測した合図となるロード）より後に実行されるという順序関係が確立
//! される。
//!
//! ```rust
//! use std::sync::atomic::{AtomicBool, fence, Ordering};
//!
//! static READY: AtomicBool = AtomicBool::new(false);
//! static mut DATA: i32 = 0;
//!
//! fn consumer() {
//!     while !READY.load(Ordering::Relaxed) {} // 合図を待つ
//!     fence(Ordering::Acquire);               // これより後の読み込みを順序付け
//!     let value = unsafe { DATA };            // Release書き込みを観測
//!     println!("{}", value);                  // 42が出力される
//! }
//! ```
//!
//! Acquireフェンスは、Acquireフェンスより後に行われる読み込みが、Acquire
//! フェンスよりも前に先行して実行されることを禁止し、Release側の「公開」された
//! 書き込みを確実に読み込めるようにする。
//!
//! したがって、`READY = true`を観測した後、`DATA = 42`を観測することが保証される。
//!
//! ## ReleaseフェンスとAcquireフェンスの関係
//!
//! ReleaseフェンスとAcquireフェンスが対になって働くと、「Releaseフェンス
//! より前の操作は、対応するAcquireフェンスの後で必ず観測できる」という順序関係
//! が成立する。
//!
//! producer thread                 consumer thread
//! --------------------            --------------------
//! DATA = 42;                      // ← この書き込みを確実に観測できる
//! fence(Release);
//! READY.store(true, Relaxed);     // 合図された
//!                                 while !READY.load(Relaxed) {}
//!                                 fence(Acquire);             // READY=trueを観測したので以降を実行
//!                                 value = unsafe { DATA };    // 確実に42を読み込む
//!
//! ## フェンスの用途
//!
//! `AtomicBool`などの`store`や`load`には、`Ordering::Release`や
//! `Ordering::Acquire`を直接指定して順序関係を確立できる。
//! しかし、次の通り、それらの操作とは独立にフェンスを置くことがある。
//!
//! * 複数の通常の変数を書き込んだ後、まとめて`Release`（公開）
//! * ある条件を満たしたら、以降の操作の順序を強制（上記コード例）
//! * ロックや同期プリミティブなどの低水準ライブラリの実装
//!
//! ## SeqCstフェンス
//!
//! SeqCstフェンスは、すべてのスレッドで共通な一本の順序線の上に「フェンス点」を打つ操作である。
//!
//! SeqCstフェンスより前の操作はSeqCstフェンスより前に、SeqCstフェンスよりも後の操作はSeqCstフェンスより後に、
//! 順番に観測されることが保証される。
//!
//! ## まとめ
//!
//! | フェンスの種類 | 保証すること | 保証の範囲 | 共通な順序 | 用途 |
//! | --- | --- | --- | --- | --- |
//! | Release | フェンスより前の書き込みが、フェンスより後の公開操作より後に観測されない | 局所的（該当スレッド内） | ❌️ | データ公開（書き手側） |
//! | Acquire | フェンスより後の読み込みが、フェンスより前の公開操作より前に実行されない | 局所的（該当スレッド内） | ❌️ | データ取得（読み手側） |
//! | SeqCst | フェンスの前後関係がすべてのスレッドで共通の1つの順序線に並ぶ | グローバル | ⭕️ | すべてのスレッドで順序を揃える |
//!
//! * Releaseフェンス
//!
//! ```rust
//! use std::sync::atomic::{AtomicBool, fence, Ordering};
//!
//! static READY: AtomicBool = AtomicBool::new(false);
//! static mut DATA: i32 = 0;
//!
//! fn producer() {
//!     unsafe { DATA = 42; };      // フェンスより前の書き込み
//!     fence(Ordering::Release);   // Releaseフェンスで上の書き込みを公開
//!     // 読み手側への合図
//!     // フェンスより前の書き込みである`DATA = 42`が、フェンスより後の公開操作である`READY = true`より後に観測されない。
//!     // つまり、`READY = true`が観測されたら、`DATA = 42`を観測する。
//!     READY.store(true, Ordering::Relaxed);
//! }
//! ```
//!
//! * Acquireフェンス
//!
//! ```rust
//! fn consumer() {
//!     while !READY.load(Ordering::Relaxed) {}     // プロデューサーからの合図を待つ
//!     fence(Ordering::Acquire);   // Acquireフェンスで公開されたデータを正しく観測
//!     // フェンスより後の`DATA`の読み込みが、フェンスより前の公開操作である`READY = true`より前に実行されない。
//!     // つまり、`READY = true`が観測されたら、`DATA = 42`を観測する。
//!     // ただし、ここでは常に`READY == true`であるため、必ず`DATA = 42`を観測する。
//!     let v = unsafe { DATA };
//! }
//! ```
//!
//! * SeqCstフェンス
//!
//! すべてのスレッドで同じ順序になるため、`LOG`には次のいずれかが格納される。
//!
//! * Aフェンスが最初の場合
//!
//! ```text
//! A1, A2, B1, B2
//! ```
//!
//! * Bフェンスが最初の場合
//!
//! ```text
//! B1, B2, A1, A2
//! ```
//!
//! ```rust
//! static mut LOG: Vec<&'static str> = Vec::new();
//!
//! fn thread_a() {
//!     unsafe { LOG.push("A1"); }
//!     fence(Ordering::SeqCst);    // Aフェンス
//!     unsafe { LOG.push("A2"); }
//! }
//!
//! fn thread_b() {
//!     unsafe { LOG.push("B1"); }
//!     fence(Ordering::SeqCst);    // Bフェンス
//!     unsafe { LOG.push("B2"); }
//! }
//! ```
use std::sync::atomic::{AtomicBool, Ordering, fence};
use std::thread;
use std::time::Duration;

static mut DATA: [u64; 10] = [0; 10];

#[allow(clippy::declare_interior_mutable_const)]
const ATOMIC_FALSE: AtomicBool = AtomicBool::new(false);
static READY: [AtomicBool; 10] = [ATOMIC_FALSE; 10];

fn main() {
    for i in 0..10 {
        thread::spawn(move || {
            let data = some_calculation(i);
            unsafe {
                DATA[i] = data;
            }
            READY[i].store(true, Ordering::Release);
        });
    }

    thread::sleep(Duration::from_millis(500));

    let ready: [bool; 10] = std::array::from_fn(|i| READY[i].load(Ordering::Relaxed));
    if ready.contains(&true) {
        fence(Ordering::Acquire);
        for i in 0..10 {
            if ready[i] {
                println!("data{i} = {}", unsafe { DATA[i] });
            }
        }
    }
}

fn some_calculation(i: usize) -> u64 {
    thread::sleep(Duration::from_millis(400 + i as u64 % 3 * 100));
    (i * i) as u64
}
