use liquid_rust_common::errors::ErrorReported;
use liquid_rust_syntax::{ast, ast::LoopInv, ast::FnSig, parse_fn_sig, parse_expr, ParseErrorKind};
use rustc_ast::{tokenstream::TokenStream, AttrKind, Attribute, MacArgs};
use rustc_hash::FxHashMap;
use rustc_hir::{
    def_id::LocalDefId, itemlikevisit::ItemLikeVisitor, ForeignItem, ImplItem, ImplItemKind, Item,
    ItemKind, TraitItem, Expr, ExprKind, intravisit, intravisit::Visitor
};
use rustc_middle::ty::TyCtxt;
use rustc_session::Session;
use rustc_span::Span;


pub(crate) struct SpecCollector<'tcx, 'a> {
    tcx: TyCtxt<'tcx>,
    specs: FxHashMap<LocalDefId, FnSpec>,
    sess: &'a Session,
    error_reported: bool,
    loop_invs: Vec<LoopInv>,
}

pub struct FnSpec {
    pub fn_sig: FnSig,
    pub assume: bool,
}

impl<'tcx, 'a> SpecCollector<'tcx, 'a> {
    pub(crate) fn collect(
        tcx: TyCtxt<'tcx>,
        sess: &'a Session,
    ) -> Result<(Vec<LoopInv>, FxHashMap<LocalDefId, FnSpec>), ErrorReported> {
        let mut collector = Self {
            tcx,
            sess,
            specs: FxHashMap::default(),
            error_reported: false,
            loop_invs: Vec::new(),
        };

        tcx.hir().visit_all_item_likes(&mut collector);

        if collector.error_reported {
            Err(ErrorReported)
        } else {
            Ok((collector.loop_invs, collector.specs))
        }
    }

    fn parse_loop_annotations(&mut self, loop_span: Option<Span>, body_span: Span, attributes: &[Attribute]) {
        for attribute in attributes {
            if let AttrKind::Normal(attr_item, ..) = &attribute.kind {
                // Be sure we are in a `liquid` attribute.
                let segments = match attr_item.path.segments.as_slice() {
                    [first, segments @ ..] if first.ident.as_str() == "lr" => segments,
                    _ => continue,
                };

                match segments {
                    [second] if &*second.ident.as_str() == "loop_invariant" => {
                        if let MacArgs::Delimited(span, _, tokens) = &attr_item.args {
                            if let Some(inv) = self.parse_loop_inv(tokens.clone(), span.entire()) {
                                self.loop_invs.push(LoopInv::new(
                                    inv,
                                    loop_span,
                                    body_span,
                                ));
                            }
                        } else {
                            println!("Error: invalid liquid annotation at {:?}", attr_item);
                        }
                    }
                    _ => println!("Error: invalid liquid annotation at {:?}", attr_item),
                }
            }
        }
    }

    fn parse_loop_inv(&mut self, tokens: TokenStream, input_span: Span) -> Option<ast::Expr> {
        match parse_expr(tokens, input_span) {
            Ok(inv) => Some(inv),
            Err(err) => {
                let msg = match err.kind {
                    ParseErrorKind::UnexpectedEOF => "type annotation ended unexpectedly",
                    ParseErrorKind::UnexpectedToken => "unexpected token",
                    ParseErrorKind::IntTooLarge => "integer literal is too large",
                };

                self.emit_error(msg, err.span);
                None
            }
        }
    }

    fn parse_annotations(&mut self, def_id: LocalDefId, attributes: &[Attribute]) {
        let mut fn_sig = None;
        let mut assume = false;
        for attribute in attributes {
            if let AttrKind::Normal(attr_item, ..) = &attribute.kind {
                // Be sure we are in a `liquid` attribute.
                let segments = match attr_item.path.segments.as_slice() {
                    [first, segments @ ..] if first.ident.as_str() == "lr" => segments,
                    _ => continue,
                };

                match segments {
                    [second] if &*second.ident.as_str() == "ty" => {
                        if fn_sig.is_some() {
                            self.emit_error("duplicated function signature.", attr_item.span());
                            return;
                        }

                        if let MacArgs::Delimited(span, _, tokens) = &attr_item.args {
                            fn_sig = self.parse_fn_annot(tokens.clone(), span.entire());
                        } else {
                            self.emit_error("invalid liquid annotation.", attr_item.span())
                        }
                    }
                    [second] if &*second.ident.as_str() == "assume" => {
                        assume = true;
                    }
                    _ => self.emit_error("invalid liquid annotation.", attr_item.span()),
                }
            }
        }
        if let Some(fn_sig) = fn_sig {
            self.specs.insert(def_id, FnSpec { fn_sig, assume });
        }
    }

    fn parse_fn_annot(&mut self, tokens: TokenStream, input_span: Span) -> Option<FnSig> {
        match parse_fn_sig(tokens, input_span) {
            Ok(fn_sig) => Some(fn_sig),
            Err(err) => {
                let msg = match err.kind {
                    ParseErrorKind::UnexpectedEOF => "type annotation ended unexpectedly",
                    ParseErrorKind::UnexpectedToken => "unexpected token",
                    ParseErrorKind::IntTooLarge => "integer literal is too large",
                };

                self.emit_error(msg, err.span);
                None
            }
        }
    }

    fn emit_error(&mut self, message: &str, span: Span) {
        self.error_reported = true;
        self.sess.span_err(span, message);
    }
}

impl<'hir> ItemLikeVisitor<'hir> for SpecCollector<'hir, '_> {
    fn visit_item(&mut self, item: &'hir Item<'hir>) {
        if let ItemKind::Fn(..) = item.kind {
            let hir_id = item.hir_id();
            let attrs = self.tcx.hir().attrs(hir_id);
            self.parse_annotations(item.def_id, attrs);
            intravisit::walk_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, _trait_item: &'hir TraitItem<'hir>) {}
    fn visit_impl_item(&mut self, item: &'hir ImplItem<'hir>) {
        if let ImplItemKind::Fn(..) = &item.kind {
            let hir_id = item.hir_id();
            let attrs = self.tcx.hir().attrs(hir_id);
            self.parse_annotations(item.def_id, attrs);
            //intravisit::walk_item(self, item);
        }
    }

    fn visit_foreign_item(&mut self, _foreign_item: &'hir ForeignItem<'hir>) {}
}

impl<'tcx> Visitor<'tcx> for SpecCollector<'tcx, '_> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'_>) {
        if let ExprKind::Loop(block, _, source, span) = expr.kind {
            let (cond_span, body_span) = match source {
                rustc_hir::LoopSource::Loop => (None, block.span),
                _ => (Some(span), block.span),
            };
            self.parse_loop_annotations(cond_span, body_span, self.tcx.hir().attrs(expr.hir_id));
        }

        intravisit::walk_expr(self, expr);
    }
}
