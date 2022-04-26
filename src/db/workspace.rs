use petgraph::{graphmap::UnGraphMap, visit::Dfs};

use super::{AnalysisDatabase, Document};

#[salsa::query_group(WorkspaceDatabaseStorage)]
pub trait WorkspaceDatabase: salsa::Database + AnalysisDatabase {
    fn compilation_unit(&self, document: Document) -> im::Vector<Document>;

    fn find_parent(&self, document: Document) -> Option<Document>;
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
                let mut all_targets = vec![&extras.implicit_links.aux, &extras.implicit_links.log];
                for link in &extras.explicit_links {
                    all_targets.push(&link.targets);
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
