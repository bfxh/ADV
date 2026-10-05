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
        .map(|line| parse_finding(line, file, rules))
        .collect()
}

fn parse_finding(line: &str, file: &Path, rules: &[Rule]) -> Result<Finding, String> {
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
    Ok(Finding {
        rule: rule.id.clone(),
        severity: adv_rules::matcher::severity_name(rule.severity),
        message: rule.message.clone(),
        file: file.to_string_lossy().replace('\\', "/"),
        start_line: num("start_line")?,
        start_col: num("start_col")?,
        end_line: num("end_line")?,
        end_col: num("end_col")?,
        engine: ENGINE_MIR.to_string(),
    })
}
