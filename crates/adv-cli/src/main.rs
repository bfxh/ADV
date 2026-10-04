//! ADV CLI 入口（M0 骨架：仅身份输出；子命令随 M1+ 落地）。

fn main() {
    println!("{} {}", adv_core::NAME, adv_core::version());
}
