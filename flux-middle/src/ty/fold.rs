//! This modules folows the implementation of folding in rustc. For more information read the
//! documentation in [`rustc_middle::ty::fold`].

use itertools::Itertools;
use rustc_hash::FxHashSet;

use crate::intern::{Internable, List};

use super::{
    BaseTy, Binders, Constraint, Expr, ExprKind, FnSig, Index, KVar, Name, Pred, Sort, Ty, TyKind,
};

pub trait TypeVisitor: Sized {
    fn visit_fvar(&mut self, name: Name) {
        name.super_visit_with(self);
    }
}

pub trait TypeFolder: Sized {
    fn fold_binders<T: TypeFoldable>(&mut self, t: &Binders<T>) -> Binders<T> {
        t.super_fold_with(self)
    }

    fn fold_ty(&mut self, ty: &Ty) -> Ty {
        ty.super_fold_with(self)
    }

    fn fold_expr(&mut self, expr: &Expr) -> Expr {
        expr.super_fold_with(self)
    }
}

pub trait TypeFoldable: Sized {
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self;
    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V);

    fn fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        self.super_fold_with(folder)
    }

    fn visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        self.super_visit_with(visitor)
    }

    /// Returns the set of all free variables.
    /// For example, `Vec<i32[n]>{v : v > m}` returns `{n, m}`.
    fn fvars(&self) -> FxHashSet<Name> {
        struct CollectFreeVars(FxHashSet<Name>);

        impl TypeVisitor for CollectFreeVars {
            fn visit_fvar(&mut self, name: Name) {
                self.0.insert(name);
            }
        }

        let mut collector = CollectFreeVars(FxHashSet::default());
        self.visit_with(&mut collector);
        collector.0
    }

    /// Replaces all [`holes`] with a fresh [`predicate`] generated by calling `mk_pred`.
    ///
    /// [`holes`]: Pred::Hole
    /// [`predicate`]: Pred
    fn replace_holes(&self, mk_pred: &mut impl FnMut(&[Sort]) -> Binders<Pred>) -> Self {
        struct ReplaceHoles<'a, F>(&'a mut F);

        impl<'a, F> TypeFolder for ReplaceHoles<'a, F>
        where
            F: FnMut(&[Sort]) -> Binders<Pred>,
        {
            fn fold_ty(&mut self, ty: &Ty) -> Ty {
                if let TyKind::Exists(bty, Binders { params, value: Pred::Hole }) = ty.kind() {
                    Ty::exists(bty.super_fold_with(self), self.0(params))
                } else {
                    ty.super_fold_with(self)
                }
            }
        }
        self.fold_with(&mut ReplaceHoles(mk_pred))
    }

    /// Turns each [`TyKind::Indexed`] into [`TyKind::Exists`] with a [`hole`] and replaces
    /// all existing [`predicates`] with a [`hole`].
    /// For example, `Vec<i32{v : v > 0}>[n]` becomes `Vec<i32{*}>{*}`.
    ///
    /// [`hole`]: Pred::Hole
    /// [`predicates`]: Pred
    fn with_holes(&self) -> Self {
        struct WithHoles;

        impl TypeFolder for WithHoles {
            fn fold_ty(&mut self, ty: &Ty) -> Ty {
                if let TyKind::Indexed(bty, _) | TyKind::Exists(bty, _) = ty.kind() {
                    let sorts = bty.sorts();
                    Ty::exists(bty.super_fold_with(self), Binders::new(Pred::Hole, sorts))
                } else {
                    ty.super_fold_with(self)
                }
            }
        }

        self.fold_with(&mut WithHoles)
    }

    fn replace_generic_types(&self, tys: &[Ty]) -> Self {
        struct GenericsFolder<'a>(&'a [Ty]);

        impl TypeFolder for GenericsFolder<'_> {
            fn fold_ty(&mut self, ty: &Ty) -> Ty {
                if let TyKind::Param(param_ty) = ty.kind() {
                    self.0[param_ty.index as usize].clone()
                } else {
                    ty.super_fold_with(self)
                }
            }
        }

        self.fold_with(&mut GenericsFolder(tys))
    }
}

