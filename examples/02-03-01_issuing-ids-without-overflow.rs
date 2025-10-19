//! CAS（Compare-And-Swap）命令（`compare_exchange`）は、マルチスレッド環境において
//! ロックを使用せずに安全に値を更新するための原子的（アトミック）な操作である。
//!
//! `compare_exchange`は、指定された期待値と現在の値が一致している場合にのみ、
//! 新しい値へ更新を試みる。これにより、複数スレッドから同時にアクセスされても
//! 一貫性のある更新が保証される。
//!
//! 一方で、`compare_exchange`は値が一致している場合には必ず更新を成功させる必要があるため、
//! ハードウェアによってはややコストが高くなることがある。
//!
//! これに対して、`compare_exchange_weak`はハードウェア実装に依存して
//! 「スプリアス失敗（値が一致していても一時的な理由で失敗する）」が発生する可能性がある。
//! ただし、この失敗はABA問題とは無関係である。
//!
//! そのため、ロックフリーな更新処理を行う場合は、`compare_exchange_weak`をループ内で
//! 繰り返し呼び出して成功するまで再試行するのが一般的な慣習である。
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

fn allocate_new_id() -> u32 {
    static NEXT_ID: AtomicU32 = AtomicU32::new(0);
    let mut id = NEXT_ID.load(Relaxed);
    loop {
        assert!(id < 1000, "too many IDs!");
        // 現在値よりも常に1大きいIDをNEXT_IDに割り当てる。
        // 現在値が想定と異なる場合、現在値をid変数に割り当て、次の比較交換で現在値よりも1大きいIDをNEXT_IDに割り当て。
        match NEXT_ID.compare_exchange_weak(id, id + 1, Relaxed, Relaxed) {
            Ok(_) => return id,
            Err(v) => id = v,
        }
    }
}

fn main() {
    for _ in 0..=1000 {
        println!("id: {}", allocate_new_id());
    }
}
