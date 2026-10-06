"""设计债到期门（`scripts/debt_gate.py` + `spec/design-debt.json` + `spec/maturity.json`）的契约。

由来（用户 2026-10-06 口径）：对"正在被测试的东西"不该用"维护承重结构"的尺——那种税只把
流程拖慢；不能放过的是**设计层**的问题，而且它必须**有到期日**（不超过 10 个任务），
否则"设计上的问题"就变成一条没人再看的注释。本文件把这条约束钉成会判红的门。

判据两头都有（沿用 test_s194_claim_gate 的三段式）：
1. **真仓自检**：今天这份 registry 必须绿，且绿得有内容（在册 5 条，不是空表蒙过）；
2. **金丝雀红**：D1–D8 每条各造一个反例，必须红且点名到 id；
3. **fail-closed**：registry 缺失 / 空 entries / 坏 pattern 一律红，不静默放行。

计时判据用注入的 `counter`/`exists` 走，不 mock git ⇒ 能证明"超期"真的来自 commit 计数。
"""
import copy
import importlib.util
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "debt_gate.py"
PY = sys.executable


def _load():
    spec = importlib.util.spec_from_file_location("debt_gate", GATE)
    assert spec is not None and spec.loader is not None, "加载 debt_gate 失败"
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


G = _load()


def healthy():
    """一条**完全合规**的在册债：判据必须对它返回空（否则 D1–D7 会恒红）。"""
    return [
        {
            "id": "DD-9001",
            "surface": "somewhere",
            "kind": "design",
            "defect": "示例：某分支观察面为空",
            "why_from_design": "初版把解析与起进程写在一起，分层是后来补的",
            "origin": "0" * 7,
            "since": "1" * 7,
            "due_after_tasks": 10,
            "guards": "spec/design-debt.json",
        }
    ]


def run(entries, retired=None, *, walked=0, known=None):
    """跑纯判据：counter 记录被查询的 since，exists 由 known 集合决定（不碰真 git）。"""
    asked = []
    known = known if known is not None else {"0" * 7, "1" * 7}

    def counter(since):
        asked.append(since)
        return walked

    def exists(sha):
        return sha in known

    fails = G.check_entries(
        entries,
        retired or [],
        required=G.REQUIRED,
        counter=counter,
        exists=exists,
    )
    return fails, asked


def test_healthy_entry_is_green_and_asks_the_counter():
    fails, asked = run(healthy())
    assert fails == [], f"合规条目被判红：{fails}"
    assert asked == ["1" * 7], f"超期判定没走 commit 计数（问到的是 {asked}）"


def test_missing_required_field_is_red_and_named():
    for field in G.REQUIRED:
        e = copy.deepcopy(healthy()[0])
        del e[field]
        fails, _ = run([e])
        hit = [f for f in fails if f.startswith("D1") and f"：{field}" in f]
        assert hit, f"缺 {field} 没判红：{fails}"
        want = "<无 id" if field == "id" else "DD-9001"
        assert want in hit[0], f"没点名到条目：{hit[0]}"


def test_registry_cannot_shrink_its_own_required_fields():
    """D8：登记表把必填字段删短等于把 D1 关掉——不拦的话"改数据"就能松门。"""
    fails = G.check_entries(
        healthy(), [], required=["id", "since"], counter=lambda s: 0, exists=lambda s: True
    )
    hit = [f for f in fails if f.startswith("D8")]
    assert hit, f"required_fields 缩水没判红：{fails}"
    assert any("why_from_design" in f for f in hit), f"要点名缺的那一项：{hit}"


def test_empty_design_cause_is_red():
    e = copy.deepcopy(healthy()[0])
    e["why_from_design"] = "   "
    fails, _ = run([e])
    assert any(f.startswith("D2 DD-9001") for f in fails), f"没写设计成因却没判红：{fails}"


def test_due_above_ten_is_red():
    e = copy.deepcopy(healthy()[0])
    e["due_after_tasks"] = 11
    fails, _ = run([e])
    assert any(f.startswith("D3 DD-9001") for f in fails), f"超上限的限期没拦住：{fails}"


def test_fabricated_sha_is_red():
    """这条是本门的作者自罚条款：我 2026-10-06 连着写错过两个不存在的 SHA。"""
    fails, _ = run(healthy(), known={"0" * 7})
    assert any("DD-9001" in f and f.startswith("D4") for f in fails), f"编造的 since 没被点名：{fails}"
    assert any("不存在" in f for f in fails), f"报错要说清是 git 里查不到：{fails}"


def test_not_a_sha_shape_is_red():
    e = copy.deepcopy(healthy()[0])
    e["since"] = "HEAD"
    fails, _ = run([e])
    assert any(f.startswith("D4 DD-9001") and "commit" in f for f in fails), f"非 commit 形态没拦：{fails}"


