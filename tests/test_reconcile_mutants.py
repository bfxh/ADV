"""`scripts/reconcile_mutants.py` 的契约：四份清单逐键列名，缺产物/缺基线一律 exit 2。

这条脚本存在的原因（2026-10-06 实测）：同一天我两次把"自己在 outcomes.json 上复算的数"
当成门的结论写进账本，而 skill 里当作收尾固定动作的 `reconcile_mutants.py` 其实并不存在。
判据必须落在能重复执行、能被反例打红的地方，不是落在我每次现写的一段 python。
"""

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "reconcile_mutants.py"
PY = sys.executable


def outcome(file, fn, summary):
    return {
        "summary": summary,
        "scenario": {"Mutant": {"file": file, "function": {"function_name": fn}}},
        "log_path": f"log/{file.replace('/', '_')}__{fn}.log",
    }


def make_tree(tmp: pathlib.Path, outcomes, baseline, logs=None):
    out_dir = tmp / "target" / "mutants-out" / "mutants.out"
    (out_dir / "log").mkdir(parents=True)
    (out_dir / "outcomes.json").write_text(
        json.dumps({"outcomes": outcomes}, ensure_ascii=False), encoding="utf-8"
    )
    bl = tmp / "tools" / "baselines"
    bl.mkdir(parents=True)
    (bl / "mutants-baseline.json").write_text(
        json.dumps({"missed": baseline}, ensure_ascii=False), encoding="utf-8"
    )
    for name, text in (logs or {}).items():
        (out_dir / "log" / name).write_text(text, encoding="utf-8")
    return out_dir


def run(tmp: pathlib.Path, *extra):
    return subprocess.run(
        [PY, "-X", "utf8", str(SCRIPT), "--root", str(tmp), *extra],
        capture_output=True,
        text=True,
    )


def section(out: str, label: str) -> list:
    """取某一段标签下面的逐键清单（脚本的契约就是"每类都点名"，所以按段解析）。"""
    lines = out.splitlines()
    idx = next(i for i, ln in enumerate(lines) if ln.strip().startswith(label))
    keys = []
    for ln in lines[idx + 1:]:
        if ln.startswith("    - "):
            keys.append(ln[6:].strip())
        else:
            break
    return keys


def test_four_lists_are_named_and_distinguished(tmp_path):
    """四种"看起来都能叫减债"的情形必须分开：新债 / 真被杀 / 只有超时 / 整条没测到。"""
    outcomes = [
        outcome("xtask/src/a.rs", "killed_now", "CaughtMutant"),
        outcome("xtask/src/a.rs", "still_alive", "MissedMutant"),
        outcome("xtask/src/b.rs", "only_unviable", "Unviable"),
        outcome("crates/x/src/c.rs", "brand_new_debt", "MissedMutant"),
        outcome("xtask/src/a.rs", "timed_out", "Timeout"),
    ]
    baseline = [
        "xtask/src/a.rs::killed_now",
        "xtask/src/a.rs::still_alive",
        "xtask/src/b.rs::only_unviable",
        "xtask/src/d.rs::vanished",
        "xtask/src/a.rs::timed_out",
    ]
    make_tree(tmp_path, outcomes, baseline)
    cp = run(tmp_path)
    assert cp.returncode == 1, cp.stdout

    # 只有"有 CaughtMutant 且无 MissedMutant"才算划账。
    assert section(cp.stdout, "可划账") == ["xtask/src/a.rs::killed_now"], cp.stdout
    # 全超时的在册键不许混进划账：它是"测试挂住"而不是"测试杀掉"的记账。
    assert section(cp.stdout, "只有超时") == ["xtask/src/a.rs::timed_out"], cp.stdout
    # 在册且本轮仍 missed 的键不算新债；只有新出现的存活键算。
    assert section(cp.stdout, "新债") == ["crates/x/src/c.rs::brand_new_debt"], cp.stdout
    # 全 Unviable 与整条没出现都不许当划账，但原因要分开写。
    assert section(cp.stdout, "不可验证") == [
        "xtask/src/b.rs::only_unviable（全 Unviable／无判定）",
        "xtask/src/d.rs::vanished（本轮没出现该键）",
    ], cp.stdout


def test_disk_shortfall_in_unviable_logs_is_named(tmp_path):
    """unviable 里有盘满签名 ⇒ 单列一类，不许把缺测读成"没问题"。"""
    outcomes = [
        outcome("xtask/src/a.rs", "link_failed", "Unviable"),
        outcome("xtask/src/a.rs", "compile_error", "Unviable"),
    ]
    logs = {
        "xtask_src_a.rs__link_failed.log": "ld.exe: final link failed: No space left on device\n",
        "xtask_src_a.rs__compile_error.log": "error[E0308]: mismatched types\n",
    }
    make_tree(tmp_path, outcomes, [], logs)
    cp = run(tmp_path)
    assert cp.returncode == 1, cp.stdout
    assert "盘满缺测：1 条" in cp.stdout, cp.stdout
    assert "No space left on device" in cp.stdout, cp.stdout


def test_keys_flag_proves_the_new_code_actually_died(tmp_path):
    """`--keys`：绿不等于新代码被杀。没杀掉/没测到都必须让它红。"""
    outcomes = [
        outcome("xtask/src/a.rs", "killed", "CaughtMutant"),
        outcome("xtask/src/a.rs", "alive", "MissedMutant"),
    ]
    make_tree(tmp_path, outcomes, ["xtask/src/a.rs::alive"])

    ok = run(tmp_path, "--keys", "xtask/src/a.rs::killed")
    assert "真被杀" in ok.stdout, ok.stdout

    bad = run(tmp_path, "--keys", "xtask/src/a.rs::alive")
    assert bad.returncode == 1, bad.stdout
    assert "未杀/没测到" in bad.stdout, bad.stdout

    absent = run(tmp_path, "--keys", "xtask/src/a.rs::never_ran")
    assert "计数=（无）" in absent.stdout, absent.stdout


def test_missing_artifacts_are_exit_two_not_zero(tmp_path):
    """产物或基线缺失一律 2：缺账不等于没债。"""
    empty = tmp_path / "empty"
    empty.mkdir()
    cp = run(empty)
    assert cp.returncode == 2, cp.stdout + cp.stderr
    assert "缺本轮产物" in cp.stdout, cp.stdout

    make_tree(tmp_path, [outcome("a.rs", "f", "CaughtMutant")], [])
    (tmp_path / "tools" / "baselines" / "mutants-baseline.json").unlink()
    cp2 = run(tmp_path)
    assert cp2.returncode == 2, cp2.stdout
    assert "缺基线" in cp2.stdout, cp2.stdout


def test_clean_round_reports_zero(tmp_path):
    """没有新债、没有不可验证、没有消失项 ⇒ 0（脚本只做核对，不替代判红）。"""
    outcomes = [
        outcome("xtask/src/a.rs", "alive", "MissedMutant"),
        outcome("xtask/src/a.rs", "killed", "CaughtMutant"),
    ]
    make_tree(tmp_path, outcomes, ["xtask/src/a.rs::alive"])
    cp = run(tmp_path)
    assert cp.returncode == 0, cp.stdout + cp.stderr
    assert "RECONCILE OK" in cp.stdout, cp.stdout
