//!An easy file system isolated from the kernel
#![no_std]
#![deny(missing_docs)]

// [destinyfvcker]
// easy-fs与底层设备驱动之间通过抽象接口 BlockDevice 来连接，采用轮询方式访问 virtio_blk 虚拟磁盘设备，
// 避免调用外设中断的相关内核函数。所以 easy-fs 避免了直接访问进程相关的数据和函数，从而能独立于内核开发。

extern crate alloc;
#[macro_use]
extern crate bitflags;
// #[macro_use]
// extern crate log;
mod bitmap;
mod block_cache;
mod block_dev;
mod efs;
mod layout;
mod vfs;
/// Use a block size of 512 bytes
pub const BLOCK_SZ: usize = 512;
use bitmap::Bitmap;
use block_cache::{block_cache_sync_all, get_block_cache};
pub use block_dev::BlockDevice;
pub use efs::EasyFileSystem;
use layout::*;
pub use vfs::Inode;
pub use vfs::Stat;
pub use vfs::StatMode;

// [destinyfvcker] easy-fs crate 以层次化思路涉及，自上而下可以分成五个层次：
// 1. 磁盘块设备接口层：以块为单位对磁盘块设备进行读写的 trait 接口
// 2. 块缓存层：在内存之中缓存磁盘块的数据，避免频繁读写磁盘
// 3. 磁盘数据结构层：磁盘上的超级块、位图、索引节点、数据块、目录项等核心数据结构和相关处理，layout.rs 和 bitmap.rs
// 4. 磁盘块管理器层：合并了上述核心数据结构和磁盘布局所形成的磁盘文件系统数据结构，block_cache.rs
// 5. 索引节点层：管理索引节点，实现了文件创建/文件打开/文件读写等成员函数，block_dev.rs

// [destinyfvcker] easy-fs 磁盘按照块编号从小到大顺序分成 5 个连续区域
// 1. 第一个区域只有一个块，也就是超级块（Super Block），用于定位其他连续区域的位置，检查文件系统的合法性。
// 2. 第二个区域是一个索引节点位图，长度是若干个块，记录了索引节点区域之中有哪些索引节点已经被分配出去使用了、
// 3. 第三个区域是一个索引节点区域，长度是若干个块，其中的每一个块都存储了若干个索引节点。
// 4. 第四个区域是一个数据块位图，长度是若干个块，记录了后面的数据块区域之中有哪些已经被分配出去使用了。
// 5. 最后的区域是数据块区域，其中的每一个被分配出去的块保存了文件或者目录的具体内容

// [destinyfvcker] 下面分别介绍文件系统的使用者对于文件系统的一些常用操作：
//
// 1. 获取根目录的 inode
// 文件系统的使用者在通过 EasyFileSystem::open 从装载了 easy-fs 镜像的块设备上打开 easy-fs 之后，
// 要做的第一件事情就是获取根目录的 Inode（位于 vfs.rs）。
// 因为我们目前仅支持绝对路径，对于任何文件/目录的索引都必须从根目录开始向下逐级进行。
// 等到索引完成之后，我们才能对文件/目录进行操作。
// 事实上 EasyFileSystem 提供了另一个名为 root_inode 的方法来获取根目录的 Inode
