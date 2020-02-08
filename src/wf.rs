extern crate rustc_data_structures;
extern crate rustc_index;

use super::syntax::ast::*;
use super::syntax::visit::{self, Visitor};
use crate::context::{ErrorReported, LiquidRustCtxt};
use rustc::infer::unify_key::ToType;
use rustc::ty::{self, Ty, TyCtxt, TyKind, TypeckTables};
use rustc_data_structures::unify::{InPlace, UnificationTable};
use rustc_span::MultiSpan;
use std::collections::HashMap;
use std::ops::Deref;

pub type TypeckTable<'tcx> = HashMap<ExprId, Ty<'tcx>>;

pub fn check_wf<'a, 'tcx>(
    cx: &LiquidRustCtxt<'a, 'tcx>,
    annots: &Vec<FnAnnots>,
) -> Result<TypeckTable<'tcx>, ErrorReported> {
    let mut expr_tys = TypeckTable::new();
    cx.track_errors(|| {
        for fn_annots in annots {
            check_fn_annots(cx, fn_annots, &mut expr_tys);
        }
        expr_tys
    })
}

fn check_fn_annots<'a, 'tcx>(
    cx: &'a LiquidRustCtxt<'a, 'tcx>,
    fn_annots: &FnAnnots,
    expr_tys: &mut TypeckTable<'tcx>,
) {
    let def_id = cx.hir().body_owner_def_id(fn_annots.body_id);
    let tables = cx.tcx().typeck_tables_of(def_id);
    let hir_id = cx.hir().as_local_hir_id(def_id).unwrap();
    let ret_ty = tables.liberated_fn_sigs()[hir_id].output();
    let mut checker = TypeChecker::new(cx, tables, ret_ty, expr_tys);

    if let Some(fn_typ) = &fn_annots.fn_ty {
        check_fn_ty(cx, fn_typ, &mut checker);
    }
    for refine in fn_annots.locals.values() {
        check_refine(cx, refine, &mut checker);
    }
}

fn check_fn_ty<'a, 'tcx>(
    cx: &'a LiquidRustCtxt<'a, 'tcx>,
    fn_ty: &FnType,
    checker: &mut TypeChecker<'a, 'tcx>,
) {
    for input in &fn_ty.inputs {
        check_refine(cx, input, checker);
    }
    check_refine(cx, &fn_ty.output, checker);
}

fn check_refine<'a, 'tcx>(
    cx: &'a LiquidRustCtxt<'a, 'tcx>,
    refine: &Refine,
    checker: &mut TypeChecker<'a, 'tcx>,
) {
    let ty = checker.infer_expr(&refine.pred);
    checker.resolve_inferred_types(&refine.pred);
    if ty.kind != TyKind::Bool && ty.kind != TyKind::Error {
        lint_malformed_refinement(cx, refine, ty);
    }
}
struct TypeChecker<'a, 'tcx> {
    cx: &'a LiquidRustCtxt<'a, 'tcx>,
    tcx: TyCtxt<'tcx>,
    tables: &'a TypeckTables<'tcx>,
    ret_ty: Ty<'tcx>,
    expr_tys: &'a mut TypeckTable<'tcx>,
    infer_ctxt: InferCtxt<'tcx>,
}

impl<'a, 'tcx> TypeChecker<'a, 'tcx> {
    pub fn new(
        cx: &'a LiquidRustCtxt<'a, 'tcx>,
        tables: &'a TypeckTables<'tcx>,
        ret_ty: Ty<'tcx>,
        expr_tys: &'a mut TypeckTable<'tcx>,
    ) -> Self {
        Self {
            cx,
            tcx: cx.tcx(),
            tables,
            ret_ty,
            expr_tys,
            infer_ctxt: InferCtxt::new(cx.tcx()),
        }
    }

