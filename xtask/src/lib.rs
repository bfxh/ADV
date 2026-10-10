//! ADV 质量门库（xtask lib）：god / lockstep / 抑制 / 变异 / 设计债 各门的可测实现。
//!
//! 用法：`cargo run -p xtask -- <god [--write] | lockstep | gate | mir | mutants | mutants --rotate>`。

pub mod gate;
pub mod god;
pub mod lockstep;
pub mod maturity;
pub mod mir;
pub mod mutants;
pub mod rotation;
pub mod suppress;
pub mod verdict;
