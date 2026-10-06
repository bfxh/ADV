# disk-corpus：unviable「盘满缺测」判据的语料

`outcomes.json` = 2026-10-06 真实一轮 `mutants.out/outcomes.json` 里**逐字取出**的 3 条：

| 条目 | summary | 作用 |
|------|---------|------|
| `xtask/src/mutants.rs::read_baseline` | Unviable | 误报对照：真编译错误（`error[E0277]`，改名留下的真实失败），日志原样拷自真产物 |
| `crates/adv-cli/src/main.rs::engine_of` | Unviable | 正例：`log_path` 指向日志重建文件（见该文件头），签名文本有账可查 |
| `xtask/src/mutants.rs::new_missed` | CaughtMutant | 证明非 Unviable 条目不进盘满扫描 |

目录布局与产物一致：判据拿 `out_dir` 后自己拼 `out_dir/mutants.out/<log_path>`，
所以测试传 `disk-corpus/` 作为 `out_dir` 就能走真路径。
