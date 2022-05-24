#![feature(rustc_private)]
#![feature(min_specialization)]
#![feature(box_patterns, once_cell)]

extern crate rustc_data_structures;
extern crate rustc_errors;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_serialize;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;

mod desugar;
mod table_resolver;
mod zip_resolver;

use liquid_rust_middle::core::{self, AdtSortsMap};
use liquid_rust_syntax::surface;
use rustc_errors::ErrorReported;
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

pub use desugar::{desugar_enum_def, desugar_qualifier, resolve_sorts};

pub fn desugar_struct_def(
    tcx: TyCtxt,
    struct_def: surface::StructDef,
) -> Result<core::AdtDef, ErrorReported> {
    let mut resolver = table_resolver::Resolver::from_adt(tcx, struct_def.def_id)?;
    let struct_def = resolver.resolve_struct_def(struct_def)?;
    desugar::desugar_struct_def(tcx.sess, struct_def)
}

pub fn desugar_fn_sig(
    tcx: TyCtxt,
    sorts: &impl AdtSortsMap,
    def_id: DefId,
    fn_sig: surface::FnSig,
) -> Result<core::FnSig, ErrorReported> {
    let default_sig = surface::default_fn_sig(tcx, def_id);
    let fn_sig = zip_resolver::zip_bare_def(fn_sig, default_sig);
    desugar::desugar_fn_sig(tcx.sess, sorts, fn_sig)
}