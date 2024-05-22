use super::File;
use crate::mm::UserBuffer;
use crate::sync::UPSafeCell;
use alloc::sync::{Arc, Weak};

use crate::task::suspend_current_and_run_next;

// [destinyfvcker] 基于文件的管道
// 我们将管道的一端（读端或者写端）抽象为 Pipe 类型
/// IPC pipe
pub struct Pipe {
    readable: bool,
    writable: bool,
    // [destinyfvcker]
    // 这个字段实际上是管道自身，是一个带有一定大小缓冲区的字节序列
    buffer: Arc<UPSafeCell<PipeRingBuffer>>,
}

impl Pipe {
    /// create readable pipe
    pub fn read_end_with_buffer(buffer: Arc<UPSafeCell<PipeRingBuffer>>) -> Self {
        Self {
            readable: true,
            writable: false,
            buffer,
        }
    }
    /// create writable pipe
    pub fn write_end_with_buffer(buffer: Arc<UPSafeCell<PipeRingBuffer>>) -> Self {
        Self {
            readable: false,
            writable: true,
            buffer,
        }
    }
}

const RING_BUFFER_SIZE: usize = 32;

// [destinyfvcker] 这个枚举体记录了缓冲区目前的状态
#[derive(Copy, Clone, PartialEq)]
enum RingBufferStatus {
    Full,   // 表示缓冲区已满不能再继续写入
    Empty,  // 表示缓冲区为空无法从里面读取
    Normal, // 表示除了 FULL 和 EMPTY 之外的其他状态
}

// [destinyfvcker] 这实际上是一个循环队列
pub struct PipeRingBuffer {
    arr: [u8; RING_BUFFER_SIZE], // 存放数据的数组
    head: usize,                 // 循环队列队头的下标
    tail: usize,                 // 表示循环队列队尾的下标
    status: RingBufferStatus,
    write_end: Option<Weak<Pipe>>, // 保存了写端的一个弱引用计数，因为在某些情况下需要确认这个管道的所有的写端是否都已经被关闭了
}

impl PipeRingBuffer {
    pub fn new() -> Self {
        Self {
            arr: [0; RING_BUFFER_SIZE],
            head: 0,
            tail: 0,
            status: RingBufferStatus::Empty,
            write_end: None,
        }
    }

    pub fn set_write_end(&mut self, write_end: &Arc<Pipe>) {
        self.write_end = Some(Arc::downgrade(write_end));
    }
    pub fn write_byte(&mut self, byte: u8) {
        self.status = RingBufferStatus::Normal;
        self.arr[self.tail] = byte;
        self.tail = (self.tail + 1) % RING_BUFFER_SIZE;
        if self.tail == self.head {
            self.status = RingBufferStatus::Full;
        }
    }

    // [destinyfvcker] 这个方法可以从管道之中读取一个字节，
    // 但是注意在调用它之前需要确保管道缓冲区不是空的，在这个方法之中并没有对管道缓冲区为空的情况进行任何的处理
    pub fn read_byte(&mut self) -> u8 {
        self.status = RingBufferStatus::Normal;
        let c = self.arr[self.head];
        // [destinyfvcker] 仅仅通过比较队头和队尾是否相同并不能确定循环队列是否为空，
        // 因为它既有可能表示队列为空，也有可能表示队列已满，因此我们要在 read_byte 的同时进行状态更新
        self.head = (self.head + 1) % RING_BUFFER_SIZE;
        if self.head == self.tail {
            self.status = RingBufferStatus::Empty;
        }
        c
    }