    fn lookup(&self, name: Name) -> Ty<'tcx> {
        match name.hir_res {
            HirRes::Binding(hir_id) => self.tables.node_type(hir_id),
            HirRes::ReturnValue => self.ret_ty,
            HirRes::Unresolved => bug!("names must be resolved"),
        }
    }

    fn infer_expr(&mut self, expr: &Expr) -> Ty<'tcx> {
        let ty = match &expr.kind {
            ExprKind::Lit(lit) => self.infer_lit(lit),
            ExprKind::Binary(e1, op, e2) => self.infer_bin_op(e1, *op, e2),
            ExprKind::Name(name) => self.lookup(*name),
            ExprKind::Unary(op, e) => self.infer_un_op(*op, e),
            ExprKind::Err => self.types.err,
        };
        self.expr_tys.insert(expr.expr_id, ty);
        ty
    }

    fn infer_lit(&mut self, lit: &Lit) -> Ty<'tcx> {
        match lit.node {
            LitKind::Str(..) => self.mk_static_str(),
            LitKind::ByteStr(ref v) => self.mk_imm_ref(
                self.lifetimes.re_static,
                self.mk_array(self.types.u8, v.len() as u64),
            ),
            LitKind::Byte(_) => self.types.u8,
            LitKind::Char(_) => self.types.char,
            LitKind::Int(_, LitIntType::Signed(t)) => self.mk_mach_int(t),
            LitKind::Int(_, LitIntType::Unsigned(t)) => self.mk_mach_uint(t),
            LitKind::Int(_, LitIntType::Unsuffixed) => self.infer_ctxt.next_int_var(),
            LitKind::Float(_, LitFloatType::Suffixed(t)) => self.mk_mach_float(t),
            LitKind::Float(_, LitFloatType::Unsuffixed) => self.infer_ctxt.next_float_var(),
            LitKind::Bool(_) => self.types.bool,
            LitKind::Err(_) => self.types.err,
        }
    }

    fn infer_un_op(&mut self, op: UnOp, e: &Expr) -> Ty<'tcx> {
        let ty = self.infer_expr(e);
        if ty.kind == TyKind::Error {
            return ty;
        }

        match (op.kind, &ty.kind) {
            (UnOpKind::Deref, TyKind::Ref(_, ty, _)) => ty,
            (UnOpKind::Not, TyKind::Bool) => ty,
            _ => {
                lint_un_op_err(self.cx, op, e, ty);
                self.types.err
            }
        }
    }

    fn infer_bin_op(&mut self, e1: &Expr, op: BinOp, e2: &Expr) -> Ty<'tcx> {
        let ty1 = self.infer_expr(e1);
        let ty2 = self.infer_expr(e2);
        if ty1.kind == TyKind::Error || ty2.kind == TyKind::Error {
            return self.types.err;
        }

        match op.kind {
            BinOpKind::Lt | BinOpKind::Gt | BinOpKind::Eq | BinOpKind::Ge => {
                match self.infer_ctxt.unify(ty1, ty2) {
                    Some(ty) if ty.is_numeric() => self.mk_bool(),
                    _ => {
                        lint_bin_op_err(self.cx, op, e1, ty1, e2, ty2);
                        self.types.err
                    }
                }
            }

            BinOpKind::Mul | BinOpKind::Div | BinOpKind::Add | BinOpKind::Sub => {
                match self.infer_ctxt.unify(ty1, ty2) {
                    Some(ty) if ty.is_numeric() => ty,
                    _ => {
                        lint_bin_op_err(self.cx, op, e1, ty1, e2, ty2);
                        self.types.err
                    }
                }
            }

            BinOpKind::And | BinOpKind::Or => {
                lint_expected_found(self.cx, e1, self.mk_bool(), ty1);
                lint_expected_found(self.cx, e2, self.mk_bool(), ty2);
                if ty1.is_bool() && ty2.is_bool() {
                    self.mk_bool()
                } else {
                    self.types.err
                }
            }
        }
    }

    fn resolve_inferred_types(&mut self, expr: &Expr) {
        self.visit_expression(expr);
    }
}

impl<'a, 'tcx> Deref for TypeChecker<'a, 'tcx> {
    type Target = ty::TyCtxt<'tcx>;

    fn deref(&self) -> &Self::Target {
        &self.tcx
    }
}

impl<'a, 'tcx> Visitor<'a> for TypeChecker<'a, 'tcx> {
    fn visit_expression(&mut self, expr: &Expr) {
        let ty = self.expr_tys.get(&expr.expr_id).unwrap();
        if_chain! {
            if let ty::Infer(infer_ty) = ty.kind;
            if let Some(inferred_ty) = self.infer_ctxt.type_inferred_for(infer_ty);
            then {
                self.expr_tys.insert(expr.expr_id, inferred_ty);
            }
        }
        visit::walk_expression(self, expr);
    }
}

struct InferCtxt<'tcx> {
    tcx: TyCtxt<'tcx>,
    int_unification_table: UnificationTable<InPlace<ty::IntVid>>,
    float_unification_table: UnificationTable<InPlace<ty::FloatVid>>,
}

impl<'tcx> InferCtxt<'tcx> {
    fn new(tcx: TyCtxt<'tcx>) -> Self {
        InferCtxt {
            tcx,
            int_unification_table: UnificationTable::new(),
            float_unification_table: UnificationTable::new(),
        }
    }