impl<T> TypeFoldable for Binders<T>
where
    T: TypeFoldable,
{
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        Binders::new(self.value.fold_with(folder), self.params.clone())
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        self.value.visit_with(visitor)
    }

    fn fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        folder.fold_binders(self)
    }
}

impl TypeFoldable for FnSig {
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        let requires = self
            .requires
            .iter()
            .map(|constr| constr.fold_with(folder))
            .collect_vec();
        let args = self
            .args
            .iter()
            .map(|arg| arg.fold_with(folder))
            .collect_vec();
        let ensures = self
            .ensures
            .iter()
            .map(|constr| constr.fold_with(folder))
            .collect_vec();
        let ret = self.ret.fold_with(folder);
        FnSig::new(requires, args, ret, ensures)
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        self.requires
            .iter()
            .for_each(|constr| constr.visit_with(visitor));
        self.args.iter().for_each(|arg| arg.visit_with(visitor));
        self.ensures
            .iter()
            .for_each(|constr| constr.visit_with(visitor));
        self.ret.visit_with(visitor);
    }
}

impl TypeFoldable for Constraint {
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        match self {
            Constraint::Type(path, ty) => {
                Constraint::Type(
                    path.to_expr()
                        .fold_with(folder)
                        .to_path()
                        .expect("folding produced an invalid path"),
                    ty.fold_with(folder),
                )
            }
            Constraint::Pred(e) => Constraint::Pred(e.fold_with(folder)),
        }
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        match self {
            Constraint::Type(path, ty) => {
                path.to_expr().visit_with(visitor);
                ty.visit_with(visitor);
            }
            Constraint::Pred(e) => e.visit_with(visitor),
        }
    }
}

impl TypeFoldable for Ty {
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Ty {
        match self.kind() {
            TyKind::Indexed(bty, indices) => {
                Ty::indexed(
                    bty.fold_with(folder),
                    indices
                        .iter()
                        .map(|idx| idx.fold_with(folder))
                        .collect_vec(),
                )
            }
            TyKind::Exists(bty, pred) => {
                TyKind::Exists(bty.fold_with(folder), pred.fold_with(folder)).intern()
            }
            TyKind::Tuple(tys) => {
                Ty::tuple(tys.iter().map(|ty| ty.fold_with(folder)).collect_vec())
            }
            TyKind::Ptr(path) => {
                Ty::ptr(
                    path.to_expr()
                        .fold_with(folder)
                        .to_path()
                        .expect("folding produced an invalid path"),
                )
            }
            TyKind::BoxPtr(loc, alloc) => {
                Ty::box_ptr(
                    Expr::fvar(*loc)
                        .fold_with(folder)
                        .to_name()
                        .expect("folding produced an invalid name"),
                    alloc.fold_with(folder),
                )
            }
            TyKind::Ref(rk, ty) => Ty::mk_ref(*rk, ty.fold_with(folder)),
            TyKind::Constr(pred, ty) => Ty::constr(pred.fold_with(folder), ty.fold_with(folder)),
            TyKind::Float(_)
            | TyKind::Uninit
            | TyKind::Param(_)
            | TyKind::Never
            | TyKind::Discr(..) => self.clone(),
        }
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        match self.kind() {
            TyKind::Indexed(bty, indices) => {
                bty.visit_with(visitor);
                indices.iter().for_each(|idx| idx.visit_with(visitor));
            }
            TyKind::Exists(bty, pred) => {
                bty.visit_with(visitor);
                pred.visit_with(visitor);
            }
            TyKind::Tuple(tys) => tys.iter().for_each(|ty| ty.visit_with(visitor)),
            TyKind::Ref(_, ty) => ty.visit_with(visitor),
            TyKind::Ptr(path) => path.to_expr().visit_with(visitor),
            TyKind::BoxPtr(loc, ty) => {
                Expr::fvar(*loc).visit_with(visitor);
                ty.visit_with(visitor);
            }
            TyKind::Constr(pred, ty) => {
                pred.visit_with(visitor);
                ty.visit_with(visitor)
            }
            TyKind::Param(_)
            | TyKind::Never
            | TyKind::Discr(..)
            | TyKind::Float(_)
            | TyKind::Uninit => {}
        }
    }

    fn fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        folder.fold_ty(self)
    }
}

impl TypeFoldable for Index {
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        Index { expr: self.expr.fold_with(folder), is_binder: self.is_binder }
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        self.expr.visit_with(visitor);
    }
}

