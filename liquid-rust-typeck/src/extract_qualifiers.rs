use crate::ty::{self, Param};
use liquid_rust_common::index::IndexGen;
use rustc_hash::{FxHashMap, FxHashSet};

struct Transformer<'a> {
    free_map: FxHashMap<ty::Name, ty::Name>,
    gen: IndexGen<ty::Name>,
    bound: Vec<ty::Name>,
    fn_sig: &'a ty::FnSig,
    params: Vec<Param>,
}

pub fn qualifiers_from_fn_sig<'a>(
    fn_sig: &'a ty::FnSig,
    params: &'a [Param],
) -> Vec<ty::Qualifier> {
    let mut transformer = Transformer::new(fn_sig, params);
    let mut vec = Vec::new();
    vec.append(&mut transformer.process_args());
    vec.append(&mut transformer.process_requires());
    vec.append(&mut transformer.process_ensures());
    vec.append(&mut transformer.process_ret());
    vec
}

impl<'a> Transformer<'a> {
    pub fn new(fn_sig: &'a ty::FnSig, params: &'a [Param]) -> Self {
        let gen = IndexGen::new();
        let mut free_map = FxHashMap::default();

        let params = params
            .iter()
            .map(|param| {
                let fresh = gen.fresh();
                free_map.insert(param.name, fresh);
                Param { name: fresh, sort: param.sort.clone() }
            })
            .collect();

        Self { free_map, gen, bound: Vec::new(), fn_sig, params }
    }

    fn process_args(&mut self) -> Vec<ty::Qualifier> {
        self.fn_sig
            .args()
            .iter()
            .filter_map(|arg| self.ty_to_qualifier(arg))
            .collect()
    }

    fn process_ret(&mut self) -> Vec<ty::Qualifier> {
        match self.ty_to_qualifier(self.fn_sig.ret()) {
            None => Vec::new(),
            Some(qualifier) => vec![qualifier],
        }
    }

    fn process_requires(&mut self) -> Vec<ty::Qualifier> {
        self.fn_sig
            .requires()
            .iter()
            .filter_map(|constr| self.constr_to_qualifer(constr))
            .collect()
    }

    fn process_ensures(&mut self) -> Vec<ty::Qualifier> {
        self.fn_sig
            .ensures()
            .iter()
            .filter_map(|constr| self.constr_to_qualifer(constr))
            .collect()
    }

    fn relevant_params(&self, seen: &FxHashSet<ty::Name>) -> Vec<(ty::Name, ty::Sort)> {
        self.params
            .iter()
            .filter_map(|param| {
                match seen.get(&param.name) {
                    Some(name) => Some((name.clone(), param.sort.clone())),
                    None => None,
                }
            })
            .collect()
    }

    fn constr_to_qualifer(&mut self, constr: &ty::Constr) -> Option<ty::Qualifier> {
        match constr {
            ty::Constr::Type(_path, ty) => self.ty_to_qualifier(ty),
            ty::Constr::Pred(expr) => {
                let mut seen = FxHashSet::default();
                let expr = self.expr_to_qualifier(&mut seen, expr);
                let args = self.relevant_params(&seen);
                Some(ty::Qualifier { name: "Test".to_string(), args, expr })
            }
        }
    }

    fn ty_to_qualifier(&mut self, ty: &ty::Ty) -> Option<ty::Qualifier> {
        match ty.kind() {
            ty::TyKind::Refine(_bty, _exprs) => None,
            ty::TyKind::Exists(bty, pred) => {
                let fresh = self.gen.fresh();
                self.bound.push(fresh);

                let qualifier = match pred {
                    ty::Pred::Infer(_) => None,
                    ty::Pred::Expr(expr) => {
                        let mut seen = FxHashSet::default();
                        let expr = self.expr_to_qualifier(&mut seen, expr);
                        let mut args = self.relevant_params(&seen);
                        args.push((fresh, basety_to_sort(bty)));
                        Some(ty::Qualifier { name: "Test".to_string(), args, expr })
                    }
                };

                self.bound.pop();
                qualifier
            }
            _ => None,
        }
    }

    fn expr_to_qualifier(&mut self, seen: &mut FxHashSet<ty::Name>, e: &ty::Expr) -> ty::Expr {
        match e.kind() {
            ty::ExprKind::Var(v) => {
                match v {
                    ty::Var::Bound(index) => {
                        ty::Expr::var(ty::Var::Free(
                            self.bound[self.bound.len() - (*index as usize) - 1],
                        ))
                    }
                    ty::Var::Free(name) => {
                        let name = self.free_map.get(name).unwrap();
                        seen.insert(*name);
                        ty::Expr::var(ty::Var::Free(name.clone()))
                    }
                }
            }
            ty::ExprKind::Constant(c) => ty::Expr::constant(c.clone()),
            ty::ExprKind::BinaryOp(bop, e1, e2) => {
                let e1 = self.expr_to_qualifier(seen, e1);
                let e2 = self.expr_to_qualifier(seen, e2);
                ty::Expr::binary_op(bop.clone(), e1, e2)
            }
            _ => unimplemented!(),
        }
    }
}

fn basety_to_sort(bty: &ty::BaseTy) -> ty::Sort {
    match bty {
        ty::BaseTy::Int(_) => ty::Sort::int(),
        // TODO: > 0
        ty::BaseTy::Uint(_) => ty::Sort::int(),
        ty::BaseTy::Bool => ty::Sort::bool(),
        ty::BaseTy::Adt(_, _) => unimplemented!(),
    }
}