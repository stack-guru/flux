//! "Core" (desugared) version of level liquid annotations. The main difference with
//! the surface syntax is that the list of refinement parameters is explicit in `core`.
//! For example, the signature `fn(x: &strg i32[@n]) -> (); x: i32[@n + 1]` desugars to
//! `for<n: int, l: loc> fn(l: i32[n]; ptr(l)) -> (); l: i32[n + 1]`.

use core::fmt;
use std::fmt::Write;

use flux_common::format::PadAdapter;
pub use flux_fixpoint::BinOp;
use itertools::Itertools;
use rustc_hir::def_id::DefId;
use rustc_index::newtype_index;
pub use rustc_middle::ty::{FloatTy, IntTy, ParamTy, UintTy};
use rustc_span::{Span, Symbol};
pub use rustc_target::abi::VariantIdx;

pub trait AdtSortsMap {
    fn get(&self, def_id: DefId) -> Option<&[Sort]>;
}

#[derive(Debug)]
pub struct StructDef {
    pub def_id: DefId,
    pub kind: StructKind,
    pub refined_by: Vec<Param>,
    pub generics: rustc_middle::ty::Generics,
}

#[derive(Debug)]
pub struct EnumDef {
    pub def_id: DefId,
    pub refined_by: Vec<Param>,
}

#[derive(Debug)]
pub enum StructKind {
    Transparent { fields: Vec<Ty> },
    Opaque,
}

pub struct FnSig {
    /// example: vec![(n: Int), (l: Loc)]
    pub params: Vec<Param>,
    /// example: vec![(0 <= n), (l: i32)]
    pub requires: Vec<Constraint>,
    /// example: vec![(x: StrRef(l))]
    pub args: Vec<Ty>,
    /// example: i32
    pub ret: Ty,
    /// example: vec![(l: i32{v:n < v})]
    pub ensures: Vec<Constraint>,
}
pub enum Constraint {
    /// A type constraint on a location
    Type(Ident, Ty),
    /// A predicate that needs to hold
    Pred(Expr),
}

#[derive(Debug)]
pub struct Qualifier {
    pub name: String,
    pub args: Vec<Param>,
    pub expr: Expr,
}

pub struct ConstSig {
    pub def_id: DefId,
    pub val: i128,
    pub ty: Ty,
}

pub enum Ty {
    /// As a base type `bty` without any refinements is equivalent to `bty{vs : true}` we don't
    /// technically need this variant, but we keep it around to simplify desugaring.
    BaseTy(BaseTy),
    Indexed(BaseTy, Indices),
    /// Existential types in core are represented with an explicit list of binders for
    /// every index of the [`BaseTy`], e.g., `i32{v : v > 0}` for one index and `RMat{v0,v1 : v0 == v1}`.
    /// for two indices. (there's currently no equivalent surface syntax).
    Exists(BaseTy, Vec<Ident>, Expr),
    /// Constrained types `{T : p}` are like existentials but without binders, and are useful
    /// for specifying constraints on indexed values e.g. `{i32[@a] | 0 <= a}`
    Constr(Expr, Box<Ty>),
    Float(FloatTy),
    Ptr(Ident),
    Ref(RefKind, Box<Ty>),
    Param(ParamTy),
    Tuple(Vec<Ty>),
    Never,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
pub enum RefKind {
    Shr,
    Mut,
}

pub struct Indices {
    pub indices: Vec<Index>,
    pub span: Span,
}

pub struct Index {
    pub expr: Expr,
    /// Whether this index was used as a binder in the surface syntax. Used as a hint for inferring
    /// parameters at function calls.
    pub is_binder: bool,
}

pub enum BaseTy {
    Int(IntTy),
    Uint(UintTy),
    Bool,
    Adt(DefId, Vec<Ty>),
}

#[derive(Debug, Clone, Copy)]
pub struct Param {
    pub name: Ident,
    pub sort: Sort,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Sort {
    Bool,
    Int,
    Loc,
}

pub struct Expr {
    pub kind: ExprKind,
    pub span: Option<Span>,
}

pub enum ExprKind {
    Const(DefId, Span),
    Var(Name, Symbol, Span),
    Literal(Lit),
    BinaryOp(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Clone, Copy)]
pub enum Lit {
    Int(i128),
    Bool(bool),
}

#[derive(Clone, Copy)]
pub struct Ident {
    pub name: Name,
    pub source_info: (Span, Symbol),
}

newtype_index! {
    pub struct Name {
        DEBUG_FORMAT = "x{}",
    }
}

impl BaseTy {
    /// Returns `true` if the base ty is [`Bool`].
    ///
    /// [`Bool`]: BaseTy::Bool
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool)
    }
}
impl Lit {
    pub fn from_i128(v: i128) -> Lit {
        Lit::Int(v)
    }
}

impl Expr {
    pub const TRUE: Expr = Expr { kind: ExprKind::Literal(Lit::TRUE), span: None };

    pub fn from_i128(v: i128) -> Expr {
        Expr { kind: ExprKind::Literal(Lit::from_i128(v)), span: None }
    }
}

impl Lit {
    pub const TRUE: Lit = Lit::Bool(true);
}

