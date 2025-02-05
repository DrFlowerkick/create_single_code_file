// functions to interact with parsed src files

mod error;
mod fold_and_visit;
mod item_name;
mod syn_extend;
pub use error::{ParsingError, ParsingResult};
pub use fold_and_visit::{FoldRemoveAttrDocComments, IdentCollector, VisitVerbatim};
pub use item_name::ItemName;
pub use syn_extend::{ItemExt, SourcePath, ToTokensExt, UseTreeExt};

use syn::{fold::Fold, visit::Visit, File, Item};

// load syntax from given file
pub fn load_syntax(code: &str) -> ParsingResult<File> {
    // Parse the source code into a syntax tree
    let syntax: File = syn::parse_file(code)?;
    // remove doc comments
    let mut remove_doc_comments = FoldRemoveAttrDocComments;
    let mut syntax = remove_doc_comments.fold_file(syntax);
    // check for verbatim parsed elements
    let mut check_verbatim = VisitVerbatim {
        verbatim_tokens: String::new(),
    };
    check_verbatim.visit_file(&syntax);
    if !check_verbatim.verbatim_tokens.is_empty() {
        return Err(ParsingError::ContainsVerbatim(
            check_verbatim.verbatim_tokens,
        ));
    }
    // remove mod tests and macros without a name
    syntax.items.retain(|item| match item {
        Item::Macro(item_macro) => item_macro.ident.is_some(),
        Item::Mod(item_mod) => item_mod.ident != "tests",
        _ => true,
    });
    Ok(syntax)
}
