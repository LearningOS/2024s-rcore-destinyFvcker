//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
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

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}

// impl TaskManager {
//     /// Run the first task in task list.
//     ///
//     /// Generally, the first task in task list is an idle task (we call it zero process later).
//     /// But in ch4, we load apps statically, so the first task is a real app.
//     fn run_first_task(&self) -> ! {
//         let mut inner = self.inner.exclusive_access();
//         let next_task = &mut inner.tasks[0];
//         next_task.task_status = TaskStatus::Running;
//         let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;

//         // set start time
//         next_task.update_start_time();

//         drop(inner);
//         let mut _unused = TaskContext::zero_init();
//         // before this, we should drop local variables that must be dropped manually
//         unsafe {
//             __switch(&mut _unused as *mut _, next_task_cx_ptr);
//         }
//         panic!("unreachable in run_first_task!");
//     }

//     /// Change the status of current `Running` task into `Ready`.
//     fn mark_current_suspended(&self) {
//         let mut inner = self.inner.exclusive_access();
//         let cur = inner.current_task;
//         inner.tasks[cur].task_status = TaskStatus::Ready;
//     }

//     /// Change the status of current `Running` task into `Exited`.
//     fn mark_current_exited(&self) {
//         let mut inner = self.inner.exclusive_access();
//         let cur = inner.current_task;
//         inner.tasks[cur].task_status = TaskStatus::Exited;
//     }

//     /// Find next task to run and return task id.
//     ///
//     /// In this case, we only return the first `Ready` task in task list.
//     fn find_next_task(&self) -> Option<usize> {
//         let inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         (current + 1..current + self.num_app + 1)
//             .map(|id| id % self.num_app)
//             .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
//     }

//     /// Get the current 'Running' task's token.
//     fn get_current_token(&self) -> usize {
//         let inner = self.inner.exclusive_access();
//         inner.tasks[inner.current_task].get_user_token()
//     }

//     /// Get the current 'Running' task's trap contexts.
//     fn get_current_trap_cx(&self) -> &'static mut TrapContext {
//         let inner = self.inner.exclusive_access();
//         inner.tasks[inner.current_task].get_trap_cx()
//     }

//     /// Change the current 'Running' task's program break
//     pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
//         let mut inner = self.inner.exclusive_access();
//         let cur = inner.current_task;
//         inner.tasks[cur].change_program_brk(size)
//     }

//     /// Switch current `Running` task to the task we have found,
//     /// or there is no `Ready` task and we can exit with all applications completed
//     fn run_next_task(&self) {
//         if let Some(next) = self.find_next_task() {
//             let mut inner = self.inner.exclusive_access();
//             let current = inner.current_task;
//             inner.tasks[next].task_status = TaskStatus::Running;
//             inner.current_task = next;
//             let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
//             let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;

//             // judge that if the task is run for the first time
//             let task = &mut inner.tasks[next];
//             if task.start_time == TimeVal::default() {
//                 task.start_time.update();
//             }

//             drop(inner);
//             // before this, we should drop local variables that must be dropped manually
//             unsafe {
//                 __switch(current_task_cx_ptr, next_task_cx_ptr);
//             }
//             // go back to user mode
//         } else {
//             panic!("All applications completed!");
//         }
//     }

//     fn get_system_call_count(&self, dst: &mut [u32]) {
//         let inner = self.inner.exclusive_access();
//         let curruent = inner.current_task;

//         inner.tasks[curruent].get_syscall_times_copy(dst)
//     }

//     fn update_system_call_count(&self, syscall_id: usize) {
//         let mut inner = self.inner.exclusive_access();
//         let curruent = inner.current_task;

//         inner.tasks[curruent].syscall_times[syscall_id] += 1;
//     }

//     fn update_last_syscall_time(&self) {
//         let mut inner = self.inner.exclusive_access();
//         let current = inner.current_task;

//         let task = &mut inner.tasks[current];
//         task.last_syscall_time.update();
//     }

//     fn calculate_time_interval(&self) -> usize {
//         let inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         let task = &inner.tasks[current];

//         task.last_syscall_time.as_ms() - task.start_time.as_ms()
//     }

//     fn is_conflict(&self, start: VirtPageNum, end: VirtPageNum) -> bool {
//         let inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         let task = &inner.tasks[current];

//         task.memory_set.is_conflict(start, end)
//     }

//     fn is_vmm_mapped(&self, start: VirtPageNum, end: VirtPageNum) -> isize {
//         let inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         let task = &inner.tasks[current];

//         task.memory_set.is_vmm_mapped(start, end)
//     }

