use super::{get_block_cache, BlockDevice, BLOCK_SZ};
use alloc::sync::Arc;

// [destinyfvcker] 在 easy-fs 布局之中存在两类不同的位图，分别对索引节点和数据块进行管理，
// 每一个位图都由若干个块组成，每一个块的大小是 4096bits
// 每一个 bit 都代表一个索引节点/数据块的分配状态

// [destinyfvcker] BitmapBlock 将位图区域之中的一个磁盘块解释为长度为 64 的一个 u64 数组
// 64 = 2^6，64 * 64 = 2^12 = 4096 也就是块的大小
/// A bitmap block
type BitmapBlock = [u64; 64];
/// Number of bits in a block
const BLOCK_BITS: usize = BLOCK_SZ * 8;

// [destinyfvcker] 位图区域的管理器，保存了位图区域的起始块编号和块数。
/// A bitmap
pub struct Bitmap {
    start_block_id: usize,
    blocks: usize, // 位图区域的块数
}

/// Decompose bits into (block_pos, bits64_pos, inner_pos)
fn decomposition(mut bit: usize) -> (usize, usize, usize) {
    let block_pos = bit / BLOCK_BITS;
    bit %= BLOCK_BITS;
    (block_pos, bit / 64, bit % 64)
}

impl Bitmap {
    /// A new bitmap from start block id and number of blocks
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }

    // [destinyfvcker] bitmap 如何分配一个 bit
    //
    // 主要思路是遍历区域之中的每一个块，再在每一个块之中以 bit 组为单位进行遍历
    // 找到一个尚未被全部分配出去的组，最后在里面分配一个 bit
    // 它将会返回分配的 bit 所在的位置，等同于索引节点/数据块的编号
    // 如果所有 bit 都被分配出去了，就返回 None
    //
    /// Allocate a new block from a block device
    pub fn alloc(&self, block_device: &Arc<dyn BlockDevice>) -> Option<usize> {
        for block_id in 0..self.blocks {
            let pos = get_block_cache(
                block_id + self.start_block_id as usize,
                Arc::clone(block_device),
            )
            .lock()
            // [destinyfvcker] 在 modify 方法之中首先会调用 BlockCache::get_mut 方法
            // 来在对应的缓冲区之中读出一个数据结构
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                if let Some((bits64_pos, inner_pos)) = bitmap_block
                    .iter()
                    .enumerate()
                    // [destinyfvcker] 也就是说这个 64 位全部都是 1，已经全部被分配完了
                    .find(|(_, bits64)| **bits64 != u64::MAX)
                    // [destinyfvcker] trailing_ones 函数的作用是给出一个以二进制表述的数最低位有多少个连续的 1
                    .map(|(bits64_pos, bits64)| (bits64_pos, bits64.trailing_ones() as usize))
                {
                    // modify cache
                    bitmap_block[bits64_pos] |= 1u64 << inner_pos;

                    // [destinyfvcker] 如果可以找到的话，bit 组的编号将会保存在变量 bits64_pos 之中，
                    // 而分配的 bit 在组内的位置将会保存在变量 inner_pos 之中
                    //
                    // 这里的逻辑大概是一个位图组之中有 BLOCK_BITS 这么多个“位图对象”，block_id * BLOCK_BITS
                    // 然后就在一个组内（BitmapBlock）进行索引：bits64_pos * 64
                    // 最后在一个u64 之中进行索引：inner_pos as usize
                    Some(block_id * BLOCK_BITS + bits64_pos * 64 + inner_pos as usize)
                } else {
                    None
                }
            });
            if pos.is_some() {
                return pos;
            }
        }
        None
    }

    /// Deallocate a block
    pub fn dealloc(&self, block_device: &Arc<dyn BlockDevice>, bit: usize) {
        // [destinyfvcker] 使用 decomposition 将上面
        // block_id * BLOCK_BITS + bits64_pos * 64 + inner_pos as usize 的计算过程逆向进行
        let (block_pos, bits64_pos, inner_pos) = decomposition(bit);
        get_block_cache(block_pos + self.start_block_id, Arc::clone(block_device))
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                // [destinyfvcker?] 关于为什么这里要有一个 assert!
                // 其实就是判断这个位到底有没有分配出去，如果没有分配就直接 panic 掉，但是就算没有分配出去又会怎么样呢？
                assert!(bitmap_block[bits64_pos] & (1u64 << inner_pos) > 0);

                // [destinyfvcker] 不过这里使用减法来代替 | 操作还是有点被震撼
                bitmap_block[bits64_pos] -= 1u64 << inner_pos;
            });
    }
    /// Get the max number of allocatable blocks
    pub fn maximum(&self) -> usize {
        self.blocks * BLOCK_BITS
    }
}
