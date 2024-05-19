//! virtio_blk device driver

mod virtio_blk;

pub use virtio_blk::VirtIOBlock;

use alloc::sync::Arc;
use easy_fs::BlockDevice;
use lazy_static::*;

// [destinyfvcker] 在 qemu 上，我们使用 VirtIOBlock 来访问 VirtIO 块设备
type BlockDeviceImpl = virtio_blk::VirtIOBlock;

lazy_static! {
    /// The global block device driver instance: BLOCK_DEVICE with BlockDevice trait
    pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(BlockDeviceImpl::new());
}

// [destinyfvcker] 在启动 Qemu 模拟器的时候，我们可以配置参数来添加一块 VirtIO 块设备：
//  1 # os/Makefile
//  2
//  3 FS_IMG := ../user/target/$(TARGET)/$(MODE)/fs.img
//  4
//  5 run: build
//  6    @qemu-system-riscv64 \
//  7        -machine virt \
//  8        -nographic \
//  9        -bios $(BOOTLOADER) \
// 10        -device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA) \
// 11        -drive file=$(FS_IMG),if=none,format=raw,id=x0 \
// 12        -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0
// 第 11 行，我们为虚拟机添加一块虚拟硬盘，内容为我们之前通过 easy-fs-fuse 工具打包的包含应用 ELF 的 easy-fs 镜像，并命名为 x0 。
// 第 12 行，我们将硬盘 x0 作为一个 VirtIO 总线中的一个块设备接入到虚拟机系统中。
// virtio-mmio-bus.0 表示 VirtIO 总线通过 MMIO 进行控制，且该块设备在总线中的编号为 0 。
//
// 内存映射 I/O（MMIO, Memory-Mapped I/O）指的是通过特定的物理内存地址来访问外设的设备寄存器。
// 查阅资料可以知道：VirtIO 总线的 MMIO 物理地址区间为从 0x10001000 开头的 4KiB
// 所以在 config 子模块之中直接硬编码 Qemu 上的 VirtIO 总线的 MMIO 地址区间（也就是起始地址和长度）
// pub const MMIO: &[(usize, usize)] = &[
//     (0x10001000, 0x1000),
// ];
//
// 并且在创建内核地址空间的时候需要建立页表映射：具体代码在 mm/memory_set.rs 中
//
// 这就在内核之中建立了我们的简单的文件系统的最基础的一层抽象：块设备驱动层

#[allow(unused)]
/// Test the block device
pub fn block_device_test() {
    let block_device = BLOCK_DEVICE.clone();
    let mut write_buffer = [0u8; 512];
    let mut read_buffer = [0u8; 512];
    for i in 0..512 {
        for byte in write_buffer.iter_mut() {
            *byte = i as u8;
        }
        block_device.write_block(i as usize, &write_buffer);
        block_device.read_block(i as usize, &mut read_buffer);
        assert_eq!(write_buffer, read_buffer);
    }
    println!("block device test passed!");
}
