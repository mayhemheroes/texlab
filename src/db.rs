mod analysis;
mod client_capabilities;
mod client_info;
mod client_options;
mod diag;
mod distro;
mod document;
mod location;
mod syntax;
mod workspace;

use std::{path::Path, sync::Arc};

use anyhow::Result;
use lsp_types::{ClientCapabilities, Url};
use rustc_hash::FxHashSet;

use crate::{
    component_db::COMPONENT_DATABASE,
    distro::{DistributionKind, Resolver},
    Options,
};

pub use self::{
    analysis::*, client_capabilities::*, client_info::*, client_options::*, diag::*, distro::*,
    document::*, location::*, syntax::*, workspace::*,
};

#[salsa::database(
    AnalysisDatabaseStorage,
    ClientCapabilitiesDatabaseStorage,
    ClientInfoDatabaseStorage,
    ClientOptionsDatabaseStorage,
    DiagnosticsDatabaseStorage,
    DistroDatabaseStorage,
    DocumentDatabaseStorage,
    LocationDatabaseStorage,
    SyntaxDatabaseStorage,
    WorkspaceDatabaseStorage
)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
}

impl salsa::Database for RootDatabase {}

impl salsa::ParallelDatabase for RootDatabase {
    fn snapshot(&self) -> salsa::Snapshot<Self> {
        salsa::Snapshot::new(Self {
            storage: self.storage.snapshot(),
        })
    }
}

impl Default for RootDatabase {
    fn default() -> Self {
        let storage = salsa::Storage::default();
        let mut db = Self { storage };
        db.set_current_directory(Arc::new(std::env::temp_dir()));
        db.set_client_capabilities(Arc::new(ClientCapabilities::default()));
        db.set_client_info(None);
        db.set_client_options(Arc::new(Options::default()));
        db.set_distro_kind(DistributionKind::Unknown);
        db.set_distro_resolver(Arc::new(Resolver::default()));
        db.set_all_documents(im::HashSet::new());
        db
    }
}

impl RootDatabase {
    pub fn upsert_document(
        &mut self,
        document: Document,
        source_code: Arc<String>,
        language: DocumentLanguage,
    ) {
        self.set_source_code(document, source_code);
        self.set_language(document, language);

        let mut all_documents = self.all_documents();
        all_documents.insert(document);
        self.set_all_documents(all_documents);

        self.expand_parent(document);
        self.expand_children(document);
    }

    pub fn insert_hidden_document(&mut self, path: &Path) -> Result<()> {
        let uri = Url::from_file_path(&path).unwrap();
        let document = self.intern_document(uri.into());

        if self.all_documents().contains(&document) {
            return Ok(());
        }

        let source_data = std::fs::read(&path)?;
        let source_code = Arc::new(String::from_utf8_lossy(&source_data).into_owned());
        let language = DocumentLanguage::by_path(path).unwrap();
        self.set_visibility(document, DocumentVisibility::Hidden);
        self.upsert_document(document, source_code, language);
        Ok(())
    }

    fn expand_parent(&mut self, document: Document) {
        let all_document_paths = self
            .all_documents()
            .into_iter()
            .map(|document| self.lookup_intern_document(document).uri)
            .filter_map(|uri| uri.to_file_path().ok())
            .collect::<FxHashSet<_>>();

        let document_uri = self.lookup_intern_document(document).uri;
        if document_uri.scheme() == "file" {
            if let Ok(mut path) = document_uri.to_file_path() {
                while path.pop() && self.find_parent(document).is_none() {
                    std::fs::read_dir(&path)
                        .into_iter()
                        .flatten()
                        .filter_map(|entry| entry.ok())
                        .filter(|entry| entry.file_type().ok().filter(|ty| ty.is_file()).is_some())
                        .map(|entry| entry.path())
                        .filter(|path| {
                            matches!(
                                DocumentLanguage::by_path(path),
                                Some(DocumentLanguage::Latex)
                            )
                        })
                        .filter(|path| !all_document_paths.contains(path))
                        .for_each(|path| {
                            let _ = self.insert_hidden_document(&path);
                        });
                }
            }
        }
    }

    fn expand_children(&mut self, document: Document) {
        let mut all_targets = vec![
            self.linked_auxiliary_files(document, AuxiliaryFileKind::Aux),
            self.linked_auxiliary_files(document, AuxiliaryFileKind::Log),
        ];

        for link in &self.extras(document).explicit_links {
            if link
                .as_component_name()
                .and_then(|name| COMPONENT_DATABASE.find(&name))
                .is_none()
            {
                all_targets.push(link.targets.clone());
            }
        }

        all_targets.into_iter().for_each(|targets| {
            for target in targets.iter().copied() {
                let uri = self.lookup_intern_document(target).uri;
                if uri.scheme() == "file"
                    && uri.fragment().is_none()
                    && uri
                        .to_file_path()
                        .ok()
                        .and_then(|path| self.insert_hidden_document(&path).ok())
                        .is_some()
                {
                    break;
                }
            }
        });
    }
}
