//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc};

/// semaphore Id
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemId(pub usize);

/// semaphore structure
pub struct Semaphore {
    /// semaphore id
    pub sem_id: SemId,
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(sem_id: usize, res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            sem_id: SemId(sem_id),
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

        let current_task = current_task().unwrap();
        let mut task_inner = current_task.inner_exclusive_access();

        if let Some((index, alloc_count)) = task_inner
            .allocation
            .iter_mut()
            .enumerate()
            .find(|(_, (sem_id, _))| *sem_id == self.sem_id)
        {
            alloc_count.1 -= 1;
            if alloc_count.1 <= 0 {
                task_inner.allocation.remove(index);
            }
        }

        drop(task_inner);
        drop(current_task);

        // [destinyfvcker] 关于为什么这里是看到 inner.count 小于 0 才触发，
        // 因为只有 inner.count <= 0 才会有线程阻塞在这个信号量之中.
        // 当然也可以使用对应等待队列的长度来进行判断.
        if inner.count <= 0 {
            // 这里实际上是对对应线程的 need 向量进行一个管理，就是将对应的 need 值 -1，然后不要忘记了将对应的 allocation 值加一
            if let Some(task) = inner.wait_queue.pop_front() {
                let mut task_inner = task.inner_exclusive_access();

                if let Some((index, sem_count)) = task_inner
                    .need
                    .iter_mut()
                    .enumerate()
                    .find(|(_, (sem_id, _))| *sem_id == self.sem_id)
                {
                    sem_count.1 -= 1;
                    if sem_count.1 <= 0 {
                        task_inner.need.remove(index);
                    }
                } else {
                    panic!("[destinyfvcker] ======== there should be a need item be registed! ========");
                }

                if let Some((_, alloc_count)) = task_inner
                    .allocation
                    .iter_mut()
                    .find(|(sem_id, _)| *sem_id == self.sem_id)
                {
                    *alloc_count += 1;
                } else {
                    task_inner.allocation.push((self.sem_id, 1));
                }

                drop(task_inner);
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    pub fn down(&self) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;

        // let current_task = current_task().unwrap();
        // let mut task_inner = current_task.inner_exclusive_access();

        // drop(task_inner);
        // drop(current_task);
        let current_task = current_task().unwrap();
        let mut task_inner = current_task.inner_exclusive_access();

        if inner.count < 0 {
            if let Some(sem_count) = task_inner
                .need
                .iter_mut()
                .find(|(sem_id, _)| *sem_id == self.sem_id)
            {
                sem_count.1 += 1;
            } else {
                task_inner.need.push((self.sem_id.clone(), 1))
            }

            drop(task_inner);
            inner.wait_queue.push_back(current_task);
            drop(inner);
            block_current_and_run_next();
        } else {
            if let Some(alloc_count) = task_inner
                .allocation
                .iter_mut()
                .find(|(sem_id, _)| *sem_id == self.sem_id)
            {
                alloc_count.1 += 1;
            } else {
                task_inner.allocation.push((self.sem_id.clone(), 1));
            }
        }
    }
    // [destinyfvcker?] 在执行上面这个方法的时候我一直有一个疑问：
    // 就是说如果一个线程需要请求两个资源的时候会不会导致，这个线程的 Arc 指针被添加到两个 semaphore 的等待队列之中
    // 然后在这两个 semaphore up 的时候，就会 add 两次 TaskControlBlock 到调度队列之中，这看起来实在像一个致命错误！
    //
    // 但是实际上并不会有这种情况的发生，在对 semaphore 进行 down 操作的时候，如果在第一个 semaphore 就被 block 的话
    // 就会将这个线程当前的控制流打断，然后它就不再会被执行了，直到它被另外一个线程的 up 方法唤醒
    // 然后它才会继续去获取下一个 semaphore，根本不会出现同时出现在两个 semaphore 的等待队列之中的情况
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