    fn type_inferred_for(&mut self, var: ty::InferTy) -> Option<Ty<'tcx>> {
        match var {
            ty::IntVar(vid) => self
                .int_unification_table
                .probe_value(vid)
                .map(|v| v.to_type(self.tcx)),
            ty::FloatVar(vid) => self
                .float_unification_table
                .probe_value(vid)
                .map(|v| v.to_type(self.tcx)),
            _ => None,
        }
    }

    fn next_int_var_id(&mut self) -> ty::IntVid {
        self.int_unification_table.new_key(None)
    }

    fn next_float_var_id(&mut self) -> ty::FloatVid {
        self.float_unification_table.new_key(None)
    }

    fn unify(&mut self, ty1: Ty<'tcx>, ty2: Ty<'tcx>) -> Option<Ty<'tcx>> {
        if ty1 == ty2 {
            return Some(ty1);
        }
        match (&ty1.kind, &ty2.kind) {
            (&ty::Infer(ty::IntVar(vid1)), &ty::Infer(ty::IntVar(vid2))) => self
                .int_unification_table
                .unify_var_var(vid1, vid2)
                .map(|_| ty1)
                .ok(),
            (&ty::Int(int_ty), &ty::Infer(ty::IntVar(vid))) => {
                self.unify_integral(vid, ty::IntType(int_ty))
            }
            (&ty::Infer(ty::IntVar(vid)), &ty::Int(int_ty)) => {
                self.unify_integral(vid, ty::IntType(int_ty))
            }
            (&ty::Uint(uint_ty), &ty::Infer(ty::IntVar(vid))) => {
                self.unify_integral(vid, ty::UintType(uint_ty))
            }
            (&ty::Infer(ty::IntVar(vid)), &ty::Uint(uint_ty)) => {
                self.unify_integral(vid, ty::UintType(uint_ty))
            }
            (&ty::Float(float_ty), &ty::Infer(ty::FloatVar(vid))) => {
                self.unify_float(vid, ty::FloatVarValue(float_ty))
            }
            (&ty::Infer(ty::FloatVar(vid)), &ty::Float(float_ty)) => {
                self.unify_float(vid, ty::FloatVarValue(float_ty))
            }
            _ => None,
        }
    }

    fn unify_integral(&mut self, vid: ty::IntVid, val: ty::IntVarValue) -> Option<Ty<'tcx>> {
        self.int_unification_table
            .unify_var_value(vid, Some(val))
            .map(|_| match val {
                ty::IntType(v) => self.tcx.mk_mach_int(v),
                ty::UintType(v) => self.tcx.mk_mach_uint(v),
            })
            .ok()
    }

    fn unify_float(&mut self, vid: ty::FloatVid, val: ty::FloatVarValue) -> Option<Ty<'tcx>> {
        self.float_unification_table
            .unify_var_value(vid, Some(val))
            .map(|_| self.tcx.mk_mach_float(val.0))
            .ok()
    }

    fn next_int_var(&mut self) -> Ty<'tcx> {
        self.tcx.mk_int_var(self.next_int_var_id())
    }

    fn next_float_var(&mut self) -> Ty<'tcx> {
        self.tcx.mk_float_var(self.next_float_var_id())
    }
}

fn lint_malformed_refinement(cx: &LiquidRustCtxt, refine: &Refine, ty: Ty) {
    let b = cx.tcx().types.bool;
    let mut mspan = MultiSpan::from_span(refine.pred.span);
    mspan.push_span_label(
        refine.pred.span,
        format!("expected `{}`, found `{}`", b, ty),
    );
    cx.span_lint(
        mspan,
        &format!("refinement predicate must be of type `{}`", b),
    );
}

fn lint_expected_found(cx: &LiquidRustCtxt, e: &Expr, expected: Ty, found: Ty) {
    if expected == found {
        return;
    }
    let mut spans = MultiSpan::from_span(e.span);
    spans.push_span_label(
        e.span,
        format!("expected `{}`, found `{}`", expected, found),
    );
    cx.span_lint(spans, "mismatched types")
}

fn lint_un_op_err(cx: &LiquidRustCtxt, op: UnOp, e: &Expr, ty: Ty) {
    cx.span_lint_label(op.span.to(e.span), &un_op_err_msg(op, ty));
}

fn lint_bin_op_err<'tcx>(
    cx: &LiquidRustCtxt,
    op: BinOp,
    e1: &Expr,
    ty1: Ty<'tcx>,
    e2: &Expr,
    ty2: Ty<'tcx>,
) {
    let mut mspan = MultiSpan::from_span(op.span);
    mspan.push_span_label(e1.span, format!("{}", ty1));
    mspan.push_span_label(e2.span, format!("{}", ty2));
    cx.span_lint(mspan, &bin_op_err_msg(ty1, op, ty2));
}

fn un_op_err_msg<'tcx>(op: UnOp, ty: Ty<'tcx>) -> String {
    match op.kind {
        UnOpKind::Deref => format!("type `{:?}` cannot be dereferenced", ty),
        UnOpKind::Not => format!("cannot apply unary operator `!` to type `{:?}`", ty),
    }
}

fn bin_op_err_msg<'tcx>(ty1: Ty<'tcx>, op: BinOp, ty2: Ty<'tcx>) -> String {
    match op.kind {
        BinOpKind::And | BinOpKind::Or => "mismatched types".into(),
        BinOpKind::Lt | BinOpKind::Gt | BinOpKind::Eq | BinOpKind::Ge => {
            format!("cannot compare `{}` with `{}`", ty1, ty2)
        }
        BinOpKind::Add => format!("cannot add `{}` to `{}`", ty1, ty2),
        BinOpKind::Mul => format!("cannot multiply `{}` to `{}`", ty2, ty1),
        BinOpKind::Div => format!("cannot divide `{}` by `{}`", ty1, ty2),
        BinOpKind::Sub => format!("cannot subtract `{}` and `{}`", ty2, ty1),
    }
}
