use super::{
    block_cache_sync_all, get_block_cache, Bitmap, BlockDevice, DiskInode, DiskInodeType, Inode,
    SuperBlock,
};
use crate::BLOCK_SZ;
use alloc::sync::Arc;
use spin::Mutex;

// [destinyfvcker] EasyFileSystem 包含索引节点和数据块的两个位图 inode_bitmap 和 data_bitmap，
// 还记录下索引节点区域和数据块区域起始块编号方便确定每个索引节点和数据块在磁盘上的具体位置。
// 我们还要在其中保留块设备的一个指针 block_device ，在进行后续操作的时候，
// 该指针会被拷贝并传递给下层的数据结构，让它们也能够直接访问块设备。
///An easy file system on block
pub struct EasyFileSystem {
    ///Real device
    pub block_device: Arc<dyn BlockDevice>,
    ///Inode bitmap
    pub inode_bitmap: Bitmap,
    ///Data bitmap
    pub data_bitmap: Bitmap,
    /// 索引节点区域起始块编号
    inode_area_start_block: u32,
    /// 数据块区域起始块编号
    data_area_start_block: u32,
}

type DataBlock = [u8; BLOCK_SZ];

/// An easy fs over a block device
impl EasyFileSystem {
    /// A data block of block size
    pub fn create(
        block_device: Arc<dyn BlockDevice>,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
    ) -> Arc<Mutex<Self>> {
        // 这里 bitmap 位图就是用来看是不是真正的用来保存数据的块被分配出去了，管理用
        // area 才是真的用来装数据的

        // [destinyfvcker] 从 1 开始，因为 0 是 super block
        // calculate block size of areas & create bitmaps
        let inode_bitmap = Bitmap::new(1, inode_bitmap_blocks as usize);
        let inode_num = inode_bitmap.maximum();
        // [destinyfvcker] 总结一下，这里的 (same_value + BLOCK_SZ - 1) / BLOCK_SZ 的意思就是：
        // 假如 same_value 的值起码是 1，整个表达式的结果就是 1（以此类推），如果是 0，那么整个表达式的结果就是 0
        //
        // [destinyfvcker] 但是这里要注意一下，从这里的逻辑来看 inode_num 实际上指的是 DiskInode 这个结构体的数量，
        //但是这个结构体才 128 字节，一个 block 可以装好多个呢
        let inode_area_blocks =
            ((inode_num * core::mem::size_of::<DiskInode>() + BLOCK_SZ - 1) / BLOCK_SZ) as u32;
        let inode_total_blocks = inode_bitmap_blocks + inode_area_blocks;

        let data_total_blocks = total_blocks - 1 - inode_total_blocks;
        // [destinyfvcker] 我们现在希望数据块位图之中的每一个 bit 仍然能够对应到一个数据块
        // 但是数据块位图不能过小，不然会造成某些数据块永远不会被使用，
        // 因此数据块位图区域最合理的大小是剩余的块数除以 4097 再向上取整（原来 some_value + 4069）/ 4097 是这个意思
        // 因为位图中的每个块能够对应 4096 个数据块，其余的块就都作为数据块来使用
        let data_bitmap_blocks = (data_total_blocks + 4096) / 4097;
        let data_area_blocks = data_total_blocks - data_bitmap_blocks;
        let data_bitmap = Bitmap::new(
            (1 + inode_bitmap_blocks + inode_area_blocks) as usize,
            data_bitmap_blocks as usize,
        );

        // 上面有用的信息实际上就只有：
        // 1. inode_bitmap：索引块的位图
        // 2. inode_total_blocks：所有用于索引节点的块数（索引块）
        // 3. data_bitmap：数据块的位图
        // 4. data_bitmap_blocks：用于计算 data_area 开始的节点号
        let mut efs = Self {
            block_device: Arc::clone(&block_device),
            inode_bitmap,
            data_bitmap,
            inode_area_start_block: 1 + inode_bitmap_blocks,
            data_area_start_block: 1 + inode_total_blocks + data_bitmap_blocks,
        };
        // clear all blocks
        for i in 0..total_blocks {
            get_block_cache(i as usize, Arc::clone(&block_device))
                .lock()
                .modify(0, |data_block: &mut DataBlock| {
                    for byte in data_block.iter_mut() {
                        *byte = 0;
                    }
                });
        }
        // initialize SuperBlock
        get_block_cache(0, Arc::clone(&block_device)).lock().modify(
            0,
            |super_block: &mut SuperBlock| {
                super_block.initialize(
                    total_blocks,
                    inode_bitmap_blocks,
                    inode_area_blocks,
                    data_bitmap_blocks,
                    data_area_blocks,
                );
            },
        );
        // write back immediately
        // create a inode for root node "/"
        assert_eq!(efs.alloc_inode(), 0);

        // [destinyfvcker] 这里传进去的 inode_id 是 inner 的，也就是说是 DiskInode 的，不是 block 的 id
        // 所以这个函数 get_dist_inode_pos 就是计算出对应的 block id，
        // 还有对应具体的 DiskInode 在这个 block 之中的偏移量
        let (root_inode_block_id, root_inode_offset) = efs.get_disk_inode_pos(0);
        get_block_cache(root_inode_block_id as usize, Arc::clone(&block_device))
            .lock()
            .modify(root_inode_offset, |disk_inode: &mut DiskInode| {
                disk_inode.initialize(DiskInodeType::Directory);
            });
        block_cache_sync_all();
        Arc::new(Mutex::new(efs))
    }

