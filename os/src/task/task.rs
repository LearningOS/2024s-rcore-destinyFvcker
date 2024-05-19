//! Types related to task management & Functions for completely changing TCB
use super::TaskContext;
use super::{kstack_alloc, pid_alloc, KernelStack, PidHandle};
use crate::config::{MAX_SYSCALL_NUM, TRAP_CONTEXT_BASE};
use crate::fs::{File, Stdin, Stdout};
use crate::mm::{MapPermission, MemorySet, PhysPageNum, VirtAddr, VirtPageNum, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::timer::TimeVal;
use crate::trap::{trap_handler, TrapContext};
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;
/// Task control block structure
///
/// Directly save the contents that will not change during running
pub struct TaskControlBlock {
    // Immutable
    /// Process identifier
    pub pid: PidHandle,

    /// Kernel stack corresponding to PID
    pub kernel_stack: KernelStack,

    /// Mutable, use UPSafeCell to prevent data competition
    inner: UPSafeCell<TaskControlBlockInner>,
}

impl TaskControlBlock {
    /// Get the mutable reference of the inner TCB
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    /// Get the address of app's page table
    pub fn get_user_token(&self) -> usize {
        let inner = self.inner_exclusive_access();
        inner.memory_set.token()
    }
}

pub struct TaskControlBlockInner {
    /// The physical page number of the frame where the trap context is placed
    pub trap_cx_ppn: PhysPageNum,

    /// Application data can only appear in areas (under this address)
    /// where the application address space is lower than base_size
    pub base_size: usize,

    // [detinyfvcker] seems like a forgot where does task_cs save, the kernel stack?
    /// Save task context
    pub task_cx: TaskContext,

    /// Maintain the execution status of the current process
    pub task_status: TaskStatus,

    /// Application address space
    pub memory_set: MemorySet,

    // [destinyfvcker] what is the usage of Weak point in Rust?
    /// Parent process of the current process.
    /// Weak will not affect the reference count of the parent
    pub parent: Option<Weak<TaskControlBlock>>,

    /// A vector containing TCBs of all child processes of the current process (as a form of Arc smart point)
    pub children: Vec<Arc<TaskControlBlock>>,

    /// It is set when active exit or execution error occurs
    pub exit_code: i32,

    /// Heap bottom
    pub heap_bottom: usize,

    /// Program break
    pub program_brk: usize,

    // +------------[impl_destinyfvcker] implimented in ch3 ------------+
    /// To record syscall times
    pub syscall_times: [u32; MAX_SYSCALL_NUM],

    /// Task start time
    pub start_time: TimeVal,

    /// last syscall time
    pub last_syscall_time: TimeVal,
    // +------------[impl_destinyfvcker] implemented in ch5 ------------+
    /// process's priotiry
    pub proc_prio: usize,

    /// process's stride
    pub proc_stride: usize,

    // +-----------[impl_destinyfvcker] implemented in ch6 -------------+
    /// [destinyfvcker] 在进程控制块之中加入描述符表的相应字段
    /// Vec：无需考虑设置一个固定的文件描述符数量上限
    /// Option：可以区分一个文件描述符当前是不是空闲的，当它是 None 的时候就是空闲的，Some 表示的就是已占用
    /// Arc：后面在多进程的章节可能会有多个进程共享一个文件来对它进行读写
    /// dyn 提供了多态能力
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
}

impl TaskControlBlockInner {
    // [destinyfvcker_gg] 这里的这个 get_mut 方法还有这种使用方式吗？以前见都没有见过！
    // 好吧，我看错了，这个 get_mut 方法根本就不是标准库之中的方法，是定义在 PhyPageNum 之中的一个泛型方法，
    // 它会调用对应的 PhyAddr 之中的 get_mut 方法——在 unsafe 块之中将 usize 转化为指针，
    // 然后使用 as_mut 方法将其转换成可变引用
    /// get the trap context
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    /// get the user token
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    /// get the status of current process
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    // detect if current process is a zombie process
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }
}

