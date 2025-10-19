use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

fn allocate_new_id_with_wrap_around() -> u32 {
    // NEXT_IDは返されるIDより1多い値を記録している。
    static NEXT_ID: AtomicU32 = AtomicU32::new(0);
    NEXT_ID.fetch_add(1, Relaxed)
}

#[allow(dead_code)]
fn allocate_new_id_with_problem() -> u32 {
    static NEXT_ID: AtomicU32 = AtomicU32::new(0);
    let id = NEXT_ID.fetch_add(1, Relaxed);
    println!("current value of NEXT_ID: {}", NEXT_ID.load(Relaxed));
    assert!(id < 10, "too many IDs!");
    id
}

fn allocate_new_id_with_decrement() -> u32 {
    static NEXT_ID: AtomicU32 = AtomicU32::new(0);
    let id = NEXT_ID.fetch_add(1, Relaxed);
    if 10 <= id {
        NEXT_ID.fetch_sub(1, Relaxed);
        println!("current value of NEXT_ID: {}", NEXT_ID.load(Relaxed));
        panic!("too many IDs!");
    }
    id
}

fn main() {
    println!("allocate new id with wrap around");
    for _ in 0..=10 {
        println!("id: {}", allocate_new_id_with_wrap_around());
    }
    println!();

    // println!("allocate new id with problem");
    // for _ in 0..=10 {
    //     println!("id: {}", allocate_new_id_with_problem());
    // }
    // println!();

    println!("allocate new id with decrement");
    for _ in 0..=10 {
        println!("id: {}", allocate_new_id_with_decrement());
    }
    println!();
}
