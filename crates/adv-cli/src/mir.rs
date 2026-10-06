//! 深轨边车调用（片A3）：`adv` 自身**不链** `rustc_private`，边车以子进程跑
//! （RESEARCH 01 边车纪律——不稳定 API 的崩溃隔离在进程外）。
//!
//! 输出映射成快轨 `Finding` 再由 `to_jsonl()` 打印 ⇒ 行契约只有一处定义，
//! 深轨不可能"长得像"而没有真的对齐。

use adv_rules::matcher::ENGINE_MIR;
use adv_rules::{Finding, Rule};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 定位驱动 exe：`--driver` 显式给定优先，否则取自身同目录（同一 target 目录的产物）。
pub fn locate_driver(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        let pb = PathBuf::from(p);
        return if pb.is_file() {
            Ok(pb)
        } else {
            Err(format!("--driver 指定的文件不存在：{p}"))
        };
    }
    let exe = std::env::current_exe().map_err(|e| format!("取自身路径失败：{e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "adv 可执行文件没有父目录".to_string())?;
    let name = format!("adv-ast-rust-driver{}", std::env::consts::EXE_SUFFIX);
    let candidate = dir.join(name);
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(format!(
            "深轨驱动不在预期位置：{}（用 --driver 指定，或先 cargo build -p adv-ast-rust）",
            candidate.display()
        ))
    }
}

