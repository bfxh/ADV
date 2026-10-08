//! 20 字节对象 id：**替代 `gix::ObjectId`**（M3-3c 剪掉 git 面，见 `VENDOR.md` 例外二）。
//!
//! 为什么要有这个文件：上游用 `gix::hashtable::{HashMap, HashSet}` 装 `gix::ObjectId` 做 blob 去重表。
//! 我们的入口面（rules/matcher/blob/provenance-from-file）**从不碰 git**，但 `gix` 是非可选依赖，
//! 于是整条 gitoxide 链（含命中 RUSTSEC-2025-0140 的 `gix-date`）被带进依赖树。剪掉它的最小改动是：
//! 把容器换成 std 的 `HashMap`/`HashSet`，键换成本地这个 20 字节 newtype——**语义逐字等价**，
//! 代价是哈希器从 gix 的开放寻址换成 std 的 SipHash（20 字节键上的常数差，未做基准）。
use crate::blob_id::BlobId;

/// 与 `BlobId` 同构的 20 字节键（等于上游的 `gix::ObjectId` 在这个用途上的形状）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId([u8; 20]);

impl ObjectId {
    /// 原始字节（20）。
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl From<BlobId> for ObjectId {
    fn from(blob_id: BlobId) -> Self {
        let mut raw = [0u8; 20];
        raw.copy_from_slice(blob_id.as_bytes());
        ObjectId(raw)
    }
}

impl From<&BlobId> for ObjectId {
    fn from(blob_id: &BlobId) -> Self {
        ObjectId::from(*blob_id)
    }
}
