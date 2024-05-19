//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::mm::{is_pysical_mm_enough, MapPermission, VirtAddr, VirtPageNum};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    // [destinyfvcker] you need to call run_tasks function to actually do this
    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    // [destinyfvcker] how to set the field of Processcor?
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        // [destinyfvcker] the "_" act as a placeholder,
        // allowing the complier to infer the type based on context
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        // [destinyfvcker] Takes the value out of the option, leaving a None in its place.
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();

        // [destinyfvcker] the method fetch_task is implied in manger.rs,
        // it will pop a Arc point to TaskControlBlock from ready array.
        //
        // [destinyfvcker] the mamager.rs moduele actually implied the RR scheduling algorithm
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();

            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            // release coming task_inner manually
            drop(task_inner);

            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);

            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
    // let token = task.inner_exclusive_access().get_user_token();
    // token
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

/// Get system call count of current running task from TASK_MANAGER
pub fn get_system_call_count(dst: &mut [u32]) {
    let current_task = current_task().unwrap();
    current_task.get_system_call_count(dst);
}

/// Get time interval of the last system call
pub fn get_time_interval() -> usize {
    let current_task = current_task().unwrap();
    current_task.calculate_time_interval()
}

/// Update system call count of current running task
pub fn update_system_call_count(syscall_id: usize) {
    let current_task = current_task().unwrap();
    current_task.update_system_call_count(syscall_id);
}

/// Update The record time of the last system call
pub fn update_last_syscall_time() {
    let current_task = current_task().unwrap();
    current_task.update_last_syscall_time();
}

/// mmap systemcall implication
pub fn mmap(start: usize, len: usize, port: usize) -> isize {
    let current_task = current_task().unwrap();

    let start_vpa = VirtAddr::from(start);
    let end_vpa = VirtAddr::from(start + len);

    let start_vpn: VirtPageNum = start_vpa.floor();
    let end_vpn: VirtPageNum = end_vpa.ceil();

    if !start_vpa.aligned() // start 没有按照页大小对齐
        || port & !0x7 != 0 // port 其余位必须为 0
        || port & 0x7 == 0 // 无意义内存
        || current_task.is_conflict(start_vpn, end_vpn) // 在请求地址范围之中存在已经被映射的页
        || !is_pysical_mm_enough(end_vpn.0 - start_vpn.0)
    // 检查物理内存是否足够进行分配
    {
        return -1;
    }

    let mut map_perm = MapPermission::U;
    if port & 0x1 == 0x1 {
        map_perm |= MapPermission::R;
    }
    if port & 0x2 == 0x2 {
        map_perm |= MapPermission::W;
    }
    if port & 0x4 == 0x4 {
        map_perm |= MapPermission::X;
    }
    current_task.alloc_mm(start_vpa, end_vpa, map_perm);
    0
}

/// munmap systemcall implication
pub fn munmap(start: usize, len: usize) -> isize {
    let current_task = current_task().unwrap();

    let start_vpa = VirtAddr::from(start);
    let end_vpa = VirtAddr::from(start + len);

    let start_vpn: VirtPageNum = start_vpa.floor();
    let end_vpn: VirtPageNum = end_vpa.ceil();

    let map_result = current_task.is_mapped(start_vpn, end_vpn);
    if !start_vpa.aligned() || map_result < 0 {
        return -1;
    }

    current_task.dealloc_mm(start_vpn, end_vpn, map_result);

    0
}

/// set priority of current running process
pub fn set_proc_prio(prio: usize) {
    let current_task = current_task().unwrap();

    let mut inner = current_task.inner_exclusive_access();
    inner.proc_prio = prio;
}