/// 工具链 `bin/` 目录：驱动运行期要在那里找到 `rustc_driver` 的 DLL。
fn toolchain_bin() -> Result<PathBuf, String> {
    let out = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .map_err(|e| format!("调用 rustc 失败（深轨需要含 rustc-dev 的工具链）：{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "rustc --print sysroot 退出码非零：{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let sysroot = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(PathBuf::from(sysroot).join("bin"))
}

/// 给子进程补 PATH（不改动本进程环境）。
fn child_path(bin_dir: &Path) -> OsString {
    let mut path = OsString::from(bin_dir);
    path.push(if cfg!(windows) { ";" } else { ":" });
    path.push(std::env::var_os("PATH").unwrap_or_default());
    path
}

/// 单文件跑深轨，返回映射后的发现。驱动非零退出/输出畸形一律 Err（不静默当"无发现"）。
pub fn scan_file(
    driver: &Path,
    rules_dir: &Path,
    file: &Path,
    rules: &[Rule],
) -> Result<Vec<Finding>, String> {
    let bin_dir = toolchain_bin()?;
    let out = Command::new(driver)
        .arg("--taint")
        .arg(rules_dir)
        .arg(file)
        .env("PATH", child_path(&bin_dir))
        .output()
        .map_err(|e| format!("启动深轨驱动 {} 失败：{e}", driver.display()))?;
    if !out.status.success() {
        return Err(format!(
            "深轨驱动退出码 {:?}：{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("")
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| parse_finding(line, Some(file), rules))
        .collect()
}

fn parse_finding(
    line: &str,
    expect_file: Option<&Path>,
    rules: &[Rule],
) -> Result<Finding, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("深轨输出不是 JSON：{line}（{e}）"))?;
    let rule_id = value["rule"]
        .as_str()
        .ok_or_else(|| format!("深轨输出缺 rule 字段：{line}"))?;
    let rule = rules
        .iter()
        .find(|r| r.id == rule_id)
        .ok_or_else(|| format!("深轨引用未知规则 {rule_id}（规则集不一致，不降级为忽略）"))?;
    let num = |key: &str| -> Result<usize, String> {
        value[key]
            .as_u64()
            .map(|v| v as usize)
            .ok_or_else(|| format!("深轨输出缺/非数值字段 {key}：{line}"))
    };
    // 文件归属以**记录里的 file** 为准（cargo 档一次扫整个 crate，没有"调用方已知文件"
    // 这个前提）。单文件档传进 expect_file 做交叉核对：记录说它扫的不是这个文件就是漂移。
    let recorded = value["file"]
        .as_str()
        .ok_or_else(|| format!("深轨输出缺 file 字段：{line}"))?
        .replace('\\', "/");
    if let Some(p) = expect_file {
        let want = p.to_string_lossy().replace('\\', "/");
        let same_name = Path::new(&recorded).file_name() == Path::new(&want).file_name();
        if recorded != want && !same_name {
            return Err(format!("深轨记录的文件 {recorded} 不是本次扫描对象 {want}"));
        }
    }
    Ok(Finding {
        rule: rule.id.clone(),
        severity: adv_rules::matcher::severity_name(rule.severity),
        message: rule.message.clone(),
        file: recorded,
        start_line: num("start_line")?,
        start_col: num("start_col")?,
        end_line: num("end_line")?,
        end_col: num("end_col")?,
        engine: ENGINE_MIR.to_string(),
    })
}

/// 整 crate 一档（片B2）：把边车当 `RUSTC_WORKSPACE_WRAPPER` 挂进目标 crate 的 cargo 构建，
/// 读回它按 crate 落的 JSONL。
///
/// 为什么需要这一档：单文件档硬拼 `--crate-type=lib` 且不带依赖解析，真实仓库里任何
/// `use 别crate` 的文件都编不过 ⇒ exit=101、0 发现（片A2「适用边界」实测）。走 cargo 才有
/// `--extern`/`--edition`/模块解析，深轨才吃得到 crate 形状的代码。
pub fn scan_crate_via_cargo(
    crate_dir: &Path,
    rules_dir: &Path,
    driver: &Path,
    rules: &[Rule],
) -> Result<Vec<Finding>, String> {
    let manifest = crate_dir.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!(
            "cargo 档要的是一个 crate 目录（含 Cargo.toml）：{}",
            crate_dir.display()
        ));
    }
    let bin_dir = toolchain_bin()?;
    // 规则目录必须传**绝对路径**：包装器进程的工作目录是被扫 crate 的根（cargo 在那儿
    // 起 rustc），相对路径会解析到不存在的目录（2026-10-06 实测：驱动报"读规则目录 rules"）。
    let rules_abs = rules_dir
        .canonicalize()
        .map_err(|e| format!("规则目录 {} 解析失败：{e}", rules_dir.display()))?;
    let scratch = std::env::temp_dir().join(format!("adv-mir-cargo-{}", std::process::id()));
    let mir_out = scratch.join("mir");
    let target_dir = scratch.join("target");
    std::fs::create_dir_all(&mir_out).map_err(|e| format!("建 {} 失败：{e}", mir_out.display()))?;
    let out = Command::new("cargo")
        .args([
            "build",
            "--quiet",
            "--manifest-path",
            manifest.to_str().ok_or("manifest 路径不是有效 UTF-8")?,
            "--target-dir",
            target_dir.to_str().ok_or("target 路径不是有效 UTF-8")?,
        ])
        .env("PATH", child_path(&bin_dir))
        .env("RUSTC_WORKSPACE_WRAPPER", driver)
        .env("ADV_MIR_WRAPPER", "1")
        .env("ADV_MIR_RULES", &rules_abs)
        .env("ADV_MIR_OUT", &mir_out)
        .output()
        .map_err(|e| format!("启动 cargo 失败：{e}"))?;
    // 先把账读回来再清临时目录：清晚了留残渣，读晚了拿不到内容。
    let collected = collect_crate_findings(&mir_out, rules);
    let _ = std::fs::remove_dir_all(&scratch);
    if !out.status.success() {
        return Err(format!(
            "cargo 构建失败（退出码 {:?}）：{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("")
        ));
    }
    collected
}

/// 读回 `<crate>.jsonl`。一个都没有 = 红（静默空就是假绿入口，与片A2 的边界口径一致）。
fn collect_crate_findings(mir_out: &Path, rules: &[Rule]) -> Result<Vec<Finding>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(mir_out)
        .map_err(|e| format!("读 {} 失败：{e}", mir_out.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "cargo 档深轨没落任何 crate 的账（{} 下没有 .jsonl）——不能折算为\"无发现\"",
            mir_out.display()
        ));
    }
    let mut found = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .map_err(|e| format!("读 {} 失败：{e}", file.display()))?;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            found.push(parse_finding(line, None, rules)?);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一条合法的深轨记录，只把 file 字段换成传进来的值。
    fn record(file: &str) -> String {
        format!(
            "{{\"rule\":\"RS-UNWRAP-USE\",\"file\":\"{file}\",\"start_line\":3,\"start_col\":4,\"end_line\":3,\"end_col\":12}}"
        )
    }

    /// `expect_file` 交叉核对的三态。缺任何一态都会放过一条变异：
    /// `!= → ==` 由第三态杀（片E 实测是 deep_cargo 的金样在杀它，本条把归属搬回本包），
    /// `&& → ||` 由第二态杀——只有"同文件名、不同目录"这个形状能区分两者。
    #[test]
    fn expect_file_guard_splits_three_ways() {
        let rules = adv_rules::load_rules(Path::new("../../rules")).expect("载规则");
        let want = Path::new("src/lib.rs");
        assert!(
            parse_finding(&record("src/lib.rs"), Some(want), &rules).is_ok(),
            "同路径必须放行"
        );
        assert!(
            parse_finding(&record("inner/src/lib.rs"), Some(want), &rules).is_ok(),
            "只比文件名这条容忍是给 cargo 档的相对路径形状，写成 || 就会在这里误判红"
        );
        let err = parse_finding(&record("src/other.rs"), Some(want), &rules)
            .expect_err("记录的文件不是本次扫描对象 ⇒ 必须红");
        assert!(err.contains("不是本次扫描对象"), "{err}");
        // 没有 expect_file（cargo 档一次扫整个 crate）时不该有任何交叉核对可跳过。
        assert!(parse_finding(&record("whatever.rs"), None, &rules).is_ok());
    }

    /// `toolchain_bin` 是深轨运行期找 `rustc_driver` DLL 的那条路：把它换成空路径
    /// （门报的存活变异之一）在本包此前完全不可观察。
    #[test]
    fn toolchain_bin_points_at_a_real_bin_dir() {
        match toolchain_bin() {
            Ok(dir) => {
                assert!(dir.is_dir(), "sysroot 的 bin 不是目录：{}", dir.display());
                let has_rustc = ["rustc.exe", "rustc"].iter().any(|n| dir.join(n).is_file());
                assert!(has_rustc, "bin 里没有 rustc：{}", dir.display());
            }
            // 本机没有 rustc-dev 工具链时如实失败——那也是判定，不是空转。
            Err(e) => panic!("本机 rustc --print sysroot 应当可用，实得：{e}"),
        }
    }
}
