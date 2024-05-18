//! File and filesystem-related syscalls
use crate::fs::{open_file, OpenFlags, Stat};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token};

// [destinyfvcker] 我原来还以为下面的内容是放在 process.rs 模块之中的，
// 但是这里实际上是独立出来了，说明这是关于文件系统-file system-fs的内容吗？

// [destinyfvcker] +----- impl in ch6 -----+
// 现在文件读写系统调用 sys_read/write 更加具有普适性，而不仅仅局限于之前特定的标准输入/输出

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
        // [destinyfvcker?] 关于为什么这里要先进行一个 clone，实际上完全没有必要，
        // 不可变引用可以在同一时间存在多个，这里借出的引用也全部都是不可变引用，没有多重借用的问题啊?
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

// [destinyfvcker] 阉割版的 open 系统调用，原本的函数签名应该是：
// fn sys_openat(dirfd: usize, path: &str, flags: u32, mode: u32) -> isize
// 上面这个函数签名的形式用于本章的用户库中
//
// 功能：打开一个常规文件，并返回访问它的文件描述符
// 参数：
// 1.path 描述要打开的文件名，这里不需要文件路径，因为阉割掉了，所有的文件都放在根目录/下.
// 2.flags 描述打开文件的标志
// - 0，则表示以只读模式 RDONLY 打开
// - 0x001，只写 WRONLY
// - 0x002，既可读又可写 RDWR
// - 0x200，CREATE
//      - 找不到文件的时候创建文件
//      - 文件已经存在则将文件的大小归零
// - 0x400，TRUNC 打开文件的时候清空文件的内容并将该文件的大小归零
//
// bitflags! {
//     pub struct OpenFlags: u32 {
//         const RDONLY = 0;
//         const WRONLY = 1 << 0;
//         const RDWR = 1 << 1;
//         const CREATE = 1 << 9;
//         const TRUNC = 1 << 10;
//     }
// }
//
// 用户态调用：
// pub fn open(path: &str, flags: OpenFlags) -> isize {
//     sys_openat(AT_FDCWD as usize, path, flags.bits, OpenFlags::RDWR.bits)
// }
//
// 可能的错误原因就只有文件不存在了
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
