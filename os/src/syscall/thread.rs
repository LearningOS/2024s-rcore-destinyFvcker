use crate::{
    mm::kernel_token,
    task::{add_task, current_task, TaskControlBlock},
    trap::{trap_handler, TrapContext},
};
use alloc::sync::Arc;

// [destinyfvcker] 当进程调用这个系统调用之后，内核会在这个进程内部创建一个新的线程，
// 这个线程能够访问到进程锁拥有的代码段，堆和其他数据段
// 但是内核会给这个新线程分配一个它专有的用户态栈，这样每一个线程才能相对独立地被调度和执行
//
// 另外由于用户态进程和内核之间有各自独立的页表，所以二者之间需要有一个跳板页 TRAMPOLINE 来处理用户态切换到内核态的地址空间平滑转换的事务
// 所以当出现线程之后，在线程的每一个线程也需要有一个独立的跳板页面 TRAMPOLINE 来完成同样的事务
//
// 相比于创建进程的 fork 系统调用：
// 1. 创建线程不需要要建立新的地址空间（这是最大的不同点）
// 2. 属于同一进程之中的线程之间没有父子关系

// [destinyfvcker] 这里重点是需要了解创建线程控制块，
// 在线程控制块之中初始化各个成员变量，建立好进程和线程的关系等等；
//
// 这里列出支持线程正确运行所需要的重要的执行环境要素：
// - 线程的用户态栈：确保在用户态的线程能正常执行函数调用；
// - 线程的内核态栈：确保线程陷入内核后能够正常执行函数调用；
// - 线程的跳板页：确保线程能正确的进行用户态<->内核态切换；
// - 线程上下文：也就是线程用到的寄存器信息，用于线程切换。

/// thread create syscall
/// 功能：当前进程创建一个新的线程
/// 参数：entry 表示线程的入口函数地址
/// 参数：arg 表示线程的一个参数         
pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_thread_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref() // [destinyfvcker] 关于为什么这里要使用 as_ref，因为 res 是具有所有权的，直接 unwrap 的话会导致所有权被移动出去
            .unwrap()
            .tid
    );
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    // create a new thread
    let new_task = Arc::new(TaskControlBlock::new(
        Arc::clone(&process),
        task.inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .ustack_base,
        true,
    ));
    // add new task to scheduler
    add_task(Arc::clone(&new_task));
    let new_task_inner = new_task.inner_exclusive_access();
    let new_task_res = new_task_inner.res.as_ref().unwrap();
    let new_task_tid = new_task_res.tid;
    let mut process_inner = process.inner_exclusive_access();
    // add new thread to current process
    let tasks = &mut process_inner.tasks;
    while tasks.len() < new_task_tid + 1 {
        tasks.push(None);
    }
    tasks[new_task_tid] = Some(Arc::clone(&new_task));
    let new_task_trap_cx = new_task_inner.get_trap_cx();

    // [destinyfvcker] 初始化位于这个线程用户态地址空间之中的 Trap 上下文：
    // 这是线程的函数入口点和用户栈，使得第一次进入用户态的时候能从线程起始位置开始正确执行；
    // 设置好内核栈和陷入韩苏指针 trap_handler，保证在 Trap 的时候用户态的线程能正确进入内核态
    *new_task_trap_cx = TrapContext::app_init_context(
        entry,
        new_task_res.ustack_top(),
        kernel_token(),
        new_task.kstack.get_top(),
        trap_handler as usize,
    );
    (*new_task_trap_cx).x[10] = arg;
    new_task_tid as isize
}

pub fn sys_gettid() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_gettid",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid as isize
}

// [destinyfvcker] 当一个线程执行完代表它的功能之后，会通过 exit 系统调用退出。
// 内核在收到线程发出的 exit 系统调用之后，会回收线程占用的部分资源，
// 这部分资源就是用户态用到的资源，比如说用户态的栈，用于系统调用和异常处理的跳板页等等。
//
// 而该线程的内核态用到的内核资源，比如内核栈等等，需要通过进程/主线程调用 waittid 来回收，
// 这样整个线程才能被彻底销毁
/// wait for a thread to exit syscall
///
/// thread does not exist, return -1
/// thread has not exited yet, return -2
/// otherwise, return thread's exit code
pub fn sys_waittid(tid: usize) -> i32 {
    trace!(
        "kernel:pid[{}] tid[{}] sys_waittid",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let task_inner = task.inner_exclusive_access();
    let mut process_inner = process.inner_exclusive_access();
    // a thread cannot wait for itself
    if task_inner.res.as_ref().unwrap().tid == tid {
        return -1;
    }
    let mut exit_code: Option<i32> = None;
    let waited_task = process_inner.tasks[tid].as_ref();
    if let Some(waited_task) = waited_task {
        if let Some(waited_exit_code) = waited_task.inner_exclusive_access().exit_code {
            exit_code = Some(waited_exit_code);
        }
    } else {
        // waited thread does not exist
        return -1;
    }
    if let Some(exit_code) = exit_code {
        // dealloc the exited thread
        process_inner.tasks[tid] = None;
        exit_code
    } else {
        // waited thread has not exited
        -2
    }
}

// [destinyfvcker] 进程相关的系统调用
// 在引入了线程机制后，进程相关的重要系统调用：fork、exec、waitpid 虽然在接口上没有变化，
// 但是它要完成的功能上需要有一定的扩展。
// 首先，需要注意到把以前进程中于处理器执行相关的部分拆分到线程之中。
// 这样，在通过 fork 创建进程起始也意味着要单独建立一个主线程来使用处理器，并为以后创建新的线程建立相应的线程控制块向量。
//
// exec 和 waitpid 这两个系统调用要做的改动比较小
