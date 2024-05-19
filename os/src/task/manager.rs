//!Implementation of [`TaskManager`]
//! 实际上我认为这个模块之中的代码就是对任务（进程）调度算法的实现
use core::cmp::Reverse;

use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::BinaryHeap;
use alloc::sync::Arc;
use lazy_static::*;

struct TcbPtr(Arc<TaskControlBlock>);

///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    // [destinyfvcker] the reason to use Arc here is the task control block often
    // needs to be put in/taken out, and if the task control block itself is moved directly,
    // there will be a lot of data copy overhead.
    // [destingfvcker] And under some case, it can make out implementation more convinient
    ready_queue: BinaryHeap<Reverse<TcbPtr>>,
}

impl PartialEq for TcbPtr {
    fn eq(&self, other: &Self) -> bool {
        self.0.inner_exclusive_access().proc_stride == other.0.inner_exclusive_access().proc_stride
    }
}

impl Eq for TcbPtr {}

impl PartialOrd for TcbPtr {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        let self_stride = self.0.inner_exclusive_access().proc_stride;
        let other_stride = other.0.inner_exclusive_access().proc_stride;
        Some(self_stride.cmp(&other_stride))
    }
}

impl Ord for TcbPtr {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let self_stride = self.0.inner_exclusive_access().proc_stride;
        let other_stride = other.0.inner_exclusive_access().proc_stride;
        self_stride.cmp(&other_stride)
    }
}

// +====== [destinyfvcker] next impl block is ablout the implementation of RR algorithm ======+
/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: BinaryHeap::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        let tcb_ptr = TcbPtr(task);
        self.ready_queue.push(Reverse(tcb_ptr));
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        if let Some(Reverse(tcb_ptr)) = self.ready_queue.pop() {
            Some(tcb_ptr.0)
        } else {
            None
        }
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

// [destinyfvcker] 爆改 stride 调度算法
/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
