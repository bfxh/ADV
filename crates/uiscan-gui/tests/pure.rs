//! GUI 映射层的判决性测试。全部平台可跑：这层是纯函数，不碰 COM。
//! 期望值来自 2026-10-16 在 x86_64-pc-windows-gnu 上真跑 UIA 的读数
//! （顶层窗口里同时出现 `(-9999,-9999)` 屏外哨兵与 `(0,0)-(0,0)` 零尺寸窗口）。

use uiscan_core::{Channel, Rect};
use uiscan_gui::{control_type_name, item_from_uia, path_segment, rect_from_ltrb};

#[test]
fn measured_control_types_are_named_and_unknown_ones_are_not_guessed() {
    assert_eq!(
        control_type_name(50030),
        "Document",
        "实测：Yan Agent 的主窗口是 50030"
    );
    assert_eq!(control_type_name(50032), "Window");
    assert_eq!(control_type_name(50033), "Pane");
    assert_eq!(control_type_name(50000), "Button");
    // 不认得的必须点名原值，别悄悄映射成 "Unknown" 就完事
    assert_eq!(control_type_name(99999), "Unknown(99999)");
}

#[test]
fn path_segments_carry_index_and_truncated_name() {
    assert_eq!(path_segment(50032, 0, "青简状态条"), "Window[0]#青简状态条");
    let long = "一二三四五六七八九十一二三四五六七八九十一二三四五";
    let seg = path_segment(50033, 3, long);
    assert!(seg.starts_with("Pane[3]#"), "{seg}");
    assert_eq!(
        seg.chars().filter(|c| !c.is_ascii()).count(),
        24,
        "名字截到 24 个字符，免得一条 loc 撑爆报告：{seg}"
    );
    assert_eq!(
        path_segment(50033, 7, "  "),
        "Pane[7]",
        "没名字就只留类型+序号"
    );
}

#[test]
fn uia_rect_conversion_never_makes_negative_size() {
    // 实测的屏外哨兵窗口
    assert_eq!(
        rect_from_ltrb(-9999, -9999, -9998, -9998),
        Rect {
            x: -9999,
            y: -9999,
            w: 1,
            h: 1
        }
    );
    // 右<左 的畸形值不许造出负宽
    assert_eq!(
        rect_from_ltrb(10, 10, 5, 3),
        Rect {
            x: 10,
            y: 10,
            w: 0,
            h: 0
        }
    );
}

#[test]
fn empty_names_produce_no_item_but_nonempty_ones_do() {
    assert!(
        item_from_uia(
            "",
            "Window[0]",
            Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10
            },
            false
        )
        .is_none()
    );
    assert!(
        item_from_uia(
            "   ",
            "Window[0]",
            Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10
            },
            false
        )
        .is_none()
    );
    let it = item_from_uia(
        "保存",
        "Window[0]/Button[1]",
        Rect {
            x: 5,
            y: 5,
            w: 80,
            h: 24,
        },
        false,
    )
    .expect("有名节点要产出条目");
    assert_eq!(it.channel, Channel::UiaName);
    assert!(it.visible);
}

#[test]
fn offscreen_and_zero_size_are_both_invisible() {
    let r = Rect {
        x: 100,
        y: 100,
        w: 200,
        h: 40,
    };
    assert!(
        !item_from_uia("x", "p", r, true).unwrap().visible,
        "UIA 说 offscreen 就不算可见"
    );
    assert!(
        !item_from_uia(
            "x",
            "p",
            Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0
            },
            false
        )
        .unwrap()
        .visible,
        "零尺寸矩形不算可见（实测存在这种窗口）"
    );
    assert!(
        !item_from_uia(
            "x",
            "p",
            Rect {
                x: -9999,
                y: -9999,
                w: 1,
                h: 1
            },
            false
        )
        .unwrap()
        .visible,
        "屏外哨兵坐标不算可见"
    );
}

/// 冻结语料：2026-10-16 在真桌面用 `uiscan gui --title fmnr --json` 采到的 **egui 应用**树。
/// 这条钉的是"实测边界"本身：即时模式 GUI 的正文不在 UIA 里，只有窗口名 + 标题栏四个按钮。
#[test]
fn egui_window_exposes_only_os_chrome_in_uia() {
    let raw = include_str!("fixtures/gui-fmnr.json");
    let v: serde_json::Value = serde_json::from_str(raw).expect("语料是合法 JSON");
    let names: Vec<String> = v["items"]
        .as_array()
        .expect("items 是数组")
        .iter()
        .map(|i| i["txt"].as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(
        names,
        vec!["fmnr", "系统", "系统", "最小化", "最大化", "关闭"],
        "egui 正文若哪天出现在 UIA 里，这条会红——那说明 AccessKit 接上了，边界要重写"
    );
    assert!(
        v["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["ch"] == "UiaName"),
        "GUI 面的通道只有 UiaName"
    );
}

/// 冻结语料（fmnr，无 accesskit）⇒ 判"只有系统装饰"；带正文的合成树 ⇒ 判否。
/// 实测对照：eframe 0.31 加 accesskit 后同一窗口出现 `Text[1]#测试文本 中文`。
#[test]
fn chrome_only_detects_an_app_that_never_exposed_its_ui() {
    let raw = include_str!("fixtures/gui-fmnr.json");
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();
    let items: Vec<uiscan_core::Item> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            let ch = uiscan_core::Channel::from_wire(i["ch"].as_str().unwrap()).unwrap();
            uiscan_core::Item::new(i["txt"].as_str().unwrap(), ch, i["loc"].as_str().unwrap())
        })
        .collect();
    let h = uiscan_core::Harvest {
        surface: "gui:uia".into(),
        items,
    };
    assert!(
        uiscan_gui::chrome_only(&h),
        "只有标题栏 ⇒ 必须判 chrome_only"
    );

    let ok = uiscan_core::Harvest {
        surface: "gui:uia".into(),
        items: vec![
            uiscan_core::Item::new(
                "akprobe",
                uiscan_core::Channel::UiaName,
                "Window[0]#akprobe",
            ),
            uiscan_core::Item::new(
                "测试文本 中文",
                uiscan_core::Channel::UiaName,
                "Window[0]#akprobe/Text[1]#测试文本 中文",
            ),
        ],
    };
    assert!(
        !uiscan_gui::chrome_only(&ok),
        "有正文节点就不许判 chrome_only"
    );
    assert!(
        !uiscan_gui::chrome_only(&uiscan_core::Harvest::default()),
        "空采集不算 chrome_only"
    );
}
