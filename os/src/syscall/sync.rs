use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, SemId, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

/// mutex create syscall
/// [destinyfvcker] blocking 控制的是锁的种类，假如 blocking 为 true，那么添加的就是一个可以睡眠的互斥锁
/// 假如 blocking 为 false，那么添加的就是一个不可睡眠的自旋锁
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };

    // [destinyfvcker] 下面这段代码的逻辑是：如果向量之中有空的元素，就在这个空元素位置放入之前创建的互斥锁
    // 如果向量满了，就在向量之中添加新的可睡眠的互斥锁
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}

// [destinyfvcker] 有了互斥锁，接下来就是实现 Mutex trait 的内核函数：
// 对应 SYSCALL_MUTEX_LOCK 系统调用的 sys_mutex_lock

/// mutex lock syscall
/// 这里操作系统执行的主要工作是：在锁已经被其他线程获取的情况下，把当前线程放到等待队列之中，并调度一个新线程执行
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());

    if process_inner.is_dl_det_enable && mutex.is_lock() {
        return -0xDEAD;
    }

    // [destinyfvcker] 注意在这里的 drop 函数需要获得对应参数的所有权
    // 所以要从内到外开始 drop
    drop(process_inner);
    drop(process);

    // [destinyfvcker] drop 完了再开始 lock，其实顺序没有什么关系
    // 这里实际上调用的是实现了 Mutex trait 的方法
    mutex.lock();
    0
}

