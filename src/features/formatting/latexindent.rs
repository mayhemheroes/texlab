use std::{
    fs,
    process::{Command, Stdio},
};

use lsp_types::{DocumentFormattingParams, TextEdit};
use rowan::{TextLen, TextRange};
use tempfile::tempdir;

use crate::{
    db::{ClientOptionsDatabase, DocumentDatabase},
    features::FeatureRequest,
    DocumentLanguage, LineIndexExt,
};

pub fn format_with_latexindent(
    request: &FeatureRequest<DocumentFormattingParams>,
) -> Option<Vec<TextEdit>> {
    let directory = tempdir().ok()?;

    let options = request.db.client_options();
    let current_dir = request
        .db
        .root_directory()
        .as_deref()
        .cloned()
        .or_else(|| {
            let document_uri = request.db.lookup_intern_document(request.document).uri;
            if document_uri.scheme() == "file" {
                document_uri
                    .to_file_path()
                    .unwrap()
                    .parent()
                    .map(ToOwned::to_owned)
            } else {
                None
            }
        })
        .unwrap_or_else(|| ".".into());

    let local = match &options.latexindent.local {
        Some(local) => format!("--local={}", local),
        None => "-l".to_string(),
    };

    let modify_line_breaks = options.latexindent.modify_line_breaks;

    let path = directory.path();
    let _ = fs::copy(
        current_dir.join("localSettings.yaml"),
        path.join("localSettings.yaml"),
    );
    let _ = fs::copy(
        current_dir.join(".localSettings.yaml"),
        path.join(".localSettings.yaml"),
    );
    let _ = fs::copy(
        current_dir.join("latexindent.yaml"),
        path.join("latexindent.yaml"),
    );

    let name = if request.db.language(request.document) == DocumentLanguage::Bibtex {
        "file.bib"
    } else {
        "file.tex"
    };

    let text = request.db.source_code(request.document);
    fs::write(directory.path().join(name), text.as_str()).ok()?;

    let mut args = Vec::new();
    if modify_line_breaks {
        args.push("--modifylinebreaks");
    }
    args.push(&local);
    args.push(name);

    let output = Command::new("latexindent")
        .args(&args)
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(directory.path())
        .output()
        .ok()?;

    let new_text = String::from_utf8_lossy(&output.stdout).into_owned();
    if new_text.is_empty() {
        None
    } else {
        Some(vec![TextEdit {
            range: request
                .db
                .line_index(request.document)
                .line_col_lsp_range(TextRange::new(0.into(), text.text_len())),
            new_text,
        }])
    }
}
