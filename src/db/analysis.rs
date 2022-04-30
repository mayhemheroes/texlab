mod command;
mod distro_file;
mod environment;
mod explicit_link;
mod graphics_path;
// mod implicit_link;
mod label_name;
mod label_number;
mod theorem;

use std::sync::Arc;

use lsp_types::Url;
use rowan::TextRange;
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;

use crate::syntax::latex;

use self::{
    command::{analyze_command, analyze_command_definition},
    environment::analyze_begin,
    explicit_link::{analyze_import, analyze_include},
    graphics_path::analyze_graphics_path,
    label_name::analyze_label_name,
    label_number::analyze_label_number,
    theorem::analyze_theorem_definition,
};

use super::{
    ClientOptionsDatabase, DistroDatabase, Document, DocumentDatabase, SyntaxDatabase, SyntaxTree,
};

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Extras {
    pub explicit_links: Vec<ExplicitLink>,
    pub has_document_environment: bool,
    pub command_names: FxHashSet<SmolStr>,
    pub environment_names: FxHashSet<String>,
    pub label_names: Vec<LabelName>,
    pub label_numbers_by_name: FxHashMap<String, String>,
    pub theorem_environments: Vec<TheoremEnvironment>,
    pub graphics_paths: FxHashSet<String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum ExplicitLinkKind {
    Package,
    Class,
    Latex,
    Bibtex,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExplicitLink {
    pub stem: SmolStr,
    pub stem_range: TextRange,
    pub targets: im::Vector<Document>,
    pub kind: ExplicitLinkKind,
}

impl ExplicitLink {
    pub fn as_component_name(&self) -> Option<String> {
        match self.kind {
            ExplicitLinkKind::Package => Some(format!("{}.sty", self.stem)),
            ExplicitLinkKind::Class => Some(format!("{}.cls", self.stem)),
            ExplicitLinkKind::Latex | ExplicitLinkKind::Bibtex => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default, Hash)]
pub struct TheoremEnvironment {
    pub name: String,
    pub description: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Default, Hash)]
pub struct LabelName {
    pub text: SmolStr,
    pub range: TextRange,
    pub is_definition: bool,
}

#[salsa::query_group(AnalysisDatabaseStorage)]
pub trait AnalysisDatabase:
    salsa::Database + DocumentDatabase + SyntaxDatabase + ClientOptionsDatabase + DistroDatabase
{
    fn base_uri(&self, document: Document) -> Arc<Url>;

    fn extras(&self, document: Document) -> Arc<Extras>;
}

fn base_uri(db: &dyn AnalysisDatabase, document: Document) -> Arc<Url> {
    db.root_directory()
        .as_deref()
        .and_then(|path| Url::from_directory_path(path).ok())
        .map(Arc::new)
        .unwrap_or_else(|| db.lookup_intern_document(document).uri)
}

fn extras(db: &dyn AnalysisDatabase, document: Document) -> Arc<Extras> {
    let mut extras = Extras::default();
    if let SyntaxTree::Latex(green) = db.syntax_tree(document) {
        let root = latex::SyntaxNode::new_root(green);
        for node in root.descendants() {
            analyze_command(&mut extras, node.clone())
                .or_else(|| analyze_command_definition(&mut extras, node.clone()))
                .or_else(|| analyze_begin(&mut extras, node.clone()))
                .or_else(|| analyze_include(&mut extras, db, document, node.clone()))
                .or_else(|| analyze_import(&mut extras, db, document, node.clone()))
                .or_else(|| analyze_label_name(&mut extras, node.clone()))
                .or_else(|| analyze_label_number(&mut extras, node.clone()))
                .or_else(|| analyze_theorem_definition(&mut extras, node.clone()))
                .or_else(|| analyze_graphics_path(&mut extras, node));
        }

        extras.has_document_environment = extras.environment_names.contains("document");
    }

    Arc::new(extras)
}