impl TypeFoldable for BaseTy {
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        match self {
            BaseTy::Adt(adt_def, substs) => {
                BaseTy::adt(adt_def.clone(), substs.iter().map(|ty| ty.fold_with(folder)))
            }
            BaseTy::Int(_) | BaseTy::Uint(_) | BaseTy::Bool => self.clone(),
        }
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        match self {
            BaseTy::Adt(_, substs) => substs.iter().for_each(|ty| ty.visit_with(visitor)),
            BaseTy::Int(_) | BaseTy::Uint(_) | BaseTy::Bool => {}
        }
    }
}

impl TypeFoldable for Pred {
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        match self {
            Pred::Kvar(kvar) => Pred::Kvar(kvar.fold_with(folder)),
            Pred::Expr(e) => Pred::Expr(e.fold_with(folder)),
            Pred::Hole => Pred::Hole,
        }
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        match self {
            Pred::Expr(e) => e.visit_with(visitor),
            Pred::Kvar(kvar) => kvar.visit_with(visitor),
            Pred::Hole => {}
        }
    }
}

impl TypeFoldable for KVar {
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        let KVar { kvid, args, scope } = self;
        let args = args.iter().map(|e| e.fold_with(folder)).collect();
        let scope = scope.iter().map(|e| e.fold_with(folder)).collect();
        KVar::new(*kvid, args, scope)
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        self.args.iter().for_each(|e| e.visit_with(visitor));
    }
}

impl TypeFoldable for Expr {
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        match self.kind() {
            ExprKind::FreeVar(name) => Expr::fvar(name.fold_with(folder)),
            ExprKind::BoundVar(bvar) => Expr::bvar(*bvar),
            ExprKind::EVar(evar) => Expr::evar(evar.clone()),
            ExprKind::ConstDefId(did) => Expr::const_def_id(*did),
            ExprKind::Local(local) => Expr::local(*local),
            ExprKind::Constant(c) => Expr::constant(*c),
            ExprKind::BinaryOp(op, e1, e2) => {
                Expr::binary_op(*op, e1.fold_with(folder), e2.fold_with(folder))
            }
            ExprKind::UnaryOp(op, e) => Expr::unary_op(*op, e.fold_with(folder)),
            ExprKind::TupleProj(e, proj) => Expr::proj(e.fold_with(folder), *proj),
            ExprKind::Tuple(exprs) => {
                Expr::tuple(exprs.iter().map(|e| e.fold_with(folder)).collect_vec())
            }
            ExprKind::PathProj(e, field) => Expr::path_proj(e.fold_with(folder), *field),
        }
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        match self.kind() {
            ExprKind::FreeVar(name) => name.visit_with(visitor),
            ExprKind::BinaryOp(_, e1, e2) => {
                e1.visit_with(visitor);
                e2.visit_with(visitor);
            }
            ExprKind::UnaryOp(_, e) | ExprKind::TupleProj(e, _) => e.visit_with(visitor),
            ExprKind::Tuple(exprs) => {
                for e in exprs {
                    e.visit_with(visitor);
                }
            }
            ExprKind::PathProj(e, _) => e.visit_with(visitor),
            ExprKind::Constant(_)
            | ExprKind::BoundVar(_)
            | ExprKind::Local(_)
            | ExprKind::ConstDefId(_)
            | ExprKind::EVar(_) => {}
        }
    }

    fn fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        folder.fold_expr(self)
    }
}

impl TypeFoldable for Name {
    fn super_fold_with<F: TypeFolder>(&self, _folder: &mut F) -> Self {
        *self
    }

    fn super_visit_with<V: TypeVisitor>(&self, _visitor: &mut V) {}

    fn visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        visitor.visit_fvar(*self)
    }
}

impl<T> TypeFoldable for List<T>
where
    T: TypeFoldable,
    [T]: Internable,
{
    fn super_fold_with<F: TypeFolder>(&self, folder: &mut F) -> Self {
        List::from_vec(self.iter().map(|t| t.fold_with(folder)).collect())
    }

    fn super_visit_with<V: TypeVisitor>(&self, visitor: &mut V) {
        self.iter().for_each(|t| t.visit_with(visitor));
    }
}
