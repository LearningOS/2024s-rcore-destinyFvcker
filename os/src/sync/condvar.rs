//! Conditian variable

use crate::sync::{Mutex, UPSafeCell};
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc};

/// Condition variable structure
pub struct Condvar {
    /// Condition variable inner
    pub inner: UPSafeCell<CondvarInner>,
}

pub struct CondvarInner {
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Condvar {
    /// Create a new condition variable
    pub fn new() -> Self {
        trace!("kernel: Condvar::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(CondvarInner {
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    /// Signal a task waiting on the condition variable
    pub fn signal(&self) {
        let mut inner = self.inner.exclusive_access();
        if let Some(task) = inner.wait_queue.pop_front() {
            wakeup_task(task);
        }
    }

    /// blocking current task, let it wait on the condition variable
    pub fn wait(&self, mutex: Arc<dyn Mutex>) {
        trace!("kernel: Condvar::wait_with_mutex");
        mutex.unlock();
        let mut inner = self.inner.exclusive_access();
        inner.wait_queue.push_back(current_task().unwrap());
        drop(inner);
        block_current_and_run_next();
        mutex.lock();
    }
}

// [destinyfvcker] 现在我们需要解决的一类同步互斥问题包括：
// 1. 首先，线程抓紧共享一些资源，于是必须使用互斥锁来对这些资源进行保护，以确保同一时间最多只有一个线程在资源的临界区内；
// 2. 其次，希望能够高效并灵活地支持线程间的条件同步，这应该基于阻塞机制来实现：线程在条件未满足的时候将自身阻塞，之后另一个线程执行到了某个阶段之后，发现条件已经满足，于是将之前阻塞的线程唤醒。
// 这并不是一种通用的解决方案，而是有局限性的：
// 1. 信号量本质上是一个整数，它不足以描述所有类型的等待条件/事件；
// 2. 在使用信号量的时候需要特别小心：up 和 down 配对使用，在和互斥锁组合使用的时候需要注意操作顺序，不然容易死锁.

// [destinyfvcker] 这里要解决两个关键问题：
// 1. 如何等待一个条件？
// 2. 在条件为真时应该如何向等待线程发出信号？
// 计算机科学家给出了管程（Monitor）和条件变量（Condition Variables）这种巧妙的方法
//
// [destinyfvcker] 管程：
// =互斥访问= 任一时刻只能有一个活跃线程调用管程之中的过程
// =条件同步= 基于阻塞等待
// 一个管程之中可以有多个不谈的条件变量，每一个条件比那辆代表多线程并发执行之中需要等待的一种特定条件，并保存所有阻塞等待该条件的线程
// <注意条件变量和管程过程自带的互斥锁是如何交互的>
// 经验告诉我们不要在持有锁的情况下陷入阻塞，因此在陷入阻塞状态之前必须先释放锁；
// 当被阻塞的线程被其他线程使用 signal 操作唤醒之后，需要重新获取到锁才能继续执行，不然就无法保证管程过程的互斥访问。
//
// [destinyfvcker] 条件变量的基本思路：
// <等待机制>：由于线程在调用管程中的某个过程时，发现某个条件不满足，那就无法继续运行而被阻塞
// wait 操作：必须持有锁才能调用：功能顺序分成下面多个阶段，由编程语言来保证原子性
// 1. 释放锁；
// 2. 阻塞当前线程；
// 3. 当前线程被唤醒之后，重新获取到锁；
// 4. wait 返回，当前线程成功向下执行；
// <唤醒机制>：另外一个线程可以在调用管程的过程之中，把某个条件设置为真，并且还需要有一种机制，来及时唤醒等待条件为真的阻塞线程
// 由于互斥锁的存在，signal 操作也不只是简单的唤醒操作。当线程 T1 在执行过程（位于管程的过程中）中发现某条件满足准备唤醒线程 T2 的时候，
// 如果直接让 T2 继续执行（也位于管程过程之中），就会违背管程过程的互斥访问要求。
// 所以问题的关键就是，在 T1 唤醒 T2 的时候，T1 如何正确处理它正持有的锁。
//
// <具体来说，根据相关线程的优先级顺序，唤醒操作有这几种语义>
// ----------------
// ==> Hoare 语意： 优先级 T2>T1>其他线程。也就是说，当 T1 发现条件发现条件满足之后，立即通过 signal 唤醒 T2 并将锁交给 T2，这样 T2 就可以立即继续执行，
// 而 T1 则暂停执行并进入一个紧急等待队列，当 T2 退出管程过程之后会将锁交回给紧急等待队列之中的 T1，从而 T1 可以继续执行
// ----------------
// ==> Hansen 语义：优先级 T1>T2>其他线程。即 T1 发现条件满足之后，先继续执行，直到退出管程之前再使用 signal 唤醒并将锁转交给 T2，
// 于是 T2 可以继续执行，注意在 Hansen 语义下，signal 必须位于管程过程末尾
// ----------------
// ==> Mesa 语义：优先级 T1>T2=其他线程。也就是说 T1 发现条件满足之后，就可以使用 signal 唤醒 T2，但是并不会将锁转交给 T2。
// 这意味着在 T1 退出管程过程释放锁之后，T2 还需要和其他线程竞争，直到抢到锁之后才能继续执行
// ----------------
// 其中 Hoare 和 Hansen 语意在 T2 被唤醒之后其等待的条件一定是成立的（T1 和 T2 中间没有其他线程），因此没有必要重复检查条件是否成立就可以向下执行
// 但是在 Mesa 语义下，应为中间可能存在其他线程，wait 操作返回的时候不见得线程等待的条件一定成立，有必要重复检查确认之后再继续执行
//
// 所以就产生了一个问题：在条件等待的时候是使用 if/else 还是 while?
// if (!condition) {
//    wait()
// } else {
//    ...
// }
// 这种方法假定了 wait 返回之后条件一定已经成立，于是不再做检车直接向下执行。
// 或：
// while (!condition) {
//    wait()
// }
// 重复检查直到条件成立为止
//
// 这里使用的基于 Mesa 语意的沟通机制：唤醒线程在发出行唤醒操作之后继续运行，并且只有它退出管程之后，才允许等待的线程开始运行，现在唤醒线程的执行位置还在管程之中
// 这种沟通机制的具体实现就是条件变量对应的操作：wait 和 signal。
// 线程使用条件变量来等待一个条件变成真。条件变量其实是一个线程等待队列，当条件不满足时，
// 线程通过执行条件变量的 wait 操作就可以把自己加入到等待队列中，睡眠等待（waiting）该条件。
// 另外某个线程，当它改变条件为真后，就可以通过条件变量的 signal 操作来唤醒一个或者多个等待的线程（通过在该条件上发信号），让它们继续执行。
