//! Mutex (spin-like and blocking(sleep))

use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_task, wakeup_task};
use alloc::{collections::VecDeque, sync::Arc};

/// Mutex trait
pub trait Mutex: Sync + Send {
    /// Lock the mutex
    fn lock(&self);
    /// Unlock the mutex
    fn unlock(&self);
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self) {
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        trace!("kernel: MutexSpin::unlock");
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

// [destinyfvcker] 这是会实现 Mutex trait 的内核数据结构，它就是我们提到的互斥资源，也就是互斥锁。
/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

/// [destinyfvcker] 操作系统需要显式地施加某种控制，来确定当一个线程释放锁时，等待的线程谁能抢到锁
pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        trace!("kernel: MutexBlocking::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self) {
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();

        // [destinyfvcker] 如果互斥锁 mutex 已经被其他线程获取了，
        // 就将当前线程放入等待队列之中，并调度其他线程执行
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            // [destinyfvcker] 如果互斥锁还没有被获取，那么当前线程就会获取给互斥锁，并返回系统调用
            mutex_inner.locked = true;
        }
    }

    /// unlock the blocking mutex
    fn unlock(&self) {
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);

        // [destinyfvcker] 如果有等待的线程，就唤醒等待最久的那个线程，相当于将锁的所有权移交给这个线程
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            wakeup_task(waking_task);
        } else {
            // [destinyfvcker] 如果没有线程等待就释放锁
            mutex_inner.locked = false;
        }
    }
}

// [destinyfvcker] 在操作系统中，需要设计实现三个核心成员变量
// 其中互斥锁的成员变量就有两个：
// 1. 表示是否锁上的 locked (在 MutexBlockingInner 中定义)
// 2. 管理等待线程的等待队列 wait_queue（在 MutexBlockingInner 之中定义）
//
// 然后就是进程的成员变量：锁向量 mutex_list