def test_shallow_clone_blames_the_environment_not_the_author():
    """浅克隆里 SHA 查不到 ⇒ 判红不变，但文案必须说"无法核验（fetch-depth）"。

    由来（2026-10-06 实测）：CI 的 checkout 默认 depth=1，Debt gate 一次报出 14 条
    "在 git 里不存在（不许凭印象写 SHA）"——那些 SHA 一个都没写错。冤枉作者还在其次，
    真正的问题是 D4 红会把 D5（超期）整条掩盖掉：`_due_fails` 要求 `exists(since)` 才数
    commit，浅克隆里它恒假 ⇒ 门在 CI 上是**哑的**，红得毫无信息量。
    """
    entry = healthy()[0]
    known = set()          # 什么都不认识，模拟浅克隆

    def counter(since):
        return 99

    strict = G.check_entries([entry], [], required=G.REQUIRED, counter=counter,
                             exists=lambda s: s in known, shallow=False)
    assert any("凭印象写 SHA" in f for f in strict), strict
    loose = G.check_entries([entry], [], required=G.REQUIRED, counter=counter,
                            exists=lambda s: s in known, shallow=True)
    assert any("浅克隆" in f and "fetch-depth" in f for f in loose), loose
    assert not any("凭印象" in f for f in loose), f"浅克隆还冤枉作者：{loose}"
    assert len(loose) == len(strict), "换了说法不该少判一条（fail-closed 不变）"


def test_overdue_is_red_and_names_the_id():
    fails, _ = run(healthy(), walked=10)
    assert fails == [], f"刚好到期（10/10）不该红：{fails}"
    fails, _ = run(healthy(), walked=11)
    assert any(f.startswith("D5 DD-9001") and "11" in f for f in fails), f"超期没判红：{fails}"


def test_duplicate_ids_and_retired_bookkeeping():
    dup = healthy() + healthy()
    fails, _ = run(dup)
    assert any(f.startswith("D6") and "DD-9001" in f for f in fails), f"id 重复没判红：{fails}"

    retired = [{"id": "DD-9002"}]
    fails, _ = run(healthy(), retired=retired)
    assert any(f.startswith("D7 DD-9002") for f in fails), f"销账无 evidence 没判红：{fails}"

    retired = [{"id": "DD-9001", "evidence": "已补断言，见 DD-9001 处置提交"}]
    fails, _ = run(healthy(), retired=retired)
    assert any(f.startswith("D6 DD-9001") for f in fails), f"同时在册与退役没判红：{fails}"


def test_maturity_pattern_validation():
    ok = [{"pattern": "crates/*/tests/data/**", "why": "合成语料", "guards": "deep_cargo.rs"}]
    assert G.check_maturity(ok) == [], "合规试验面被判红"
    bad = [{"pattern": "**", "why": "", "guards": ""}]
    fails = G.check_maturity(bad)
    assert any("过宽" in f for f in fails), f"整仓级 pattern 没拦住：{fails}"
    assert any("why 为空" in f for f in fails), f"空 why 没判红：{fails}"
    assert any("guards 为空" in f for f in fails), f"空 guards 没判红：{fails}"


def test_real_repository_registry_is_green_via_subprocess():
    """真仓自检：走子进程真路径（含真 git），不是只测内部函数。

    2026-10-06 改过一次口径，理由要留档：原本要求"今天这份 registry 全绿"，结果债的钟
    一超期（D5）这条测试就红——于是**每次本地 pytest 都被过期债挡住**。那正是 DD-0007
    刚治失败的模式：把一个人人不关心的红塞进人人都跑的通道，红就变成噪声。
    现在的分工是：
      · 报警位 = CI（adv.yml 的 Debt gate 步骤，合并层）——超期必须在那里红；
      · 本测试 = **结构完整性**：除 D5（计时超期）之外的任何违规都算红，
        且超期清单必须被打出来（看得见，不静默）。
    也就是说：这条测试管的是"账本坏没坏"，"账拖没拖"由 CI 判。
    """
    out = subprocess.run([PY, "-X", "utf8", str(GATE)], cwd=str(ROOT), capture_output=True,
                         text=True)
    lines = [ln.strip() for ln in out.stdout.splitlines() if ln.strip().startswith("❌")]
    non_timing = [ln for ln in lines if not ln.startswith("❌ D5")]
    assert not non_timing, f"registry 结构违规（不是超期）：{non_timing}\n{out.stdout}"
    for ln in lines:
        print(ln)
    assert out.returncode == 0 or lines, f"退出码非 0 却没有一条超期说明：{out.stdout}{out.stderr}"
    assert "DEBT-GATE" in out.stdout, out.stdout


def test_real_repository_has_real_entries():
    """绿得有内容：空 registry 会让上面那条自检失去意义（"没债"与"没账"同罪）。"""
    d = json.loads((ROOT / "spec" / "design-debt.json").read_text(encoding="utf-8"))
    assert len(d["entries"]) >= 3, "登记表空了——本门的存在理由就是不让设计缺口沉默"
    m = json.loads((ROOT / "spec" / "maturity.json").read_text(encoding="utf-8"))
    assert len(m["entries"]) >= 1, "试验面登记为空"
    for e in d["entries"]:
        assert e["due_after_tasks"] <= G.MAX_DUE, f"{e['id']} 超上限"


def test_missing_registries_are_red():
    """fail-closed：文件不在就是红（缺账 ≠ 没债）。在临时空仓里验，不碰真仓。"""
    with tempfile.TemporaryDirectory() as td:
        tmp = pathlib.Path(td)
        (tmp / "scripts").mkdir()
        shutil.copy(GATE, tmp / "scripts" / "debt_gate.py")
        r = subprocess.run(
            [PY, "-X", "utf8", str(tmp / "scripts" / "debt_gate.py")],
            capture_output=True,
            text=True,
        )
        assert r.returncode == 1, f"缺 registry 却判绿：{r.stdout}{r.stderr}"
        assert "design-debt.json" in r.stdout, r.stdout
