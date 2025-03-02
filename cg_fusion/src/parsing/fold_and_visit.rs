// contains functions to visit and fold over parsed src code.

use crate::add_context;
use quote::ToTokens;
use std::fmt::Write;
use syn::{Attribute, Ident, Meta, fold::Fold, visit::Visit};

// helper to remove doc comments from src file
pub struct FoldRemoveAttrDocComments;

impl Fold for FoldRemoveAttrDocComments {
    fn fold_attributes(&mut self, mut i: Vec<Attribute>) -> Vec<Attribute> {
        i.retain(|attr| match &attr.meta {
            Meta::NameValue(mnv) => match mnv.path.segments.last() {
                // filter all doc comments
                Some(path) => path.ident != "doc",
                None => true,
            },
            _ => true,
        });
        i
    }
}

// helper to search for verbatim parsed code
pub struct VisitVerbatim {
    pub verbatim_tokens: String,
}

impl<'ast> Visit<'ast> for VisitVerbatim {
    fn visit_expr(&mut self, i: &'ast syn::Expr) {
        if let syn::Expr::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_item(&mut self, i: &'ast syn::Item) {
        if let syn::Item::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_foreign_item(&mut self, i: &'ast syn::ForeignItem) {
        if let syn::ForeignItem::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_trait_item(&mut self, i: &'ast syn::TraitItem) {
        if let syn::TraitItem::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_impl_item(&mut self, i: &'ast syn::ImplItem) {
        if let syn::ImplItem::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_lit(&mut self, i: &'ast syn::Lit) {
        if let syn::Lit::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_pat(&mut self, i: &'ast syn::Pat) {
        if let syn::Pat::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_type(&mut self, i: &'ast syn::Type) {
        if let syn::Type::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_type_param_bound(&mut self, i: &'ast syn::TypeParamBound) {
        if let syn::TypeParamBound::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
}

// visitor to collect all ident with the given name
pub struct IdentCollector {
    pub ident_name: String,
    pub ident_collector: Vec<Ident>,
}

impl IdentCollector {
    pub fn new(ident_name: String) -> Self {
        Self {
            ident_name,
            ident_collector: Vec::new(),
        }
    }
    pub fn extract_collector(&mut self) -> Option<Vec<Ident>> {
        if self.ident_collector.is_empty() {
            return None;
        }
        Some(std::mem::take(&mut self.ident_collector))
    }
}

impl<'ast> Visit<'ast> for IdentCollector {
    fn visit_ident(&mut self, ident: &'ast Ident) {
        if *ident == self.ident_name {
            self.ident_collector.push(ident.to_owned());
        }
        syn::visit::visit_ident(self, ident);
    }
}

// visitor to identify write! and writeln! macro usage
pub struct MacroWriteFinder {
    pub found_write: bool,
}

impl MacroWriteFinder {
    pub fn new() -> Self {
        Self { found_write: false }
    }
}

impl<'ast> Visit<'ast> for MacroWriteFinder {
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac.path.is_ident("write") {
            self.found_write = true;
        }
        syn::visit::visit_macro(self, mac);
    }
}
