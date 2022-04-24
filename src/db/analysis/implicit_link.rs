use std::sync::Arc;

use lsp_types::Url;

use crate::db::Document;

use super::{AnalysisDatabase, Extras};

pub fn analyze_implicit_links(extras: &mut Extras, db: &dyn AnalysisDatabase, document: Document) {
    extras.implicit_links.aux = find_by_extension(db, document, "aux").unwrap_or_default();
    extras.implicit_links.log = find_by_extension(db, document, "log").unwrap_or_default();
    extras.implicit_links.pdf = find_by_extension(db, document, "pdf").unwrap_or_default();
}

fn find_by_extension(
    db: &dyn AnalysisDatabase,
    document: Document,
    extension: &str,
) -> Option<Vec<Arc<Url>>> {
    let document_uri = db.lookup_intern_document(document).uri;
    let mut targets = vec![Arc::new(with_extension(&document_uri, extension)?)];
    if document_uri.scheme() == "file" {
        let file_path = document_uri.to_file_path().ok()?;
        let file_stem = file_path.file_stem()?;
        let aux_name = format!("{}.{}", file_stem.to_str()?, extension);

        if let Some(root_dir) = db.root_directory() {
            let path = root_dir.join(&aux_name);
            targets.push(Arc::new(Url::from_file_path(path).ok()?));
        }

        if let Some(aux_dir) = db.aux_directory() {
            let path = aux_dir.join(&aux_name);
            targets.push(Arc::new(Url::from_file_path(path).ok()?));
        }
    }

    Some(targets)
}

fn with_extension(uri: &Url, extension: &str) -> Option<Url> {
    let file_name = uri.path_segments()?.last()?;
    let file_stem = file_name
        .rfind('.')
        .map(|i| &file_name[..i])
        .unwrap_or(file_name);

    uri.join(&format!("{}.{}", file_stem, extension)).ok()
}
