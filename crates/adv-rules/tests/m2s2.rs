//! M2 片2 判据：同文件跨函数污点传播（函数摘要）+ 作用域隔离。

use adv_parse::Language;
use std::path::Path;

fn rules() -> Vec<adv_rules::Rule> {
    adv_rules::load_rules(Path::new("../../rules")).unwrap()
}

const CROSS_FN: &str = r#"
import os
from os import system

def get_input():
    return input()

def wrap(x):
    return system(x)

def clean(x):
    return int(x)

def handler():
    s = get_input()
    system(s)  # ruleid: PY-TAINT-EVAL
    wrap(s)  # ruleid: PY-TAINT-EVAL

def ok():
    n = clean(input())
    system(n)

def unrelated():
    s = "literal"
    system(s)
"#;

#[test]
fn cross_function_taint_flows() {
    adv_rules::testing::check_fixture("cross_fn", Language::Python, CROSS_FN, &rules()).unwrap();
}

#[test]
fn scope_isolation_no_cross_leak() {
    // 作用域隔离：unrelated 里的同名变量 s 是字面量，不得吃 handler 的污点状态
    //（check_fixture 已覆盖：unrelated 的 system(s) 无注解无发现）
    let rules = rules();
    let ast = adv_parse::parse_source(Language::Python, CROSS_FN).unwrap();
    let f = adv_rules::run_matchers(
        &ast,
        std::path::Path::new("cross.py"),
        Language::Python,
        &rules,
        "2026-10-04",
    );
    assert_eq!(f.len(), 2, "恰好两条（handler 内）：{f:?}");
    assert_eq!(f[0].start_line, 16, "handler 的外部汇点行");
    assert_eq!(
        f[1].start_line, 17,
        "wrap 调用点行（参数入体内汇点报在调用点）"
    );
}

#[test]
fn returns_param_taint_path() {
    // 参数污点 → 返回值：passthrough(id) 形态
    let src = r#"
import os

def passthrough(x):
    return x

def handler():
    v = passthrough(input())
    os.system(v)  # ruleid: PY-TAINT-EVAL
"#;
    adv_rules::testing::check_fixture("returns_param", Language::Python, src, &rules()).unwrap();
}
