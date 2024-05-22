//! File and filesystem-related syscalls
use crate::fs::{make_pipe, open_file, OpenFlags, Stat};
use crate::mm::{translated_byte_buffer, translated_refmut, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token};
use alloc::sync::Arc;

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

// [destinyfvcker] 实际上这段代码的逻辑因为我看不懂 Makefile，在这个项目之中改不过来，
// 所以如果想要看到实际的运行效果，请移步 rCore-Tutorial-v3
// 功能：为当前进程打开一个管道
// 参数：pipe 表示应用地址空间之中的一个长度为 2 的 usize 数组的起始位置，内核需要按顺序将管道读端和写段的文件描述符写入到数组之中
// 返回值：如果出现了错误就返回-1，否则返回 0.可能的错误原因就只有：传入的地址不合法
pub fn sys_pipe(pipe: *mut usize) -> isize {
    trace!("kernel:pid[{}] sys_pipe", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let mut inner = task.inner_exclusive_access();
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    *translated_refmut(token, pipe) = read_fd;
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd;
    0
} // 只有当一个管道的所有读端/写端文件都被关闭之后，管道占有的资源才会被回收

// [destinyfvcker] 这个系统调用在用户态的使用方式
// 注意：这里是在父进程之中通过系统调用打开的管道文件数组，而且对应的 OSInode 放在文件打开表之中，
// 而且对于 OSInode 而言，只要还有指向它的 Arc 指针，就不会真正将对应的资源释放掉
//
// let mut pipe_fd = [0usize; 2];
// pipe(&mut pipe_fd);
// assert_eq!(pipe_fd[0], 3);
// assert_eq!(pipe_fd[1], 4);
//
// [destinyfvcker] 在 fork 的时候会把子进程的文件打开表复制一份，
// 所以子进程也可以通过同样的文件描述符来访问同一个管道的读端和写端，但是之前提到过管道是单向的，
// 在这个测例之中我们希望管道之中的数据从父进程流向子进程，
// 也就是父进程仅仅通过管道的写端写入数据，而子进程仅仅通过管道的读端读取数据
//
// [destinyfvcker] 所以这里分别在第一时间在子进程自重关闭管道的鞋垫和在
// 父进程之中关闭管道的读端，要是想要在父子进程之中实现双向通信，就必须要创建两个管道。
// if fork() == 0 {
//     close(pipe_fd[1]);
//     let mut buffer = [0u8; 32];
//     let len_read = read(pipe_fd[0], &mut buffer) as usize;

//     close(pipe_fd[0]);
//     assert_eq!(core::str::from_utf8(&buffer[..len_read]).unwrap(), STR);
//     println!("Read OK, child process exited!");
//     0
// } else {
//     close(pipe_fd[0]);
//     assert_eq!(write(pipe_fd[1], STR.as_bytes()), STR.len() as isize);
//     close(pipe_fd[1]);
//     let mut child_exit_code: i32 = 0;
//     wait(&mut child_exit_code);
//     assert_eq!(child_exit_code, 0);
//     println!("pipetest passed!");
//     0
// }

// [destinyfvcker] 关于如何实现父子进程之间的双向通信，这段代码见项目根目录之中的 pipe_large_test.md 文件

//  下面这个系统调用是为了对应用进程的文件描述符表进行某种替换
/// 功能：将进程之中的一个已经打开的文件复制一份并分配到一个新的文件描述符中
/// 参数：fd 表示进程之中一个已经打开的文件的文件描述符
/// 返回值：如果出现了错误就返回 -1，否则就能够访问已经打开文件的新的文件描述符
/// 可能的错误原因是：传入的 fd 并不对应一个合法的已打开文件
pub fn sys_dup(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_dup", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    // [destinyfvcker] 在文件描述符表之中找到第一个没有被占用的项
    let new_fd = inner.alloc_fd();

    // [destinyfvcker] as_ref() 方法在这里的作用是：Converts from &Option<T> to Option<&T>.
    // (as_ref 函数接受的参数是 &self，传入所有权也没有问题，可以借出来，这个方法是在 core::option::Option 之中实现的)
    //
    // 所以在 inner.fd_table[fd] 之中取出来的值实际上是 Option<Arc<dyn File + Sync + Send>>，
    // 在经过了 as_ref 之后就变味了 Option<&Arc<dyn File + Sync + Send>>，Arc::clone 正好接受一个不可变借用
    //
    // 所以说这里仅仅是拷贝了一份指针，它们实际上指向的是同一份文件
    inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}

// [destinyfvcker] 关于如何在用户应用程序之中实现输入和输出重定向，见user_shell.md

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    -1
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    -1
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    -1
}
