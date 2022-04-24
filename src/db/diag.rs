mod build_log;
mod syntax;

use lsp_types::Diagnostic;

use crate::DocumentLanguage;

use super::{Document, DocumentDatabase, SyntaxDatabase, WorkspaceDatabase};

#[salsa::query_group(DiagnosticsDatabaseStorage)]
pub trait DiagnosticsDatabase:
    salsa::Database + DocumentDatabase + SyntaxDatabase + WorkspaceDatabase
{
    fn diagnostics(&self, document: Document) -> im::HashMap<Document, im::Vector<Diagnostic>>;
}

fn diagnostics(
    db: &dyn DiagnosticsDatabase,
    document: Document,
) -> im::HashMap<Document, im::Vector<Diagnostic>> {
    match db.language(document) {
        DocumentLanguage::Latex => {
            im::hashmap! {
                document => syntax::analyze_latex_static(db, document),
            }
        }
        DocumentLanguage::Bibtex => {
            im::hashmap! {
                document => syntax::analyze_bibtex_static(db, document),
            }
        }
        DocumentLanguage::BuildLog => build_log::analyze_build_log_static(db, document),
    }
}
