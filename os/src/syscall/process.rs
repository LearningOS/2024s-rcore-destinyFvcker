//! Process management syscalls
use core::mem::size_of;

use crate::{
    config::MAX_SYSCALL_NUM,
    mm::translated_byte_buffer,
    task::{
        change_program_brk, current_user_token, exit_current_and_run_next, get_system_call_count,
        get_time_interval, mmap, munmap, suspend_current_and_run_next, TaskStatus,
    },
    timer::{get_time_us, TimeVal},
};

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    // let sec = us / 1_000_000;
    // let usec = us % 1_000_000;
    let data = [us / 1_000_000, us % 1_000_000];
    let mut bytes_array: [u8; 16] = [0; 16]; // 目标数组，用于存储转换后的字节

    for (i, &num) in data.iter().enumerate() {
        let bytes = num.to_ne_bytes();
        let start_index = i * size_of::<usize>();
        let end_index = start_index + bytes.len();
        bytes_array[start_index..end_index].copy_from_slice(&bytes);
    }

    let mut buffers =
        translated_byte_buffer(current_user_token(), ts as *const u8, size_of::<TimeVal>());

    match buffers.len() {
        1 => {
            buffers[0].copy_from_slice(&bytes_array);
        }
        2 => {
            let first_len = buffers[0].len();
            buffers[0].copy_from_slice(&bytes_array[0..first_len]);
            buffers[2].copy_from_slice(&bytes_array[first_len..]);
        }
        _ => panic!(
            "[Kernel by destinyFvcker]Unexcepted TimeVal size: {}!",
            buffers.len()
        ),
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    // trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    trace!("kernel: sys_task_info");
    let status = TaskStatus::Running;
    let mut syscall_times = [0 as u32; MAX_SYSCALL_NUM];
    get_system_call_count(&mut syscall_times);
    let time = get_time_interval();

    // println!("[kernerl] the status of")

    // println!(
    //     "[kenerl]: the times to call get time is {}",
    //     syscall_times[169]
    // );

    // println!("[kenerl]: the times of syscall is {:?}\n", syscall_times);

    for (i, &count) in syscall_times.iter().enumerate() {
        if count != 0 {
            println!("[kenerl] the index if exit syscall is {}", i);
        }
    }

    let mut bytes_data = [0 as u8; size_of::<TaskInfo>()];
    // bytes_data[..8].copy_from_slice(&(status as u8 as usize).to_ne_bytes());
    bytes_data[0] = status as u8;

    // let what = [0; size_of::<u32>()];

    for (i, &num) in syscall_times.iter().enumerate() {
        let bytes = num.to_ne_bytes();
        let start_index = 8 + i * size_of::<u32>();
        let end_index = start_index + bytes.len();
        bytes_data[start_index..end_index].copy_from_slice(&bytes);
    }

    let end_index = size_of::<TaskInfo>() - size_of::<usize>();
    bytes_data[end_index..].copy_from_slice(&time.to_ne_bytes());

    // println!("trasferred byte array is: {:?}\n\n", bytes_data);

    let ti_ptr = ti as *const u8;
    let mut buffers = translated_byte_buffer(current_user_token(), ti_ptr, size_of::<TaskInfo>());

    // println!("[kenerl] the point of taskInfo is {:p}", ti);
    // let point_to_task_info = bytes_data.as_mut_ptr() as *mut TaskInfo;
    // unsafe {
    //     let trasferred_task_info = point_to_task_info.as_mut().unwrap();
    //     println!(
    //         "[kenerl] trasferred taskinfo is {:?} {:?}",
    //         trasferred_task_info.status, trasferred_task_info.syscall_times
    //     );
    // }

    match buffers.len() {
        1 => {
            println!("situation is 1");
            buffers[0].copy_from_slice(&bytes_data);
            // unsafe {
            //     println!("the infomation of task is: {:?}", *ti);
            // }
        }
        2 => {
            println!("situation is 2");
            let first_len = buffers[0].len();
            buffers[0].copy_from_slice(&bytes_data[..first_len]);
            buffers[2].copy_from_slice(&bytes_data[first_len..]);

            // unsafe {
            //     println!("the infomation of task is: {:?}", *ti);
            // }
        }
        _ => panic!(
            "[Kernel by destinyFvcker]Unexcepted TaskInfo size: {}!",
            buffers.len()
        ),
    }

    println!("[kenerl] return byte array(buffer) is:{:?}\n", buffers);
    println!("");
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    mmap(start, len, port)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sysmunmap");
    munmap(start, len)
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
