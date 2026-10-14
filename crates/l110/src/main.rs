use l110_core::{Args, Ctx};

unsafe extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
}

/// Rust ตั้ง SIGPIPE เป็น ignore ทำให้ `cmd | head` ได้ error "Broken pipe";
/// เครื่องมือ Unix ปกติต้องตายเงียบๆ ด้วย SIGPIPE จึงคืนค่าเป็น default
fn restore_sigpipe() {
    const SIGPIPE: i32 = 13;
    const SIG_DFL: usize = 0;
    // SAFETY: เรียก signal(2) ตอนเริ่ม process ก่อนสร้าง thread ใดๆ ด้วย handler ค่า SIG_DFL
    unsafe {
        signal(SIGPIPE, SIG_DFL);
    }
}

fn main() -> std::process::ExitCode {
    restore_sigpipe();
    let args = Args::from_env();
    let mut ctx = Ctx::stdio("l110");
    l110::run(&mut ctx, args).into()
}
