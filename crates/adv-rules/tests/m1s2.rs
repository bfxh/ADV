//! M1 片2 判据：污点（源/汇/传播/净化）+ 导入感知 + 抑制四要素 + 到期账。

use adv_parse::Language;
use std::path::PathBuf;

fn rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../rules")
}

fn load() -> Vec<adv_rules::Rule> {
    adv_rules::load_rules(&rules_dir()).unwrap()
}

const TAINT: &str = r#"
import os
from os import system
from subprocess import run

def handler():
    s = input()
    eval(s)  # ruleid: PY-TAINT-EVAL PY-EVAL-USE
    t = s.strip()
    system(t)                   # ruleid: PY-TAINT-EVAL
    os.system("cmd " + s)       # ruleid: PY-TAINT-EVAL
    n = int(input())
    eval(n)  # ruleid: PY-EVAL-USE
    eval("1+1")  # ruleid: PY-EVAL-USE
    # from-import 裸调：导入感知解析命中
    run("ls", shell=True)  # ruleid: PY-SUBPROCESS-SHELL
"#;

const SUPPRESS: &str = r#"
import subprocess
def f(cmd, cmd2):
    subprocess.run(cmd, shell=True)  # adv:allow(PY-SUBPROCESS-SHELL, reason=内部可信命令白名单, until=2027-06-30)
    subprocess.run(cmd2, shell=True)  # ruleid: PY-SUBPROCESS-SHELL adv:allow(PY-SUBPROCESS-SHELL, reason=过期样例, until=2026-01-01)
"#;

#[test]
fn taint_sources_reach_sinks() {
    adv_rules::testing::check_fixture("taint", Language::Python, TAINT, &load()).unwrap();
}

#[test]
fn import_aware_resolution_works() {
    // `from subprocess import run` 后裸调 run(shell=True) 必须命中（片1 的 FN 面已闭）
    // `from os import system` 后污点 system(t) 命中 os.system 汇点——已并入 taint 夹具
    let rules = load();
    let ast = adv_parse::parse_source(Language::Python, SUPPRESS).unwrap();
    let f = adv_rules::matcher::run_matchers(
        &ast,
        std::path::Path::new("sup.py"),
        Language::Python,
        &rules,
        "2026-10-04",
    );
    // 第 4 行被有效抑制；第 5 行抑制过期 ⇒ 照报
    let lines: Vec<usize> = f.iter().map(|x| x.start_line).collect();
    assert_eq!(lines, vec![5], "有效抑制要吃掉发现，过期抑制要放行：{f:?}");
}

#[test]
fn suppression_four_elements_enforced() {
    let ast = adv_parse::parse_source(Language::Python, SUPPRESS).unwrap();
    let (ok, bad) = adv_rules::suppression::collect(&ast);
    assert_eq!(ok.len(), 2, "两条 adv:allow 都该解析成功：{ok:?}");
    assert!(bad.is_empty(), "不应有畸形抑制：{bad:?}");
    assert!(
        adv_rules::suppression::expired(&ast, "2026-10-04").len() == 1,
        "恰好一条到期"
    );
    assert!(
        adv_rules::suppression::expired(&ast, "2026-01-01").is_empty(),
        "回放到期前为零"
    );

    // 畸形样例：缺 until ⇒ 进畸形账
    let bad_src = "x = 1  # adv:allow(PY-EVAL-USE, reason=忘了日期)\n";
    let ast2 = adv_parse::parse_source(Language::Python, bad_src).unwrap();
    let (_, bad2) = adv_rules::suppression::collect(&ast2);
    assert_eq!(bad2.len(), 1);
    assert_eq!(bad2[0].why, "until 缺失或非 YYYY-MM-DD");
}

#[test]
fn match_shape_must_be_exclusive() {
    // call 与 taint 同时出现 = 加载即红（fail-closed）
    let tmp = std::env::temp_dir().join("adv-rules-shape-test");
    let _ = std::fs::create_dir_all(&tmp);
    let bad = tmp.join("both.yaml");
    std::fs::write(
        &bad,
        "id: X\nlanguages: [python]\nseverity: info\nmessage: m\nmatch:\n  call:\n    callee: a\n  taint:\n    sources: [b]\n    sinks: [c]\n",
    )
    .unwrap();
    assert!(adv_rules::load_rules(&tmp).is_err(), "双形态必须红");
    let _ = std::fs::remove_file(&bad);
}
