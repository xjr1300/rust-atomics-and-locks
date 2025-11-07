use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

static A: AtomicBool = AtomicBool::new(false);
static B: AtomicBool = AtomicBool::new(false);

static mut S: String = String::new();

fn main() {
    // スレッドaがAにtrueが保存し、スレッドbがBにtrueを保存した場合、
    // どちらのスレッドもSにアクセスしない。
    //
    // スレッドaがAにtrueが保存し、スレッドbがBにtrueを保存していない場合、
    // スレッドaはSにアクセスして、スレッドbはSにアクセスしない。
    // スレッドbは、Bにtrueを保存した後（ただしスレッドaはBがfalseであったことを観測済み）、
    // Aの値をロードする。このときメモリオーダリングはSeqCstである。
    // SeqCstオーダリングでは、すべてのスレッドでSeqCst操作が単一の全体順序で並ぶように見える。
    // したがって、スレッドaが「Bがfalseである」と観測してからAにtrueを保存したのであれば、
    // スレッドbがそのAの値をロードする際には、Aがtrueであることを観測しなければならない。
    // よって、スレッドbはAがtrueであることを観測して、Sにアクセスしない。
    //
    // 上記は、AとBを入れ替えても同様に成り立つ。
    //
    // SeqCstは実際に1つの順序で処理が*実行された*のではなく、その順番で実行されたように
    // *観測できる*ことに注意すること。
    let a = thread::spawn(|| {
        A.store(true, Ordering::SeqCst);
        if !B.load(Ordering::SeqCst) {
            unsafe {
                #[allow(static_mut_refs)]
                S.push('!')
            }
        }
    });

    let b = thread::spawn(|| {
        B.store(true, Ordering::SeqCst);
        if !A.load(Ordering::SeqCst) {
            unsafe {
                #[allow(static_mut_refs)]
                S.push('!')
            }
        }
    });

    a.join().unwrap();
    b.join().unwrap();
}
