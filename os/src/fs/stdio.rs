//!Stdin & Stdout
use core::panic;

use super::File;
use crate::mm::UserBuffer;
use crate::sbi::console_getchar;
use crate::task::suspend_current_and_run_next;

// [destinyfvcker] 在第五章之中引入了标准输入接口 sys_read（在 syscall 模块之中），
// 并在用户库之中将其进一步封装成每次能够从标准输入之中获取一个字符的 getchar 函数
/// stdin file for getting chars from console
pub struct Stdin;

// [destinyfvcker] 实际上在第二章就对应用程序引入了基于文件的标准输出接口 sys_write （之前一直都放在 console.rs 模块之中）
// 用于 println! 和 print! 宏在屏幕上打印信息
/// stdout file for putting chars to console
pub struct Stdout;

// [destinyfvcker] 为标准输入流实现我们的文件系统接口
impl File for Stdin {
    // 标准输入流是只读不能写的
    fn readable(&self) -> bool {
        true
    }
    fn writable(&self) -> bool {
        false
    }
    fn read(&self, mut user_buf: UserBuffer) -> usize {
        assert_eq!(user_buf.len(), 1);
        // busy loop
        let mut c: usize;
        loop {
            c = console_getchar();
            if c == 0 {
                suspend_current_and_run_next();
                continue;
            } else {
                break;
            }
        }
        let ch = c as u8;
        unsafe {
            user_buf.buffers[0].as_mut_ptr().write_volatile(ch);
        }
        1
    }
    fn write(&self, _user_buf: UserBuffer) -> usize {
        panic!("Cannot write to stdin!");
    }
    fn stat(&self, _st: &mut easy_fs::Stat) {
        panic!("[stat for Stdin] Not implemented!")
    }
}

// [destinyfvcker] 为标准输出流实现文件系统接口
impl File for Stdout {
    // 标准输入流是只写不能读的
    fn readable(&self) -> bool {
        false
    }
    fn writable(&self) -> bool {
        true
    }
    fn read(&self, _user_buf: UserBuffer) -> usize {
        panic!("Cannot read from stdout!");
    }
    fn write(&self, user_buf: UserBuffer) -> usize {
        for buffer in user_buf.buffers.iter() {
            print!("{}", core::str::from_utf8(*buffer).unwrap());
        }
        user_buf.len()
    }
    fn stat(&self, _st: &mut easy_fs::Stat) {
        panic!("[stat for Stdout] Not implemented!")
    }
}

// [destinyfvcker] 但是实际上之前实现的 console.rs 并没有被 合并掉
// 其中还是封装了对于 print! 和 println! 的实现，包括了为 Stdout 实现 Write trait，
// 但是要注意在 console.rs 之中，struct Stdout 并没有被标记为 pub！！！！！！！！！
// console.rs 的意义仅仅在于提供 println! 宏和 print! 宏来为外部进行调用。
//
// [destinyfvcker?] 但是实际上在这里的 read 和 write 方法实现的用处是在哪里呢？
// 实际上这里的 write 是建立在 console.rs 的基础上的，这里 Stdout 的 write 通过调用 print! 来实现向标准输入流输出，
// 而console.rs 则是建立在 rust_sbi 上的.
