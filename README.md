# rCore-Tutorial-Code-2024S

- [rCore-Tutorial-Code-2024S](#rcore-tutorial-code-2024s)
  - [\[destinyfvcker\] 引言](#destinyfvcker-引言)
    - [线程定义](#线程定义)
    - [同步互斥](#同步互斥)
  - [Code](#code)
  - [Documents](#documents)
  - [OS API docs of rCore Tutorial Code 2024S](#os-api-docs-of-rcore-tutorial-code-2024s)
  - [Related Resources](#related-resources)
  - [Build \& Run](#build--run)
  - [Grading](#grading)

### [destinyfvcker] 引言

对于很多应用来说，如果以单一进程的形式运行，逻辑上会存在多个可并行执行的任务，如果其中一个任务被阻塞，将会引起不依赖该任务的其他任务也被阻塞。

如果我们把一个进程内的多个可并行执行的任务通过一种更细粒度的方式来让操作系统进行调度，就可以通过处理器时间片切换实现这种细粒度的并发执行，这种细粒度的调度对象就是线程。

到目前为止的并发，仅仅是进程间的并发，对于一个进程内部还没有并发性的提现。这就是线程（Thread）出现的原因：提高一个进程内部的并发性。

#### 线程定义

进程可以包含 1~n 个线程，属于同一个进程的线程共享进程的资源，就像是地址空间、打开的文件等等，基本的线程由下面的元素组成：

1. 线程 ID
2. 执行状态
3. 当前指令指针（PC）
4. 寄存器集合
5. 栈

线程是可以被操作系统或者用户态调度器独立调度（Scheduling）和分派（Dispatch） 的基本单位。

#### 同步互斥

并发相关术语

- 共享资源（shared resource）：不同的线程/进程都能访问的变量或数据结构。

- 临界区（critical section）：访问共享资源的一段代码。

- 竞态条件（race condition）：多个线程/进程都进入临界区时，都试图更新共享的数据结构，导致产生了不期望的结果。

- 不确定性（indeterminate）： 多个线程/进程在执行过程中出现了竞态条件，导致执行结果取决于哪些线程在何时运行， 即执行结果不确定，而开发者期望得到的是确定的结果。

- 互斥（mutual exclusion）：一种操作原语，能保证只有一个线程进入临界区，从而避免出现竞态，并产生确定的执行结果。

- 原子性（atomic）：一系列操作要么全部完成，要么一个都没执行，不会看到中间状态。在数据库领域， 具有原子性的一系列操作称为事务（transaction）。

- 同步（synchronization）：多个并发执行的进程/线程在一些关键点上需要互相等待，这种相互制约的等待称为进程/线程同步。

- 死锁（dead lock）：一个线程/进程集合里面的每个线程/进程都在等待只能由这个集合中的其他一个线程/进程 （包括他自身）才能引发的事件，这种情况就是死锁。

- 饥饿（hungry）：指一个可运行的线程/进程尽管能继续执行，但由于操作系统的调度而被无限期地忽视，导致不能执行的情况。

### Code

- [Soure Code of labs for 2024S](https://github.com/LearningOS/rCore-Tutorial-Code-2024S)

### Documents

- Concise Manual: [rCore-Tutorial-Guide-2024S](https://LearningOS.github.io/rCore-Tutorial-Guide-2024S/)

- Detail Book [rCore-Tutorial-Book-v3](https://rcore-os.github.io/rCore-Tutorial-Book-v3/)

### OS API docs of rCore Tutorial Code 2024S

- [OS API docs of ch1](https://learningos.github.io/rCore-Tutorial-Code-2024S/ch1/os/index.html)
  AND [OS API docs of ch2](https://learningos.github.io/rCore-Tutorial-Code-2024S/ch2/os/index.html)
- [OS API docs of ch3](https://learningos.github.io/rCore-Tutorial-Code-2024S/ch3/os/index.html)
  AND [OS API docs of ch4](https://learningos.github.io/rCore-Tutorial-Code-2024S/ch4/os/index.html)
- [OS API docs of ch5](https://learningos.github.io/rCore-Tutorial-Code-2024S/ch5/os/index.html)
  AND [OS API docs of ch6](https://learningos.github.io/rCore-Tutorial-Code-2024S/ch6/os/index.html)
- [OS API docs of ch7](https://learningos.github.io/rCore-Tutorial-Code-2024S/ch7/os/index.html)
  AND [OS API docs of ch8](https://learningos.github.io/rCore-Tutorial-Code-2024S/ch8/os/index.html)
- [OS API docs of ch9](https://learningos.github.io/rCore-Tutorial-Code-2024S/ch9/os/index.html)

### Related Resources

- [Learning Resource](https://github.com/LearningOS/rust-based-os-comp2022/blob/main/relatedinfo.md)

### Build & Run

```bash
# setup build&run environment first
$ git clone https://github.com/LearningOS/rCore-Tutorial-Code-2024S.git
$ cd rCore-Tutorial-Code-2024S
$ git clone https://github.com/LearningOS/rCore-Tutorial-Test-2024S.git user
$ cd os
$ git checkout ch$ID
# run OS in ch$ID
$ make run
```

Notice: $ID is from [1-9]

### Grading

```bash
# setup build&run environment first
$ git clone https://github.com/LearningOS/rCore-Tutorial-Code-2024S.git
$ cd rCore-Tutorial-Code-2024S
$ git clone https://github.com/LearningOS/rCore-Tutorial-Checker-2024S.git ci-user
$ git clone https://github.com/LearningOS/rCore-Tutorial-Test-2024S.git ci-user/user
$ cd ci-user && make test CHAPTER=$ID
```

Notice: $ID is from [3,4,5,6,8]
