//! File trait & inode(dir, file, pipe, stdin, stdout)

mod inode;
mod stdio;

use crate::mm::UserBuffer;
use easy_fs::Stat;

// [destinyfvcker] read 是指从文件（I/O）资源之中读取数据放到缓冲区之中，最多将缓冲区填满（也就是读取缓冲区长度那么多的字节）
// write 指的是将缓冲区中的数据写入文件，最多将缓冲区的数据全部写入，并返回直接写入的字节数

// [destinyfvcker] 文件可以代表很多不同类型的 I/O 资源，但是在进程看来，
// 所有文件的访问都可以通过一个简洁统一的抽象接口 File 来进行
/// trait File for all file types
pub trait File: Send + Sync {
    /// the file readable?
    fn readable(&self) -> bool;
    /// the file writable?
    fn writable(&self) -> bool;
    // [destinyfvcker] UserBuffer 是我们在 mm 子模块之中定义的应用地址空间之中的一段缓冲区，可以将其看成一个 &[u8] 的切片
    /// read from the file to buf, return the number of bytes read
    fn read(&self, buf: UserBuffer) -> usize;
    /// write to the file from buf, return the number of bytes written
    fn write(&self, buf: UserBuffer) -> usize;
    /// get stat of the file
    fn stat(&self, st: &mut Stat);
}

pub use inode::{inode_link, inode_unlink};
pub use inode::{list_apps, open_file, OSInode, OpenFlags};
pub use stdio::{Stdin, Stdout};
