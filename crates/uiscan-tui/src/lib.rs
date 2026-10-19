//! TUI 面：把一段终端字节流喂进 `vt100` 的屏幕模型，读出"屏幕上真的出现了什么"。
//!
//! 宽度口径是这面最容易糊的地方，实测（vt100 0.16.2，2026-10-16）：中文主格 `is_wide()=true`
//! 且**下一格是 `is_wide_continuation()=true` 的空内容格**；全角拉丁 `ＡＢＣ` 同样双宽；
//! 而 `→ ← ✓ ✗` 判成**单宽**（就是 tmux/kitty 在案的那条 ambiguous-width 分歧）。
//! 所以：续格必须跳过而不是当空格切开；单格空白不能断词（否则 `混排 A 本` 被切成三段）。

use uiscan_core::{Channel, Harvest, Item};
use vt100::Parser;

/// 用固定尺 `(rows, cols)` 采集一段字节流的最终屏幕态。
pub fn harvest(bytes: &[u8], rows: u16, cols: u16) -> Harvest {
    let mut parser = Parser::new(rows, cols, 0);
    parser.process(bytes);
    harvest_screen(parser.screen(), rows, cols)
}

/// 拆出来单独可见：给定屏幕模型 → 行内连续文案段。
pub fn harvest_screen(screen: &vt100::Screen, rows: u16, cols: u16) -> Harvest {
    let mut items = Vec::new();
    for row in 0..rows {
        let mut run = String::new();
        let mut start: u16 = 0;
        let mut blanks: u16 = 0;
        for col in 0..cols {
            let Some(cell) = screen.cell(row, col) else {
                continue;
            };
            if cell.is_wide_continuation() {
                continue;
            }
            let ch = cell.contents();
            if ch.is_empty() || ch == " " {
                if !run.is_empty() {
                    blanks += 1;
                }
                continue;
            }
            if blanks >= 2 {
                items.push(Item::new(
                    run.trim_end(),
                    Channel::TermCell,
                    format!("r{row}c{start}"),
                ));
                run.clear();
                start = col;
            } else if blanks == 1 {
                run.push(' ');
            } else if run.is_empty() {
                start = col;
            }
            run.push_str(ch);
            blanks = 0;
        }
        if !run.trim().is_empty() {
            items.push(Item::new(
                run.trim_end(),
                Channel::TermCell,
                format!("r{row}c{start}"),
            ));
        }
    }
    Harvest {
        surface: format!("tui:{rows}x{cols} alt={}", screen.alternate_screen()),
        items,
    }
}
