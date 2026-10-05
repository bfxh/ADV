//! 片A2：MIR 级污点到达分析（`rustc_mir_dataflow::framework::Analysis` 实现）。
//!
//! 分工：callee 路径的解析必须在拿得到 `tcx` 的环节做完（`build_plan`），
//! 框架的 `apply_*_effect` 只给 state/statement/terminator/location，拿不到 `tcx`
//! ⇒ 分析阶段只查预计算好的 Location→CallPlan 表。
//!
//! 口径与快轨同源且同样保守（见 `adv_ast_rust::spec` 模块头）：源点产污、
//! 净化工杀灭、**只有声明过的 propagators 传播**，未声明的调用一律杀目的局部。

use adv_ast_rust::spec::{Spec, callee_matches};
use rustc_hir::def_id::LocalDefId;
use rustc_index::bit_set::DenseBitSet;
use rustc_middle::mir::{
    self, BasicBlock, Body, Location, Operand, Rvalue, StatementKind, TerminatorKind,
};
use rustc_middle::ty::TyCtxt;
use rustc_mir_dataflow::{Analysis, Forward};
use std::cell::RefCell;
use std::collections::HashMap;

/// 实参对本分析的可见形态：某个局部，或常量（常量不带污点）。
#[derive(Clone, Copy, Debug)]
pub enum ArgKind {
    /// 局部变量索引（`Local::index()`）。
    Local(usize),
    /// 常量/其他操作数。
    Other,
}

/// 一个调用点预计算后的计划。
#[derive(Clone, Debug)]
pub struct CallPlan {
    /// callee 的 `def_path_str`（实测形态可能带 `::<T, E>` 段，故匹配走后缀档）。
    pub callee: String,
    /// 返回值写入的局部索引。
    pub dest: usize,
    /// 实参局部索引。
    pub args: Vec<ArgKind>,
    /// 调用点所在源码行（1 起，来自 terminator 的 span）——finding 要能锚到行，
    /// 否则对不上仓里"ruleid 注释逐行一致"的夹具门禁口径。
    pub line: usize,
}

/// 单个函数的分析输入：调用点表 + 局部数。
pub struct Plan {
    /// 函数名（进 finding）。
    pub function: String,
    /// Location → 调用计划。
    pub calls: HashMap<Location, CallPlan>,
    /// `local_decls` 长度（bitset 宽度）。
    pub n_locals: usize,
}

/// 一条命中（片A2 形态；与快轨 schema 对齐是片A3 的事）。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hit {
    /// 规则 ID。
    pub rule: String,
    /// 函数名。
    pub function: String,
    /// 汇点所在 `bbN.M`。
    pub location: String,
    /// 汇点源码行（1 起）。
    pub line: usize,
}

fn arg_kind(op: &Operand<'_>) -> ArgKind {
    match op {
        Operand::Copy(place) | Operand::Move(place) => ArgKind::Local(place.local.index()),
        Operand::Constant(_) | Operand::RuntimeChecks(_) => ArgKind::Other,
    }
}

/// 在有 `tcx` 的环节把函数体扫成计划表。
pub fn build_plan(tcx: TyCtxt<'_>, def: LocalDefId, function: &str) -> Plan {
    let body = tcx.optimized_mir(def);
    let mut calls = HashMap::new();
    for (bb, data) in body.basic_blocks.iter().enumerate() {
        let Some(term) = &data.terminator else {
            continue;
        };
        let TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } = &term.kind
        else {
            continue;
        };
        let Some(callee) = crate::callee_path(tcx, func) else {
            continue;
        };
        calls.insert(
            Location {
                block: BasicBlock::from_usize(bb),
                statement_index: data.statements.len(),
            },
            CallPlan {
                callee,
                dest: destination.local.index(),
                args: args.iter().map(|a| arg_kind(&a.node)).collect(),
                line: tcx
                    .sess
                    .source_map()
                    .lookup_char_pos(term.source_info.span.lo())
                    .line,
            },
        );
    }
    Plan {
        function: function.to_string(),
        calls,
        n_locals: body.local_decls.len(),
    }
}

/// 污点到达分析：Domain = 带污点的局部集合（前向）。
struct Reach<'a> {
    specs: &'a [Spec],
    plan: &'a Plan,
    hits: &'a RefCell<Vec<Hit>>,
}

impl<'a, 'tcx> Analysis<'tcx> for Reach<'a> {
    // 1.99 的 Analysis 还要求 NAME（框架用它区分同一分析的不同轮次）。
    const NAME: &'static str = "adv-mir-taint";

