use std::sync::Arc;

use lsp_types::Url;

use crate::{DocumentLanguage, LineIndex};

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

impl Document {
    pub fn lookup(self, db: &dyn DocumentDatabase) -> DocumentData {
        db.lookup_intern_document(self)
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct DocumentData {
    pub uri: Arc<Url>,
}

impl From<Url> for DocumentData {
    fn from(uri: Url) -> Self {
        Self { uri: Arc::new(uri) }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum DocumentVisibility {
    Visible,
    Hidden,
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
