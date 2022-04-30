use lsp_types::Url;
use petgraph::{graphmap::UnGraphMap, visit::Dfs};

use super::{AnalysisDatabase, Document};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum AuxiliaryFileKind {
    Aux,
    Log,
    Pdf,
}

impl AuxiliaryFileKind {
    pub fn extension(self) -> &'static str {
        match self {
            AuxiliaryFileKind::Aux => "aux",
            AuxiliaryFileKind::Log => "log",
            AuxiliaryFileKind::Pdf => "pdf",
        }
    }
}

#[salsa::query_group(WorkspaceDatabaseStorage)]
pub trait WorkspaceDatabase: salsa::Database + AnalysisDatabase {
    fn linked_auxiliary_files(
        &self,
        document: Document,
        kind: AuxiliaryFileKind,
    ) -> im::Vector<Document>;

    fn compilation_unit(&self, document: Document) -> im::Vector<Document>;

    fn find_parent(&self, document: Document) -> Option<Document>;
}

fn linked_auxiliary_files(
    db: &dyn WorkspaceDatabase,
    document: Document,
    kind: AuxiliaryFileKind,
) -> im::Vector<Document> {
    let document_uri = db.lookup_intern_document(document).uri;
    let mut targets = im::vector![db.intern_document(
        with_extension(&document_uri, kind.extension())
            .unwrap()
            .into(),
    )];

    if document_uri.scheme() == "file" {
        if let Some(name) = document_uri.to_file_path().ok().and_then(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| format!("{}.{}", stem, kind.extension()))
        }) {
            if let Some(uri) = db
                .root_directory()
                .map(|dir| dir.join(&name))
                .and_then(|path| Url::from_file_path(path).ok())
            {
                targets.push_back(db.intern_document(uri.into()));
            }

            if let Some(uri) = db
                .aux_directory()
                .map(|dir| dir.join(&name))
                .and_then(|path| Url::from_file_path(path).ok())
            {
                targets.push_back(db.intern_document(uri.into()));
            }
        }
    }

    targets
}

fn with_extension(uri: &Url, extension: &str) -> Option<Url> {
    let file_name = uri.path_segments()?.last()?;
    let file_stem = file_name
        .rfind('.')
        .map(|i| &file_name[..i])
        .unwrap_or(file_name);

    uri.join(&format!("{}.{}", file_stem, extension)).ok()
}

fn compilation_unit(db: &dyn WorkspaceDatabase, document: Document) -> im::Vector<Document> {
    let all_documents: Vec<_> = db.all_documents().iter().copied().collect();
    all_documents
        .iter()
        .position(|d| *d == document)
        .map(|start| {
            let mut edges = Vec::new();
            for (i, document) in all_documents.iter().copied().enumerate() {
                let extras = db.extras(document);

                let mut all_targets = vec![
                    db.linked_auxiliary_files(document, AuxiliaryFileKind::Aux),
                    db.linked_auxiliary_files(document, AuxiliaryFileKind::Log),
                ];

                for link in &extras.explicit_links {
                    all_targets.push(link.targets.clone());
                }

                for targets in all_targets {
                    if let Some(j) = targets
                        .iter()
                        .find_map(|target| all_documents.iter().position(|d| d == target))
                    {
                        edges.push((i, j, ()));
                    }
                }
            }

            let graph = UnGraphMap::from_edges(edges);
            let mut dfs = Dfs::new(&graph, start);
            let mut unit = im::Vector::new();
            while let Some(i) = dfs.next(&graph) {
                unit.push_back(all_documents[i]);
            }

            unit
        })
        .unwrap_or_default()
}

fn find_parent(db: &dyn WorkspaceDatabase, document: Document) -> Option<Document> {
    db.compilation_unit(document).into_iter().find(|document| {
        let extras = db.extras(*document);
        extras.has_document_environment
            && !extras
                .explicit_links
                .iter()
                .filter_map(|link| link.as_component_name())
                .any(|name| name == "subfiles.cls")
    })
}