    // [destinyfvcker] 这个方法可以计算管道之中还有多少个字符可以读取。
    // 我们首先需要判断队列是否为空，如果队列为空的话直接就返回 0，否则根据队头和队尾的相对位置进行计算
    pub fn available_read(&self) -> usize {
        if self.status == RingBufferStatus::Empty {
            0
        } else if self.tail > self.head {
            // 假如 head 在 tail 的前面，这是一种一般情况，直接减
            self.tail - self.head
        } else {
            self.tail + RING_BUFFER_SIZE - self.head // 否则特殊处理
        }
    }
    pub fn available_write(&self) -> usize {
        if self.status == RingBufferStatus::Full {
            0
        } else {
            RING_BUFFER_SIZE - self.available_read()
        }
    }
    // [destinyfvcker] 这个方法可以判断是否管道的所有写端都已经被关闭了
    // 通过尝试将管道之中保存的写端的弱引用计数升级为强引用计数来实现的，
    // 如果升级失败的话，就说明管道写端的强引用计数为 0，这也就意味着管道的所有写端都被关闭了，
    // 从而管道之中的数据不会再得到补充，等到管道中仅剩的数据被读取完毕之后，管道就可以被销毁了
    pub fn all_write_ends_closed(&self) -> bool {
        self.write_end.as_ref().unwrap().upgrade().is_none()
    }
}

/// Return (read_end, write_end)
pub fn make_pipe() -> (Arc<Pipe>, Arc<Pipe>) {
    let buffer = Arc::new(unsafe { UPSafeCell::new(PipeRingBuffer::new()) });
    let read_end = Arc::new(Pipe::read_end_with_buffer(buffer.clone()));
    let write_end = Arc::new(Pipe::write_end_with_buffer(buffer.clone()));
    buffer.exclusive_access().set_write_end(&write_end);
    (read_end, write_end)
}

impl File for Pipe {
    fn readable(&self) -> bool {
        self.readable
    }
    fn writable(&self) -> bool {
        self.writable
    }

    // [destinyfvcker] 将 Pipe 看成一个文件进行读取的方法
    fn read(&self, buf: UserBuffer) -> usize {
        assert!(self.readable());
        let want_to_read = buf.len();
        // [destinyfvcker] 将传入应用缓冲区 buf 转化为一个能够逐字节对于缓冲区进行访问的迭代器
        let mut buf_iter = buf.into_iter();
        let mut already_read = 0usize;

        // [destinyfvcker] read 的语意原本就是从文件之中最多读取应用缓冲区大小那么多的字符。
        // 但是这可能超出了循环队列的大小，或者由于尚未有进程从管道的写端写入足够的字符因此我们需要将整个读取的过程放在一个循环之中，
        // 当循环队列之中不存在足够字符的时候暂时进行任务切换，等待循环队列之中的字符得到补充之后再继续读取
        loop {
            let mut ring_buffer = self.buffer.exclusive_access();
            let loop_read = ring_buffer.available_read();
            if loop_read == 0 {
                if ring_buffer.all_write_ends_closed() {
                    return already_read;
                }
                // 在调用之前，我们需要手动释放管道自身的锁，因为切换任务时候的 __switch 不是一个正常的函数调用
                drop(ring_buffer);
                suspend_current_and_run_next();
                continue;
            }
            for _ in 0..loop_read {
                // [destinyfvcker] 对于这个自己迭代器，每次调用 next 就可以按顺序取出用于访问缓冲区的一个字节的裸指针
                if let Some(byte_ref) = buf_iter.next() {
                    unsafe {
                        *byte_ref = ring_buffer.read_byte();
                    }
                    already_read += 1;
                    if already_read == want_to_read {
                        return want_to_read;
                    }
                } else {
                    return already_read;
                }
            }
        }
    }
    fn write(&self, buf: UserBuffer) -> usize {
        assert!(self.writable());
        let want_to_write = buf.len();
        let mut buf_iter = buf.into_iter();
        let mut already_write = 0usize;
        loop {
            let mut ring_buffer = self.buffer.exclusive_access();
            let loop_write = ring_buffer.available_write();
            if loop_write == 0 {
                drop(ring_buffer);
                suspend_current_and_run_next();
                continue;
            }
            // write at most loop_write bytes
            for _ in 0..loop_write {
                if let Some(byte_ref) = buf_iter.next() {
                    ring_buffer.write_byte(unsafe { *byte_ref });
                    already_write += 1;
                    if already_write == want_to_write {
                        return want_to_write;
                    }
                } else {
                    return already_write;
                }
            }
        }
    }
}