//     fn alloc_mm(&self, start: VirtAddr, end: VirtAddr, port: MapPermission) {
//         let mut inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         let task = &mut inner.tasks[current];

//         task.memory_set.insert_framed_area(start, end, port);
//     }

//     fn dealloc_mm(&self, start_va: VirtPageNum, end_va: VirtPageNum, is_cross: isize) {
//         let mut inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         let task = &mut inner.tasks[current];

//         task.memory_set.free(start_va, end_va, is_cross as usize);
//     }
// }

// /// Run the first task in task list.
// pub fn run_first_task() {
//     TASK_MANAGER.run_first_task();
// }

// /// Switch current `Running` task to the task we have found,
// /// or there is no `Ready` task and we can exit with all applications completed
// fn run_next_task() {
//     TASK_MANAGER.run_next_task();
// }

// /// Change the status of current `Running` task into `Ready`.
// fn mark_current_suspended() {
//     TASK_MANAGER.mark_current_suspended();
// }

// /// Change the status of current `Running` task into `Exited`.
// fn mark_current_exited() {
//     TASK_MANAGER.mark_current_exited();
// }

// /// Suspend the current 'Running' task and run the next task in task list.
// pub fn suspend_current_and_run_next() {
//     mark_current_suspended();
//     run_next_task();
// }

// /// Exit the current 'Running' task and run the next task in task list.
// pub fn exit_current_and_run_next() {
//     mark_current_exited();
//     run_next_task();
// }

// /// Get the current 'Running' task's token.
// pub fn current_user_token() -> usize {
//     TASK_MANAGER.get_current_token()
// }

// /// Get the current 'Running' task's trap contexts.
// pub fn current_trap_cx() -> &'static mut TrapContext {
//     TASK_MANAGER.get_current_trap_cx()
// }

// /// Change the current 'Running' task's program break
// pub fn change_program_brk(size: i32) -> Option<usize> {
//     TASK_MANAGER.change_current_program_brk(size)
// }

// /// Get system call count of current running task from TASK_MANAGER
// pub fn get_system_call_count(dst: &mut [u32]) {
//     TASK_MANAGER.get_system_call_count(dst);
// }
// /// Get time interval of the last system call
// pub fn get_time_interval() -> usize {
//     TASK_MANAGER.calculate_time_interval()
// }

// /// Update system call count of current running task
// pub fn update_system_call_count(syscall_id: usize) {
//     TASK_MANAGER.update_system_call_count(syscall_id);
// }

// /// Update The record time of the last system call
// pub fn update_last_syscall_time() {
//     TASK_MANAGER.update_last_syscall_time();
// }

// /// mmap systemcall implication
// pub fn mmap(start: usize, len: usize, port: usize) -> isize {
//     let start_vpa = VirtAddr::from(start);
//     let end_vpa = VirtAddr::from(start + len);

//     let start_vpn: VirtPageNum = start_vpa.floor();
//     let end_vpn: VirtPageNum = end_vpa.ceil();

//     if !start_vpa.aligned() // start 没有按照页大小对齐
//         || port & !0x7 != 0 // port 其余位必须为 0
//         || port & 0x7 == 0 // 无意义内存
//         || TASK_MANAGER.is_conflict(start_vpn, end_vpn) // 在请求地址范围之中存在已经被映射的页
//         || !is_pysical_mm_enough(end_vpn.0 - start_vpn.0)
//     // 检查物理内存是否足够进行分配
//     {
//         return -1;
//     }

//     let mut map_perm = MapPermission::U;
//     if port & 0x1 == 0x1 {
//         map_perm |= MapPermission::R;
//     }
//     if port & 0x2 == 0x2 {
//         map_perm |= MapPermission::W;
//     }
//     if port & 0x4 == 0x4 {
//         map_perm |= MapPermission::X;
//     }

//     TASK_MANAGER.alloc_mm(start_vpa, end_vpa, map_perm);

//     0
// }

// /// munmap systemcall implication
// pub fn munmap(start: usize, len: usize) -> isize {
//     let start_vpa = VirtAddr::from(start);
//     let end_vpa = VirtAddr::from(start + len);

//     let start_vpn: VirtPageNum = start_vpa.floor();
//     let end_vpn: VirtPageNum = end_vpa.ceil();

//     let map_result = TASK_MANAGER.is_vmm_mapped(start_vpn, end_vpn);
//     if !start_vpa.aligned() || map_result < 0 {
//         return -1;
//     }

//     TASK_MANAGER.dealloc_mm(start_vpn, end_vpn, map_result);

//     0
// }