impl TaskControlBlock {
    /// Create a new process
    ///
    /// [destinyfucker] parameter is the elf_data of target user application
    /// At present, **it is only used for the creation of initproc** ([destinyfvcker] attention! it is only used for what?)
    pub fn new(elf_data: &[u8]) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = kstack_alloc();
        let kernel_stack_top = kernel_stack.get_top();
        // push a task context which goes to trap_return to the top of kernel stack
        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    // [destinyfvcker] 初始化任务，在切换到它时直接返回用户态执行
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    heap_bottom: user_sp,
                    program_brk: user_sp,
                    // [destinyfvcker] +---------------methond implied in chapter 3~4---------------+
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    start_time: TimeVal::default(),
                    last_syscall_time: TimeVal::default(),
                    proc_prio: 16,
                    proc_stride: 0,
                    // [destinyfvcker] +----- impl in ch6 -----+
                    // 当新建一个进程的时候，按照先前的说明为进程打开标准输入文件和标准输出文件，
                    // 同时可以知道：当 fork 的时候，字进程会完全继承父进程的文件描述符表
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                })
            },
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        // [destinyfvcker] 将用户态相关的寄存器（执行环境）恢复成我们希望的样子
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            // [destinyfvcker] 因为在切换特权级的时候需要进行 sp 指向的栈的切换，
            // 所以这里关于内核栈的信息不仅内核要知道，用户也要知道
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }

    // [destinyfvcker] exec 系统调用使得一个进程能够加载一个新的 ELF 可执行文件替换原有的应用地址空间并开始执行
    /// Load a new elf to replace the original application address space and start execution
    pub fn exec(&self, elf_data: &[u8]) {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);

        // [destinyfvcker] 注意这里是通过一个对应的 memory_set 实例来对对应的 trap 上下文进行加载
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();

        // **** access current TCB exclusively
        let mut inner = self.inner_exclusive_access();

        // [destinyfvcker] 将生成的全新的地址空间之中的信息直接替换进来，这将导致原有地址空间生命周期结束，里面包含的全部物理页帧都会被回收
        // substitute memory_set
        inner.memory_set = memory_set;
        // update trap_cx ppn
        inner.trap_cx_ppn = trap_cx_ppn;
        // initialize base_size
        inner.base_size = user_sp;

        // [destinyfvcker] TaskControlBlockInner 的字段有：
        //  trap_cx_ppn, 被替换
        //  base_size: user_sp, 被修改
        //  task_cx,
        //  task_status: TaskStatus::Ready,
        //  memory_set, 被替换
        //  parent: None,
        //  children: Vec::new(),
        //  exit_code: 0,
        //  heap_bottom: user_sp,
        //  program_brk: user_sp,
        //
        // 下面都是在实验之中自己实现的字段：
        //             syscall_times: [0; MAX_SYSCALL_NUM],
        //             start_time: TimeVal::default(),
        //             last_syscall_time: TimeVal::default(),

        // [destinyfvcker] 然后修改新的地址空间之中的 Trap 上下文，将解析得到的应用入口点、用户栈位置以及一些内核的信息进行初始化，这样才能正常实现 Trap 机制
        // initialize trap_cx
        let trap_cx = inner.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
        // **** release inner automatically
        // **** release current PCB
    }

    /// spawn a new process by elf_data provided by user
    pub fn spawn(self: &Arc<Self>, elf_data: &[u8]) -> Arc<Self> {
        let spawn_task_control_block = Arc::new(TaskControlBlock::new(elf_data));

        let mut parent_inner = self.inner_exclusive_access();
        parent_inner.children.push(spawn_task_control_block.clone());

        let mut inner = spawn_task_control_block.inner_exclusive_access();
        inner.parent = Some(Arc::downgrade(self));

        // [destinyfvcker!] 在 Rust 之中，如果一个变量已经被借用，它的所有权就不能被移动！
        // 所以如果下面你把这个 drop(inner) 注释掉的话，会过不了编译
        drop(inner);
        // return
        spawn_task_control_block
    }

    // [destinyfvcker] 这个方法借用了在 memory_set.rs 之中对地址空间
    // from_existed_user 的实现（可以复制一个完整的地址空间）
    // [destinyfvcker?] 这个函数的参数和返回值我觉得非常有意思，
    // 返回值不用多说，就是返回对于新创建的子进程的进程控制块 TaskControlBlock 的引用，但是这个参数是什么意思？
    // 注意在下面创建 TaskControlBlockInner 的时候，使用了一个 Arc::downgrade 方法，
    // 这个方法可以将一个 Arc 指针的引用转化为一个对于其指向对象的弱应用
    //
    // [destinyfvcker] 这个方法最后会提供给系统调用 sys_fork 进行使用（os/src/syscall/process.rs）
    /// parent process fork the child process
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        // ---- access parent PCB exclusively
        let mut parent_inner = self.inner_exclusive_access();
        // copy user space(include trap context)
        let memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);

        // [destinyfvcker?] 关于这个 TrapContext，我现在还是有点不清不楚的，
        // 在这里的 fork 实现里面，这个 trap_cx_ppn 里面的数据肯定就是使用上面的 from_existed_user 方法拷贝过来的
        // [destinyfvcker?]但是拷贝的源头是什么呢？是在 memory_set 之中实现的 from_elf 方法，这个方法我还没有看，因为根本就不了解 elf 的构成
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();

        // [destinyfvcker] 这里使用了我们在 id.rs 之中实现的 PID 和 KERNEL_STACK 分配器，
        // 这里印证了我在 kstack_alloc() 函数之中的猜测：pid_alloc() 函数和 kernel_stack() 函数是同事进行调用的，
        // 这样才能保证它们行为一致
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = kstack_alloc();

        // [destinyfvcker?] 这里有一个疑问啊, 就是在 kernel stack 之中到低分配了什么？
        // 想了一下发现，好像在用户空间（MemorySet）之中没有一个叫做 kernel_stack 的东西
        // kernel_stack 就是保存在任务控制块之中
        // 算了，再来回顾一下内核栈的相关内容吧，内核栈保存在【destinyfvcker?】 我忘记了，反正就在内核的某一个段里面（好像是.bss）
        // 所有应用的内核栈都排列在一起，这里应为内存还是相对宽松的，所以两个应用的内核栈之间会隔一个页的大小，用来检测是不是访问了错误的内存【destinyfvcker?】但是原理是什么？
        // 对于上面的这个问题，我找到了在文档之中的解释：
        //
        // [destinyfvcker document] *==============
        // 注意相邻两个内核栈之间会预留一个 保护页面 (Guard Page) ，它是内核地址空间中的空洞，多级页表中并不存在与它相关的映射。
        // 它的意义在于当内核栈空间不足（如调用层数过多或死递归）的时候，代码会尝试访问空洞区域内的虚拟地址，然而它无法在多级页表中找到映射，便会触发异常，
        // 此时控制权会交给内核 trap handler 函数进行异常处理。由于编译器会对访存顺序和局部变量在栈帧中的位置进行优化，我们难以确定一个已经溢出的栈帧中的哪些位置会先被访问，
        // 但总的来说，空洞区域被设置的越大，我们就能越早捕获到这一可能覆盖其他重要数据的错误异常。
        // ====================
        //
        // 现在每一个对应的用户应用都有一个自己专属的内核栈 id，相应的栈底和栈顶地址都可以通过一个固定的运算模式得出
        // 上面的 kstack_alloc 方法就是负责将相关地址打包成一个逻辑段 MapArea，然后加入到内核的虚拟地址空间之中
        let kernel_stack_top = kernel_stack.get_top();

        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size, // [destinyfvcker] Copy from parent
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    // [destinyfvcker] 父子进程关系的管理，将父进程的弱应用计数放到子进程的进程控制块之中
                    // [destinyfvcker?] 为什么要这么做？
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    heap_bottom: parent_inner.heap_bottom, // [destinyfvcker] Copy from parent
                    program_brk: parent_inner.program_brk, // [destinyfvcker] Copy from parent
                    // +----------------------------------+
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    start_time: TimeVal::default(),
                    last_syscall_time: TimeVal::default(),
                    proc_prio: 16,
                    proc_stride: 0,
                    // +---------------ch6----------------+
                    fd_table: new_fd_table,
                })
            },
        });
        // add child
        parent_inner.children.push(task_control_block.clone());
        // modify kernel_sp in trap_cx
        // **** access child PCB exclusively
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        // return
        task_control_block
        // **** release child PCB
        // ---- release parent PCB
    }

    /// get pid of process
    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    /// change the location of the program break. return None if failed.
    pub fn change_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner_exclusive_access();
        let heap_bottom = inner.heap_bottom;
        let old_break = inner.program_brk;
        let new_brk = inner.program_brk as isize + size as isize;
        if new_brk < heap_bottom as isize {
            return None;
        }
        let result = if size < 0 {
            inner
                .memory_set
                .shrink_to(VirtAddr(heap_bottom), VirtAddr(new_brk as usize))
        } else {
            inner
                .memory_set
                .append_to(VirtAddr(heap_bottom), VirtAddr(new_brk as usize))
        };
        if result {
            inner.program_brk = new_brk as usize;
            Some(old_break)
        } else {
            None
        }
    }

    // +---------- [impl_destinyfvcker] in ch5 ----------+
    /// get syscall count of process
    pub fn get_system_call_count(&self, dst: &mut [u32]) {
        let inner = self.inner_exclusive_access();

        for (i, &count) in inner.syscall_times.iter().enumerate() {
            dst[i] = count;
        }
    }

    /// undate system call count of process
    pub fn update_system_call_count(&self, syscall_id: usize) {
        let mut inner = self.inner_exclusive_access();

        inner.syscall_times[syscall_id] += 1;
    }

    /// update start time of process
    pub fn update_start_time(&self) {
        let mut inner = self.inner_exclusive_access();

        inner.start_time.update();
    }

    /// update last system call time of process
    pub fn update_last_syscall_time(&self) {
        let mut inner = self.inner_exclusive_access();

        inner.last_syscall_time.update();
    }

    /// calculate time inverval between the process start time and the last syscall time of a process
    pub fn calculate_time_interval(&self) -> usize {
        let inner = self.inner_exclusive_access();

        inner.last_syscall_time.as_ms() - inner.start_time.as_ms()
    }

    /// judge a vp provided by user application is confict with existed vp when user use mmap syscall to allocate memory
    pub fn is_conflict(&self, start: VirtPageNum, end: VirtPageNum) -> bool {
        let inner = self.inner_exclusive_access();

        inner.memory_set.is_conflict(start, end)
    }

    /// judge a address range is allocated when user use munmap to deallocate memory
    pub fn is_mapped(&self, start: VirtPageNum, end: VirtPageNum) -> isize {
        let inner = self.inner_exclusive_access();

        inner.memory_set.is_vmm_mapped(start, end)
    }

    /// alloc a range of memory
    pub fn alloc_mm(&self, start: VirtAddr, end: VirtAddr, port: MapPermission) {
        let mut inner = self.inner_exclusive_access();

        inner.memory_set.insert_framed_area(start, end, port);
    }

    /// deallocate a range of memory
    pub fn dealloc_mm(&self, start: VirtPageNum, end: VirtPageNum, is_cross: isize) {
        let mut inner = self.inner_exclusive_access();

        inner.memory_set.free(start, end, is_cross as usize);
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
/// task status: UnInit, Ready, Running, Exited
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Zombie,
}
