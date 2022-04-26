use std::{ffi::OsStr, path::Path, sync::Arc};

use derive_more::From;
use lsp_types::Url;

use crate::LineIndex;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Document(salsa::InternId);

impl salsa::InternKey for Document {
    fn from_intern_id(v: salsa::InternId) -> Self {
        Self(v)
    }

    fn as_intern_id(&self) -> salsa::InternId {
        self.0
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, From)]
#[from(forward)]
pub struct DocumentData {
    pub uri: Arc<Url>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum DocumentVisibility {
    Visible,
    Hidden,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd, Ord)]
pub enum DocumentLanguage {
    Latex,
    Bibtex,
    BuildLog,
}

impl DocumentLanguage {
    pub fn by_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(OsStr::to_str)
            .and_then(Self::by_extension)
    }

    pub fn by_extension(extension: &str) -> Option<Self> {
        match extension.to_lowercase().as_str() {
            "tex" | "sty" | "cls" | "def" | "lco" | "aux" | "rnw" => Some(Self::Latex),
            "bib" | "bibtex" => Some(Self::Bibtex),
            "log" => Some(Self::BuildLog),
            _ => None,
        }
    }

    pub fn by_language_id(language_id: &str) -> Option<Self> {
        match language_id {
            "latex" | "tex" => Some(Self::Latex),
            "bibtex" | "bib" => Some(Self::Bibtex),
            _ => None,
        }
    }
}

#[salsa::query_group(DocumentDatabaseStorage)]
pub trait DocumentDatabase: salsa::Database {
    #[salsa::interned]
    fn intern_document(&self, data: DocumentData) -> Document;

    #[salsa::input]
    fn all_documents(&self) -> im::HashSet<Document>;

    #[salsa::input]
    fn source_code(&self, document: Document) -> Arc<String>;

    #[salsa::input]
    fn language(&self, document: Document) -> DocumentLanguage;

    #[salsa::input]
    fn visibility(&self, document: Document) -> DocumentVisibility;

    fn line_index(&self, document: Document) -> Arc<LineIndex>;
}

fn line_index(db: &dyn DocumentDatabase, document: Document) -> Arc<LineIndex> {
    let text = db.source_code(document);
    Arc::new(LineIndex::new(text.as_str()))
}
