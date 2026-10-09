# disk-corpus：unviable「盘满缺测」判据的语料

`outcomes.json` = 2026-10-06 真实一轮 `mutants.out/outcomes.json` 里**逐字取出**的 3 条：

| 条目 | summary | 语料文件 | 作用 |
|------|---------|----------|------|
| `xtask/src/mutants.rs::read_baseline` | Unviable | `compile-error.txt` | 误报对照：真编译错误（`error[E0277]`，改名留下的真实失败），日志原样拷自真产物 |
| `crates/adv-cli/src/main.rs::engine_of` | Unviable | `enospc.txt` | 正例：`log_path` 指向重建文件，签名文本有账可查 |
| `xtask/src/mutants.rs::new_missed` | CaughtMutant | （不摆） | 证明非 Unviable 条目不进盘满扫描 |

`outcomes.json` 里的 `log_path` 保持产物形态（`log/<名>.log`）；测试把上面两个 `.txt`
按 `<临时目录>/mutants.out/log/<名>.log` 摆好后，再拿这个临时目录当 `out_dir` 喂给判据，
所以走的是判据真实的拼路径分支。

## 为什么摊平存、并且不用 `mutants.out/` 目录名和 `.log` 后缀

两条都是 2026-10-06 实测踩出来的，不是设想：

1. **`mutants.out` 不能作为仓内目录名**：cargo-mutants 复制源码树到 scratch 时
   （`copy_tree.rs:109` 的 `filter_entry`）无条件丢掉任何叫 `mutants.out` /
   `mutants.out.old` 的目录。随仓建这层目录 → scratch 树里没有语料 → 未变异树自测
   就失败 → 门在"一条变异都没跑"的状态下中止（这里门判红是对的，但白跑一轮）。
2. **`.log` 后缀进不了仓**：本仓 `.gitignore` 第 6 行是 `*.log`。用它存语料的结果是
   `git add` 静默不收、提交里只有 README 和 outcomes.json，CI 上 `cargo test --workspace`
   会因缺文件而红。


## oom.txt（2026-10-09 追加）：内存资源线的正例

2026-10-09 增量档一轮**未变异基线**日志逐字拷贝：vectorscan 的 C++ 编译在 4 worker
并发下 cc1plus OOM（`out of memory allocating 16781311 bytes`）。它是内存资源线的实测形状——
与盘满是两条不同的线：磁盘预检再准也拦不住它，判据侧因此拆成
`resource_signature`（返回 `Resource::Disk | Resource::Memory`）与
`unviable_resource_failures`（按线归档文案）。金丝雀
`canary_oom_in_baseline_log_is_memory_line_not_disk` 断言：签名命中、归内存线、
盘满签名集在纯 OOM 日志上零误报（两条线互斥）。操作面出口：`ADV_MUTANTS_JOBS`
降并发（金丝雀 `canary_mutants_jobs_cover_is_fail_closed` 钉 fail-closed：0/负/非数字
一律退默认 4 并播报，不静默放大）。

该轮挂在未变异基线（outcomes 里只有一条 `Failure`/`log/baseline.log`），所以
`unviable_resource_failures` 的 outcomes 扫描分支由重建条目覆盖——键名
`rebuilt_oom_entry` 如实标注重建来源，签名文本出自本文件这份真日志。
