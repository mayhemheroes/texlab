use std::sync::Arc;

use crate::syntax::{
    bibtex,
    build_log::{self, BuildError},
    latex,
};

use super::{Document, DocumentDatabase, DocumentLanguage};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum SyntaxTree {
    Latex(rowan::GreenNode),
    Bibtex(rowan::GreenNode),
    BuildLog(Arc<Vec<BuildError>>),
}

impl SyntaxTree {
    pub fn into_latex(self) -> Option<rowan::GreenNode> {
        match self {
            SyntaxTree::Latex(green) => Some(green),
            _ => None,
        }
    }

    pub fn into_bibtex(self) -> Option<rowan::GreenNode> {
        match self {
            SyntaxTree::Bibtex(green) => Some(green),
            _ => None,
        }
    }
}

#[salsa::query_group(SyntaxDatabaseStorage)]
pub trait SyntaxDatabase: salsa::Database + DocumentDatabase {
    fn syntax_tree(&self, document: Document) -> SyntaxTree;
}

fn syntax_tree(db: &dyn SyntaxDatabase, document: Document) -> SyntaxTree {
    let text = db.source_code(document);
    match db.language(document) {
        DocumentLanguage::Latex => {
            let green = latex::parse(&text);
            SyntaxTree::Latex(green)
        }
        DocumentLanguage::Bibtex => {
            let tree = bibtex::parse(&text);
            SyntaxTree::Bibtex(tree)
        }
        DocumentLanguage::BuildLog => {
            let errors = Arc::new(build_log::parse(&text).errors);
            SyntaxTree::BuildLog(errors)
        }
    }
}
