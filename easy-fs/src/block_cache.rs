use super::{BlockDevice, BLOCK_SZ};
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
use spin::Mutex;
/// Cached block inside memory
pub struct BlockCache {
    /// cached block data
    cache: [u8; BLOCK_SZ], // [destinyfvcker] 每一个块的大小是 4096 比特，也就是 512 个字节
    /// underlying block id
    block_id: usize,
    /// underlying block device
    block_device: Arc<dyn BlockDevice>,
    /// whether the block is dirty
    modified: bool,
}

impl BlockCache {
    /// Load a new BlockCache from disk.
    pub fn new(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        let mut cache = [0u8; BLOCK_SZ];

        // [destinyfvcker] 触发 read_block 进行块读取
        block_device.read_block(block_id, &mut cache);
        Self {
            cache,
            block_id,
            block_device,
            modified: false,
        }
    }

    // [destinyfvcker] BlockCache 向上提供以下方法： addr_of_offset、get_ref、get_mut
    /// Get the address of an offset inside the cached block data
    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.cache[offset] as *const _ as usize
    } // [destinyfvcker?] 这个地址是物理地址还是虚拟地址？我现在认为应该就是物理地址

    /// [destinyfvcker] 获取缓冲区之中的位于偏移量 offset 的一个类型为 T 的磁盘上数据结构的不可变引用
    pub fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        let addr = self.addr_of_offset(offset);
        unsafe { &*(addr as *const T) }
    }

    /// [destinyfvcker] 返回一个可变引用，其余和 get_ref 之中的逻辑大致相同
    /// 注意这里要将 BlockCache 的 modified 标记为 true 来表示这些缓冲区已经被修改
    /// 之后需要将数据写回磁盘块才能真正将修改同步到磁盘
    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        self.modified = true;
        let addr = self.addr_of_offset(offset);
        // [destinyfvcker] 可以通过这种方式来将裸指针转换成引用吗？
        unsafe { &mut *(addr as *mut T) }
    }

    // [destinyfvcker] 实际上就是对上面这三个方法的封装，这里用到了闭包
    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }

    pub fn modify<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }

    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            self.block_device.write_block(self.block_id, &self.cache);
        }
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync() // [destinyfvcker] RAII
    }
}
/// Use a block cache of 16 blocks
const BLOCK_CACHE_SIZE: usize = 16;

// ^：块缓存，v：块缓存全局管理器
// ------------------------------------------

// [destinyfvcker] 当我们要对一个磁盘块进行读写的时候，块缓存全局管理器检查它是否已经被载入内存之中，
// 如果是就直接返回，否则就读取磁盘块到内存，如果内存之中驻留的磁盘块缓冲区的数量已满，则需要进行缓存替换。
// 这里使用一种类似于 FIFO 的缓存替换算法，在内存之中维护一个队列：
pub struct BlockCacheManager {
    // [destinyfvcker] 这里块缓存的类型是一个 Arc<Mutex<BlockCache>> 这是 Rust 之中的一个经典组合，
    // 可以同时提供引用和互斥访问
    //
    // 这里共享引用的意义在于：块缓存既需要在管理器 BlockCacheManager 之中保留一个引用，
    // 还需要将引用返回给块缓存的请求者。
    queue: VecDeque<(usize, Arc<Mutex<BlockCache>>)>,
}

impl BlockCacheManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// [destinyfvcker] 尝试从块缓存管理器之中获取一个编号为 block_id 的块缓存，
    /// 通过迭代器之中的 find 方法
    /// 如果找不到的话就会读取磁盘，有可能发生缓存替换
    pub fn get_block_cache(
        &mut self,
        block_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<Mutex<BlockCache>> {
        if let Some(pair) = self.queue.iter().find(|pair| pair.0 == block_id) {
            // [destinyfvcker_new] 之前都没有注意过还有这个方法来克隆 Arc 的值
            Arc::clone(&pair.1)
        } else {
            // substitute
            if self.queue.len() == BLOCK_CACHE_SIZE {
                // from front to tail
                if let Some((idx, _)) = self
                    .queue
                    .iter()
                    .enumerate()
                    // [destinyfvcker] 在这里替换的标准是强引用计数 = 1
                    // 也就是说除了块缓存管理器保留的一份副本之外，在外面没有副本正在使用
                    .find(|(_, pair)| Arc::strong_count(&pair.1) == 1)
                {
                    self.queue.drain(idx..=idx);
                } else {
                    panic!("Run out of BlockCache!");
                }
            }

            // [destinyfvcker] 关于为什么要在这里使用 Arc::clone block_device
            // 因为在 BlockCache 的 new 方法之中涉及到对内存块的读取，
            // 一个 BloackCache 的其他操作也需要 block_device 的支持

            // load block into mem and push back
            let block_cache = Arc::new(Mutex::new(BlockCache::new(
                block_id,
                Arc::clone(&block_device),
            )));
            self.queue.push_back((block_id, Arc::clone(&block_cache)));
            block_cache
        }
    }
}

lazy_static! {
    /// The global block cache manager
    pub static ref BLOCK_CACHE_MANAGER: Mutex<BlockCacheManager> =
        Mutex::new(BlockCacheManager::new());
}
/// Get the block cache corresponding to the given block id and block device
pub fn get_block_cache(
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<Mutex<BlockCache>> {
    BLOCK_CACHE_MANAGER
        .lock()
        .get_block_cache(block_id, block_device)
}
/// Sync all block cache to block device
pub fn block_cache_sync_all() {
    let manager = BLOCK_CACHE_MANAGER.lock();
    for (_, cache) in manager.queue.iter() {
        cache.lock().sync();
    }
}
