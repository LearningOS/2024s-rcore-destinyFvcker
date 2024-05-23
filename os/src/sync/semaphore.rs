//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    pub fn down(&self) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
    }
}

// [destinyfvcker] 通过互斥锁，可以让线程在临界区执行时，独占临界资源。
// 当我们需要更灵活的互斥访问或同步操作方式，如提供了最多只允许 N 个线程访问临界资源的情况，让某个线程等待另外一个线程执行完毕后再继续执行的同步过程等，互斥锁这种方式就有点力不从心了。
//
// 信号量是对互斥锁的一种巧妙的扩展，互斥锁的初始值一般设置为 1 的整型变量，表示临界区还没有被某一个线程占用，0 就表示已经被占用了
// 信号量的初始值可以设置为 N 的整数变量，如果 N > 0，就表示最多可以有 N 个线程进入临界区执行，
// 如果 N 小于等于 0，就表示不能有线程进入临界区了，必须在后续操作之中让信号量的值加 1，才能唤醒某一个等待的线程
//
// Dijkstra 对信号量设计了两种操作：P（Proberen（荷兰语），尝试）操作和 V（Verhogen（荷兰语），增加）操作。
// P 操作：
// P 操作是检查信号量的值是否大于 0，若该值大于 0，则将其值减 1 并继续（表示可以进入临界区了）；若该值为 0，则线程将睡眠。注意，此时 P 操作还未结束。而且由于信号量本身是一种临界资源（可回想一下上一节的锁， 其实也是一种临界资源），所以在 P 操作中，检查/修改信号量值以及可能发生的睡眠这一系列操作， 是一个不可分割的原子操作过程。通过原子操作才能保证，一旦 P 操作开始，则在该操作完成或阻塞睡眠之前， 其他线程均不允许访问该信号量。
// V 操作：
// V 操作会对信号量的值加 1 ，然后检查是否有一个或多个线程在该信号量上睡眠等待。如有， 则选择其中的一个线程唤醒并允许该线程继续完成它的 P 操作；如没有，则直接返回。注意，信号量的值加 1， 并可能唤醒一个线程的一系列操作同样也是不可分割的原子操作过程。不会有某个进程因执行 V 操作而阻塞。
//
// 有两种类型的信号量：
// 计数信号量（Counting Semaphore）/ 一般信号量（General Semaphore）
// 二值信号量（Binary semaphore）
