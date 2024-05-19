//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the whole operating system.
//!
//! A single global instance of [`Processor`] called `PROCESSOR` monitors running
//! task(s) for each core.
//!
//! A single global instance of `PID_ALLOCATOR` allocates pid for user apps.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
mod context;
mod id;
mod manager;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
#[allow(rustdoc::private_intra_doc_links)]
mod task;

use crate::fs::{open_file, OpenFlags};
use alloc::sync::Arc;
pub use context::TaskContext;
use crate::{config::BIG_STRIDE, loader::get_app_data_by_name};
use lazy_static::*;
pub use manager::{fetch_task, TaskManager};
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;
pub use id::{kstack_alloc, pid_alloc, KernelStack, PidHandle};
pub use manager::add_task;
pub use processor::{
    current_task, current_trap_cx, current_user_token, get_system_call_count, get_time_interval,
    mmap, munmap, run_tasks, schedule, set_proc_prio, take_current_task, update_last_syscall_time,
    update_system_call_count, Processor,
};

// [destinyfvcker] included in "进程管理机制的设计和实现"
// [destinyfvcker] this function is called in os/src/syscall/process.rs for system call
// The most important usage of this function is in os/stc/trap/mod.rs，在时间片到的时候切换到下一个进程运行
/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = take_current_task().unwrap();

    // ---- access current TCB exclusively
    let mut task_inner = task.inner_exclusive_access();
    task_inner.proc_stride += BIG_STRIDE / task_inner.proc_prio;

    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // Change status to Ready
    task_inner.task_status = TaskStatus::Ready;
    // [destinyfvcker?] about why you should drop task_inner here?
    drop(task_inner);
    // ---- release current PCB

    // push back to ready queue.
    add_task(task);
    // jump to scheduling cycle
    schedule(task_cx_ptr);
}

/// pid of usertests app in make run TEST=1
pub const IDLE_PID: usize = 0;

// [destinyfvcker] 这个函数在 syscall/process.rs 进行主动退出、trap/mod.rs 之中因为用户程序异常崩溃由操作系统进行管理的被动式退出之中都有使用
// 带有一个退出码作为参数，这个退出码会在该函数之中写入当前进程的进程控制块
/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next(exit_code: i32) {
    // [destinyfvcker] 注意这里是直接取出，而不是得到一份拷贝，这是为了正确维护进程控制块的引用计数
    // 通过实验发现：无论是使用 Arc::clone/Rc::clone 方法还是直接对变量本身调用 clone 方法，都会增加引用计数（clone 方法接受一个不可变引用）
    // take from Processor
    let task = take_current_task().unwrap();

    let pid = task.getpid();
    if pid == IDLE_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        panic!("All applications completed!");
    }

    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    {
        // [destinyfvcker] INITPROC 是最开始的用户进程（SHELL）
        // 下面这一段代码的作用是将当前进程的所有子进程都挂载初始进程下面
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ release parent PCB

    inner.children.clear();

    // deallocate user space，[destinyfvcker] from MemorySet::
    // 这里只是将地址空间之中的逻辑段列表清空，这将导致应用地址空间之中的所有数据被存放在的物理页帧被回收，而用来存放页表的哪些物理页帧此时则不会被回收
    inner.memory_set.recycle_data_pages();

    // drop file descriptors
    inner.fd_table.clear();

    // [destinyfvcker] 关于为什么要在这里drop
    // 1. 首先在 schedule 之后这个函数就不会返回了（schedule 之中存在 swich 的调用）
    // 2. 其次可以参考 processor.rs 模块之中的 current_task 方法，这个方法用的地方很多，
    // 基本上都是在调用完之后就释放了，这里是最后两个强引用，在这个 drop 之后最后一个强引用在 parent process 手里捏着
    drop(inner);
    // **** release current PCB
    // drop task manually to maintain rc correctly
    drop(task);
    // we do not have to save task context [destinyfvcker] 所以这里就存了一个空的
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

// [destinyfvcker] included in "进程管理机制的设计和实现"
lazy_static! {
    /// Creation of initial process
    ///
    /// the name "initproc" may be changed to any other app name like "usertests",
    /// but we have user_shell, so we don't need to change it.
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let inode = open_file("ch6b_initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

// [destinyfvcker] this function is called in main.rs
///Add init process to the manager
pub fn add_initproc() {
    add_task(INITPROC.clone());
}