/// mutex unlock syscall
/// [destinyfvcker] 对应 SYSCALL_MUTEX_UNLOCK 系统调用
///  操作系统的主要工作是：如果有等待在这个互斥锁上的线程，就需要唤醒最早等待的线程
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);

    // [destinyfvcker] 实现了 Mutex trait 的方法
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(id, res_count)));
        id
    } else {
        let id = process_inner.semaphore_list.len();
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(id, res_count))));
        id
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    // [destinyfvcker?] 临时变量的作用域问题
    let tid_now = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid_now
    );

    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let is_dl_det_enable = process_inner.is_dl_det_enable;

    // [destinyfvcker?] 关于这里的 as_ref() 方法的作用。
    // 好像并没有什么关系啊，这里也不涉及到所有权的转移啊？？？？
    //
    // 在经过一些实验之后我发现：实际上是这样的，Arc::clone 想要传入的是一个引用，
    // 但是如果这里传入的是一个有所有权的值的话就会出现问题.
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());

    if is_dl_det_enable {
        println!(
            "kernel:pid[{}] tid[{}] sys_semaphore_down",
            current_task().unwrap().process.upgrade().unwrap().getpid(),
            current_task()
                .unwrap()
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .tid
        );
        // let task_count = process_inner.tasks.len();
        // let mut finish = Vec::new();
        println!("************[KERNAL DEBUG]: starting deadlock detection now!!!*************\n");
        println!("[KERNEL DEBUG]: try to down semaphore: {}", sem_id);

        let mut work = Vec::new();
        for sem in &process_inner.semaphore_list {
            if sem.is_some() {
                // [destinyfvcker?] 这里需要使用 as_ref 的原因是：
                // cannot move out of `*sem` which is behind a shared reference
                // help: consider calling `.as_ref()` or `.as_mut()` to borrow the type's contents
                let sem_id = sem.as_ref().unwrap().sem_id;
                let mut count = sem.as_ref().unwrap().inner.exclusive_access().count;

                // [destinyfvcker!] 这里我在 semaphore.rs 之中的实现实际上使用了 clone 方法
                // 但是现在看来是不用的，会自动调用 Copy
                // [destinyfvcker?] 什么时候会自动调用？
                println!(
                    "[KERNAL DEBUG] work: sem_id = {:?}, count = {}",
                    sem_id, count
                );
                count = count.max(0);
                work.push((sem_id, count));
            }
        }
        println!("[KERNAL DEBUG] work: print end-----------------------\n");
        println!("------------- [kernel debug start to print work] -------------");
        for item in &work {
            println!("[KERNAL DEBUG] work: {:?}", item);
        }
        println!("------------- [kernel debug print work end] -------------\n");

        let mut allocations = Vec::new();
        let mut needs = Vec::new();
        let mut finish = Vec::new();
        println!(
            "[KERNEL DEBUG] process taks count is: {}",
            &process_inner.tasks.len()
        );

        // let mut tid = 0;
        for task in &process_inner.tasks {
            if task.is_none() {
                continue;
            }
            let mut task_allocation = Vec::new();
            let mut task_need = Vec::new();

            let task = Arc::clone(task.as_ref().unwrap());
            let task_inner = task.inner_exclusive_access();
            if task_inner.res.is_none() {
                continue;
            }
            let tid = task_inner.res.as_ref().unwrap().tid;

            for sem_alloc in &task_inner.allocation {
                // let a: SemId = sem_alloc.0;
                // let b: isize = sem_alloc.1;
                println!(
                    "[KERNAL DEBUG] task_alloc: tid = {}, sem_id = {:?}, count = {}",
                    tid, sem_alloc.0, sem_alloc.1
                );
                task_allocation.push((sem_alloc.0, sem_alloc.1));
            }

            for sem_need in &task_inner.need {
                println!(
                    "[KERNAL DEBUG] task_need: tid = {}, sem_id = {:?}, count = {}",
                    tid, sem_need.0, sem_need.1
                );
                task_need.push((sem_need.0, sem_need.1));
            }
            if tid == tid_now {
                task_need.push((SemId(sem_id), 1));
            }
            println!("[KERNAL DEBUG] starting next task.........\n");

            // if !task_allocation.is_empty() {
            allocations.push((tid, task_allocation));
            // }

            // if !task_need.is_empty() {
            needs.push((tid, task_need));
            // }
            finish.push((tid, false));
            // tid += 1;
        }
        // allocations.remove(0);
        // let needs_len = needs.len();
        // needs.swap(0, needs_len - 1);

        // println!("[KERNAL DEBUG] work: print end-----------------------\n");
        // println!("------------- [kernel debug start to print work] -------------");
        // for item in &work {
        //     println!("[KERNAL DEBUG] work: {:?}", item);
        // }
        // println!("------------- [kernel debug print work end] -------------\n");
        // let mut inde = 0;
        let mut is_processing = true;
        while is_processing {
            is_processing = false;
            for (tid, finished) in &mut finish {
                if !*finished {
                    let (_, task_needs) = needs.iter().find(|(tid_, _)| *tid_ == *tid).unwrap();

                    let mut is_enough = true;
                    for (sem_id, count) in task_needs {
                        if !is_enough {
                            break;
                        }
                        for item in &work {
                            if item.0 == *sem_id {
                                if item.1 < *count as isize {
                                    is_enough = false;
                                    break;
                                }
                            }
                        }
                    }

                    if is_enough {
                        let task_allocation = allocations
                            .iter()
                            .find(|(tid_, _)| *tid_ == *tid)
                            .map(|(_, t_alloc)| t_alloc);
                        if task_allocation.is_some() {
                            let task_allocation = task_allocation.unwrap();
                            for (sem_id, alloc_count) in task_allocation {
                                let work_item = work
                                    .iter_mut()
                                    .find(|(sem_id_, _)| *sem_id_ == *sem_id)
                                    .unwrap();

                                work_item.1 += alloc_count;
                            }
                        }
                        *finished = true;
                        is_processing = true;
                    }
                }
            }
        }

        for (_, is_finished) in &finish {
            if !is_finished {
                return -0xDEAD;
            }
        }

        // for (tid, task_needs) in needs {
        //     // if tid == 0 {
        //     //     continue;
        //     // }
        //     println!("[KERNEL DEBUG] index:{}", inde);
        //     inde += 1;
        //     for (sem_id, count) in task_needs {
        //         // if let Some(item) = work
        //         //     .iter()
        //         //     .find(|(sem_id_, count_)| *sem_id_ == sem_id && *count_ >= count as isize)
        //         // {
        //         // } else {
        //         //     return -0xDEAD;
        //         // }
        //         // let is_satisfied = true;
        //         for item in &work {
        //             if item.0 == sem_id {
        //                 if item.1 < count as isize {
        //                     println!("[KERNEL DEBUG]: DEAD LOCK!!!!!!!!!!!!!!!!!!!!!!!!");
        //                     return -0xDEAD;
        //                 }
        //                 println!("[KERNEL DEBUG]: in sys_semaphore_down dead lock detect,
        //                             sem_id_ = {:?} and count_ = {}; can satisfy require need tid = {}, need = {:?}:{}", item.0, item.1, tid, sem_id, count);
        //             }
        //         }
        //     }

        //     let task_allocation = allocations
        //         .iter()
        //         .find(|(tid_, _)| *tid_ == tid)
        //         .map(|(_, t_alloc)| t_alloc);
        //     if task_allocation.is_some() {
        //         let task_allocation = task_allocation.unwrap();
        //         for (sem_id, alloc_count) in task_allocation {
        //             let work_item = work
        //                 .iter_mut()
        //                 .find(|(sem_id_, _)| *sem_id_ == *sem_id)
        //                 .unwrap();

        //             work_item.1 += alloc_count;
        //         }
        //     }
        // }

        // println!("\n\n\n\n\n")
    }

    drop(process_inner);
    sem.down();
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_enable_deadlock_detect",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );

    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    process_inner.is_dl_det_enable = true;

    0
}