    // [destinyfvcker] 从一个已经写入了 easy-fs 镜像的块设备上打开 easy-fs
    // 只需要将块设备编号为 0 的块作为超级块读取进来，就可以知道 easy-fs 的磁盘布局，构造 efs 实例
    /// Open a block device as a filesystem
    pub fn open(block_device: Arc<dyn BlockDevice>) -> Arc<Mutex<Self>> {
        // read SuperBlock
        get_block_cache(0, Arc::clone(&block_device))
            .lock()
            .read(0, |super_block: &SuperBlock| {
                assert!(super_block.is_valid(), "Error loading EFS!");
                let inode_total_blocks =
                    super_block.inode_bitmap_blocks + super_block.inode_area_blocks;
                let efs = Self {
                    block_device,
                    inode_bitmap: Bitmap::new(1, super_block.inode_bitmap_blocks as usize),
                    data_bitmap: Bitmap::new(
                        (1 + inode_total_blocks) as usize,
                        super_block.data_bitmap_blocks as usize,
                    ),
                    inode_area_start_block: 1 + super_block.inode_bitmap_blocks,
                    data_area_start_block: 1 + inode_total_blocks + super_block.data_bitmap_blocks,
                };
                Arc::new(Mutex::new(efs))
            })
    }

    /// Get the root inode of the filesystem
    pub fn root_inode(efs: &Arc<Mutex<Self>>) -> Inode {
        let block_device = Arc::clone(&efs.lock().block_device);
        // acquire efs lock temporarily，根目录的 inode index 永远都是 0，因为它总是第一个被创建的
        let (block_id, block_offset) = efs.lock().get_disk_inode_pos(0);
        // release efs lock
        Inode::new(block_id, block_offset, Arc::clone(efs), block_device)
    }

    /// Get inode by id
    pub fn get_disk_inode_pos(&self, inode_id: u32) -> (u32, usize) {
        let inode_size = core::mem::size_of::<DiskInode>();
        let inodes_per_block = (BLOCK_SZ / inode_size) as u32;
        let block_id = self.inode_area_start_block + inode_id / inodes_per_block;
        (
            block_id,
            (inode_id % inodes_per_block) as usize * inode_size,
        )
    }

    /// Get data block by id
    pub fn get_data_block_id(&self, data_block_id: u32) -> u32 {
        self.data_area_start_block + data_block_id
    }

    // [destinyfvcker] 现在 inode 和数据块的分配/回收也由这个文件系统负责了：
    /// Allocate a new inode
    pub fn alloc_inode(&mut self) -> u32 {
        self.inode_bitmap.alloc(&self.block_device).unwrap() as u32
    }

    // [destinyfvcker] dealloc_inode 没有实现，不支持文件删除

    // [destinyfvcker] alloc_data 和 dealloc_data 的分配/回收数据块
    // 传入/返回的参数都表示数据块在块设备上的编号，而不是在数据块位图中分配的bit编号；
    /// Allocate a data block
    pub fn alloc_data(&mut self) -> u32 {
        self.data_bitmap.alloc(&self.block_device).unwrap() as u32 + self.data_area_start_block
    }
    /// Deallocate a data block
    pub fn dealloc_data(&mut self, block_id: u32) {
        get_block_cache(block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(0, |data_block: &mut DataBlock| {
                data_block.iter_mut().for_each(|p| {
                    *p = 0;
                })
            });
        self.data_bitmap.dealloc(
            &self.block_device,
            (block_id - self.data_area_start_block) as usize,
        )
    }
}
