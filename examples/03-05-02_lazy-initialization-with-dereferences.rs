use std::sync::atomic::{AtomicPtr, Ordering};
use std::thread;

#[allow(dead_code)]
#[derive(Debug)]
struct Data {
    foo: i32,
}

fn generate_data() -> Data {
    Data { foo: 42 }
}

fn get_data() -> &'static Data {
    static PTR: AtomicPtr<Data> = AtomicPtr::new(std::ptr::null_mut());

    let mut p = PTR.load(Ordering::Acquire);

    if p.is_null() {
        p = Box::into_raw(Box::new(generate_data()));

        // compare_exchange内部では、現在の値を読み出して比較する必要がある。
        // この読み出しのメモリオーダリングは、成功・失敗の結果によって事後的に決定される。
        // 成功した場合はsuccessオーダリング（ここではRelease）が、失敗した場合はfailureオーダリング
        // （ここではAcquire）が適用されたように振る舞う。
        //
        // したがって、このCAS（Compare-And-Swap／Compare-And-Set）操作が成功したとき：
        //   - PTRがnullであることを確認した上で、上記新しいポインタpがPTRに書き込まれる。
        //   - このストアはReleaseオーダリングで行われるため、generate_dataで初期化された内容が
        //     他のスレッドからAcquireロードを通して正しく観測されることが保証される。
        //
        // 一方、CASが失敗した場合（＝他のスレッドがすでにPTRを更新していた場合）：
        //   - 失敗したCASはfailureオーダリング（ここではAcquire）でPTRの値を読み取ったように
        //     振る舞う。
        //   - その結果、他のスレッドが生成したDataの初期化済み状態を正しく観測できる。
        //
        // 失敗した場合、現在のPTRの値（他スレッドが確保したポインタ）がErr(e)として返る。
        // このとき、自分で確保した領域pは不要なのでBox::from_rawで再びBoxに戻してドロップし、
        // メモリリークを防ぐ。
        //
        // 最後に、pを他スレッドが確保したポインタ（e）に更新し、それを戻り値として返す。
        if let Err(e) = PTR.compare_exchange(
            std::ptr::null_mut(),
            p,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            drop(unsafe { Box::from_raw(p) });
            p = e;
        }
    }

    unsafe { &*p }
}

fn main() {
    thread::scope(|s| {
        for _ in 0..100 {
            s.spawn(get_data);
        }
    });
}
