use std::path::PathBuf;

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::{
    db::{AuxiliaryFileKind, Document, DocumentData, SyntaxTree},
    syntax::build_log::BuildErrorLevel,
};

use super::DiagnosticsDatabase;

pub fn analyze_build_log_static(
    db: &dyn DiagnosticsDatabase,
    document: Document,
) -> im::HashMap<Document, im::Vector<Diagnostic>> {
    let mut diag_map: im::HashMap<Document, im::Vector<Diagnostic>> = im::HashMap::new();

    if let SyntaxTree::BuildLog(errors) = db.syntax_tree(document) {
        if let Some(root_document) = db.compilation_unit(document).into_iter().find(|&root| {
            db.lookup_intern_document(root)
                .uri
                .as_str()
                .ends_with(".aux")
                && db
                    .linked_auxiliary_files(root, AuxiliaryFileKind::Log)
                    .contains(&document)
        }) {
            let base_path = PathBuf::from(db.lookup_intern_document(root_document).uri.path());

            for error in errors.iter() {
                let pos = Position::new(error.line.unwrap_or(0), 0);
                let severity = match error.level {
                    BuildErrorLevel::Error => DiagnosticSeverity::ERROR,
                    BuildErrorLevel::Warning => DiagnosticSeverity::WARNING,
                };
                let range = Range::new(pos, pos);
                let diag = Diagnostic {
                    range,
                    severity: Some(severity),
                    code: None,
                    code_description: None,
                    source: Some("latex".into()),
                    message: error.message.clone(),
                    related_information: None,
                    tags: None,
                    data: None,
                };

                let full_path = base_path.join(&error.relative_path);

                let child = if full_path.starts_with(&base_path) {
                    error
                        .relative_path
                        .to_str()
                        .and_then(|path| {
                            db.lookup_intern_document(root_document).uri.join(path).ok()
                        })
                        .map(|uri| db.intern_document(DocumentData::from(uri)))
                        .unwrap_or(root_document)
                } else {
                    root_document
                };

                diag_map.entry(child).or_default().push_back(diag);
            }
        }
    }

    diag_map
}