    type Domain = DenseBitSet<mir::Local>;
    type Direction = Forward;

    fn bottom_value(&self, _body: &Body<'tcx>) -> Self::Domain {
        DenseBitSet::new_empty(self.plan.n_locals)
    }

    fn initialize_start_block(&self, _body: &Body<'tcx>, _state: &mut Self::Domain) {}

    fn apply_primary_statement_effect(
        &self,
        state: &mut Self::Domain,
        stmt: &mir::Statement<'tcx>,
        _loc: Location,
    ) {
        let StatementKind::Assign(assign) = &stmt.kind else {
            return;
        };
        let (place, rvalue) = &**assign;
        let dest = place.local;
        let carried = match rvalue {
            Rvalue::Use(op, _) | Rvalue::UnaryOp(_, op) => tainted_operand(state, op),
            Rvalue::BinaryOp(_, operands) => {
                let (a, b) = &**operands;
                tainted_operand(state, a) || tainted_operand(state, b)
            }
            Rvalue::Ref(_, _, p) | Rvalue::RawPtr(_, p) | Rvalue::CopyForDeref(p) => {
                state.contains(p.local)
            }
            Rvalue::Cast(_, op, _) | Rvalue::Repeat(op, _) => tainted_operand(state, op),
            // 其余形态（Aggregate/Discriminant/Downcast/Len/ThreadLocalRef 等）本片不追——
            // 保守杀目的局部，FN 面如实进 FP 账。
            _ => false,
        };
        if carried {
            state.insert(dest);
        } else {
            state.remove(dest);
        }
    }

    fn apply_primary_terminator_effect(
        &self,
        state: &mut Self::Domain,
        term: &mir::Terminator<'tcx>,
        loc: Location,
    ) {
        if !matches!(term.kind, TerminatorKind::Call { .. }) {
            return;
        }
        let Some(call) = self.plan.calls.get(&loc) else {
            return;
        };
        let tainted_arg = call.args.iter().any(|a| match a {
            ArgKind::Local(i) => state.contains(mir::Local::from_usize(*i)),
            ArgKind::Other => false,
        });
        for spec in self.specs {
            if callee_matches(&call.callee, &spec.sinks) && tainted_arg {
                self.hits.borrow_mut().push(Hit {
                    rule: spec.id.clone(),
                    function: self.plan.function.clone(),
                    location: format!("bb{}.{}", loc.block.index(), loc.statement_index),
                    line: call.line,
                });
            }
        }
        if callee_matches(&call.callee, &self.all_sources()) {
            state.insert(mir::Local::from_usize(call.dest));
        } else if callee_matches(&call.callee, &self.all_sanitizers()) {
            state.remove(mir::Local::from_usize(call.dest));
        } else if callee_matches(&call.callee, &self.all_propagators()) {
            if tainted_arg {
                state.insert(mir::Local::from_usize(call.dest));
            } else {
                state.remove(mir::Local::from_usize(call.dest));
            }
        } else {
            state.remove(mir::Local::from_usize(call.dest));
        }
    }
}

fn tainted_operand(state: &DenseBitSet<mir::Local>, op: &Operand<'_>) -> bool {
    matches!(arg_kind(op), ArgKind::Local(i) if state.contains(mir::Local::from_usize(i)))
}

/// 规则四要素的展平视图（深轨按"任一规则声明即生效"处理）。
fn flat_pats(specs: &[Spec], get: impl Fn(&Spec) -> &Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for spec in specs {
        out.extend(get(spec).iter().cloned());
    }
    out
}

impl Reach<'_> {
    fn all_sources(&self) -> Vec<String> {
        flat_pats(self.specs, |s| &s.sources)
    }
    fn all_sanitizers(&self) -> Vec<String> {
        flat_pats(self.specs, |s| &s.sanitizers)
    }
    fn all_propagators(&self) -> Vec<String> {
        flat_pats(self.specs, |s| &s.propagators)
    }
}

/// 对所有函数跑一遍到达分析，返回去重排序后的命中。
pub fn analyze(tcx: TyCtxt<'_>, specs: &[Spec]) -> Vec<Hit> {
    let hits = RefCell::new(Vec::new());
    for &def in tcx.mir_keys(()).iter() {
        let function = tcx.def_path_str(def);
        let plan = build_plan(tcx, def, &function);
        let body = tcx.optimized_mir(def);
        let _results = Reach {
            specs,
            plan: &plan,
            hits: &hits,
        }
        .iterate_to_fixpoint(tcx, body, Some("adv-mir-taint"));
    }
    let mut out = hits.take();
    out.sort();
    out.dedup();
    out
}
