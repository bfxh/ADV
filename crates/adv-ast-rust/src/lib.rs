//! 深轨边车 crate 的可复用件：rustc-dev 在位判据（`driver_probe`）。
//! 这里刻意不碰 `rustc_private`——判据要能被 build.rs、xtask 门、测试三方共用，
//! 而驱动本体（需要不稳定 API 的部分）留在 bin 目标 `src/main.rs`。

pub mod driver_probe;
pub mod spec;
pub mod wrapper;