impl StructDef {
    pub fn sorts(&self) -> Vec<Sort> {
        self.refined_by.iter().map(|param| param.sort).collect()
    }
}

impl<S> AdtSortsMap for std::collections::HashMap<DefId, Vec<Sort>, S>
where
    S: std::hash::BuildHasher,
{
    fn get(&self, def_id: DefId) -> Option<&[Sort]> {
        self.get(&def_id).map(|sorts| sorts.as_slice())
    }
}

impl fmt::Debug for FnSig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            let mut p = PadAdapter::wrap_fmt(f, 4);
            write!(p, "(\nfn")?;
            if !self.params.is_empty() {
                write!(
                    p,
                    "<{}>",
                    self.params.iter().format_with(", ", |param, f| {
                        f(&format_args!("{:?}: {:?}", param.name, param.sort))
                    })
                )?;
            }
            write!(p, "({:?}) -> {:?}", self.args.iter().format(", "), self.ret)?;
            if !self.requires.is_empty() {
                write!(p, "\nrequires {:?} ", self.requires.iter().format(", "))?;
            }
            if !self.ensures.is_empty() {
                write!(p, "\nensures {:?}", self.ensures.iter().format(", "))?;
            }
            write!(f, "\n)")?;
        } else {
            if !self.params.is_empty() {
                write!(
                    f,
                    "for<{}> ",
                    self.params.iter().format_with(", ", |param, f| {
                        f(&format_args!("{:?}: {:?}", param.name, param.sort))
                    })
                )?;
            }
            if !self.requires.is_empty() {
                write!(f, "[{:?}] ", self.requires.iter().format(", "))?;
            }
            write!(f, "fn({:?}) -> {:?}", self.args.iter().format(", "), self.ret)?;
            if !self.ensures.is_empty() {
                write!(f, "; [{:?}]", self.ensures.iter().format(", "))?;
            }
        }

        Ok(())
    }
}

impl fmt::Debug for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constraint::Type(loc, ty) => write!(f, "{loc:?}: {ty:?}"),
            Constraint::Pred(e) => write!(f, "{e:?}"),
        }
    }
}

impl fmt::Debug for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::BaseTy(bty) => write!(f, "{bty:?}"),
            Ty::Indexed(bty, e) => fmt_bty(bty, Some(e), f),
            Ty::Exists(bty, binders, p) => {
                write!(f, "{bty:?}{{{binders:?} : {p:?}}}")
            }
            Ty::Float(float_ty) => write!(f, "{}", float_ty.name_str()),
            Ty::Ptr(loc) => write!(f, "ref<{loc:?}>"),
            Ty::Ref(RefKind::Mut, ty) => write!(f, "&mut {ty:?}"),
            Ty::Ref(RefKind::Shr, ty) => write!(f, "&{ty:?}"),
            Ty::Param(param) => write!(f, "{param}"),
            Ty::Tuple(tys) => write!(f, "({:?})", tys.iter().format(", ")),
            Ty::Never => write!(f, "!"),
            Ty::Constr(pred, ty) => write!(f, "{{{ty:?} : {pred:?}}}"),
        }
    }
}

impl fmt::Debug for BaseTy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_bty(self, None, f)
    }
}

fn fmt_bty(bty: &BaseTy, e: Option<&Indices>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match bty {
        BaseTy::Int(int_ty) => write!(f, "{}", int_ty.name_str())?,
        BaseTy::Uint(uint_ty) => write!(f, "{}", uint_ty.name_str())?,
        BaseTy::Bool => write!(f, "bool")?,
        BaseTy::Adt(did, _) => write!(f, "{did:?}")?,
    }
    match bty {
        BaseTy::Int(_) | BaseTy::Uint(_) | BaseTy::Bool => {
            if let Some(e) = e {
                write!(f, "<{e:?}>")?;
            }
        }
        BaseTy::Adt(_, args) => {
            if !args.is_empty() || e.is_some() {
                write!(f, "<")?;
            }
            write!(f, "{:?}", args.iter().format(", "))?;
            if let Some(e) = e {
                if !args.is_empty() {
                    write!(f, ", ")?;
                }
                write!(f, "{:?}", e)?;
            }
            if !args.is_empty() || e.is_some() {
                write!(f, ">")?;
            }
        }
    }
    Ok(())
}

impl fmt::Debug for Indices {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}]", self.indices.iter().format(", "))
    }
}

impl fmt::Debug for Index {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_binder {
            write!(f, "@")?;
        }
        write!(f, "{:?}", self.expr)
    }
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ExprKind::Var(x, ..) => write!(f, "{x:?}"),
            ExprKind::BinaryOp(op, e1, e2) => write!(f, "({e1:?} {op:?} {e2:?})"),
            ExprKind::Literal(lit) => write!(f, "{lit:?}"),
            ExprKind::Const(x, _) => write!(f, "{x:?}"),
        }
    }
}

impl fmt::Debug for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.name)
    }
}

impl fmt::Debug for Lit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lit::Int(i) => write!(f, "{i}"),
            Lit::Bool(b) => write!(f, "{b}"),
        }
    }
}

impl fmt::Debug for Sort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sort::Bool => write!(f, "bool"),
            Sort::Int => write!(f, "int"),
            Sort::Loc => write!(f, "loc"),
        }
    }
}