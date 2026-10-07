# OSV 语料（M3-3a）

两份**真** advisory 的逐字切片 + 一份**真**列表响应切片。出处与抓取时间写在这里，测试引用它们时
不用再猜；任何一份若被编辑过，下面的 sha256 就对不上（本目录没有自动核对，但改的人必须同时改这里）。

| 文件 | 出处 | 抓取日 | sha256（前 16） | 字节 |
|---|---|---|---|---|
| `crates.io/RUSTSEC-2020-0071.json` | https://osv-vulnerabilities.storage.googleapis.com/crates.io/RUSTSEC-2020-0071.json | 2026-10-08 | c27b15523b116db6… | 4042 |
| `crates.io/GHSA-2226-4v3c-cff8.json` | https://osv-vulnerabilities.storage.googleapis.com/crates.io/GHSA-2226-4v3c-cff8.json | 2026-10-08 | 0df2f012f112f463… | 1442 |
| `crates.io/RUSTSEC-2021-0120.json` | https://osv-vulnerabilities.storage.googleapis.com/crates.io/RUSTSEC-2021-0120.json | 2026-10-08 | 76ce244978cf51cd… | 1482 |
| `listing-page.txt` | 同一桶的 `?list-type=2&prefix=crates.io/` 首页（裁到 2 个对象、保留真 token 形状） | 2026-10-08 | — | — |

**三条语料各钉一个边界**：
- RUSTSEC-2020-0071（`time`）：16 个事件的**多段区间**（8 段）+ `0.0.0-0` 哨兵；
- GHSA-2226-4v3c-cff8（`rustc-serialize`）：`last_affected` **闭**上界（0.3.24 在内、0.3.25 在外）；
- RUSTSEC-2021-0120（`abomonation`）：`informational = "unsound"` —— **在 `affected[].database_specific` 层**（顶层那个没有它），
  口径是「不是漏洞通告」⇒ 只出信号不判红。

**已实测的一条事实**：GCS 对普通对象的 `ETag` 就是内容的 MD5——两份语料的 `md5(文件)` 与列表里的
ETag 逐字相等（`d5b43c07c6a025c2be387b782f3f23e0` / `f680efa7a65d5dfb346031994837c67c`）。
`manifest.json` 就是按这条事实造的（etag == md5 才允许落账）。

`osv_lock/` 是配套的锁语料：`time 0.1.44`（命中）与 `rustc-serialize 0.3.25`（不许命中）。
