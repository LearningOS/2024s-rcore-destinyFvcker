//! Types related to task management
use super::TaskContext;
use crate::syscall::MAX_SYSCALL_NUM;
use crate::timer::*;

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// To record syscall times
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Task start time
    pub start_time: TimeVal,
    /// last syscall time
    pub last_syscall_time: TimeVal,
}

impl TaskControlBlock {
    /// Update the start time of a TaskControl Block
    pub fn update_start_time(&mut self) {
        self.start_time.update();
    }

    /// Update the last syscall time of a taskControl Block
    pub fn update_last_syscall_time(&mut self) {
        self.last_syscall_time.update();
    }

    /// Get a copy of syscall_times
    pub fn get_syscall_times_copy(&self) -> [u32; MAX_SYSCALL_NUM] {
        self.syscall_times.clone()
    }
}

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}

// pub struct TaskInfo {
//     status: TaskStatus,
//     syscall_times: [u32; MAX_SYSCALL_NUM],
//     time: usize,
// }
