//! ADV 质量门库（xtask lib）：god 门与 lockstep 门的可测实现。
//!
//! 用法：`cargo run -p xtask -- <god [--write] | lockstep | gate | mir | mutants>`。

pub mod god;
pub mod lockstep;
pub mod mir;
pub mod mutants;
pub mod suppress;
