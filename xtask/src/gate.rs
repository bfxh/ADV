//! `gate` 档的聚合壳：三个子门在同一条根上依次跑，任一子步抛错 ⇒ 整档判「判不了」。
//!
//! 为什么住在 lib 而不是 bin（DD-0011）：写在 bin 里时集成测试 import 不到它，
//! `replace gate_run -> Ok(vec![])` 在**全绿的树**上与真值同值，端到端断言区分不了，
//! 变异门只能把它记成债。root 做成参数后，金丝雀可以造一根「子门必然红」的根喂进来，
//! 这条变异就有可判定的观察面了——红的树上门不许说绿。

use anyhow::Result;
use std::path::Path;

use crate::{god, lockstep, suppress, verdict};

/// 合档执行：返回违规清单（空 = 绿），明细带子门名前缀（见 [`crate::verdict::merge_gate`]）。
pub fn run(root: &Path) -> Result<Vec<String>> {
    let god_violations = god::run(root)?;
    lockstep::run(root)?;
    let sup = suppress::run(root, &adv_core::today())?;
    Ok(verdict::merge_gate(&[
        ("god", god_violations),
        ("suppress", sup),
    ]))
}
