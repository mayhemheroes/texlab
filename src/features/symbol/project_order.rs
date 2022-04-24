use petgraph::{algo::tarjan_scc, Directed, Graph};
use rustc_hash::FxHashSet;

use crate::db::{
    AnalysisDatabase, Document, DocumentData, DocumentDatabase, RootDatabase, WorkspaceDatabase,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ProjectOrdering {
    ordering: Vec<Document>,
}

impl ProjectOrdering {
    pub fn get(&self, document: Document) -> usize {
        self.ordering
            .iter()
            .position(|d| *d == document)
            .unwrap_or(std::usize::MAX)
    }
}

impl From<&RootDatabase> for ProjectOrdering {
    fn from(db: &RootDatabase) -> Self {
        let mut ordering = Vec::new();

        let comps = connected_components(db);
        for comp in comps {
            let graph = build_dependency_graph(db, &comp);

            let mut visited = FxHashSet::default();
            let root_index = *graph.node_weight(tarjan_scc(&graph)[0][0]).unwrap();
            let mut stack = vec![comp[root_index]];

            while let Some(document) = stack.pop() {
                if !visited.insert(document) {
                    continue;
                }

                ordering.push(document);
                for link in db.extras(document).explicit_links.iter().rev() {
                    for target in link
                        .targets
                        .iter()
                        .cloned()
                        .map(|uri| db.intern_document(DocumentData { uri }))
                    {
                        if db.all_documents().contains(&target) {
                            stack.push(target);
                        }
                    }
                }
            }
        }

        Self { ordering }
    }
}

fn connected_components(db: &RootDatabase) -> Vec<im::Vector<Document>> {
    let mut components = Vec::new();
    let mut visited = FxHashSet::default();
    for root_document in db.all_documents() {
        if !visited.insert(root_document) {
            continue;
        }

        let unit = db.compilation_unit(root_document);
        visited.extend(unit.iter());
        components.push(unit);
    }

    components
}

fn build_dependency_graph(
    db: &RootDatabase,
    documents: &im::Vector<Document>,
) -> Graph<usize, (), Directed> {
    let mut graph = Graph::new();
    let nodes: Vec<_> = (0..documents.len()).map(|i| graph.add_node(i)).collect();

    for (i, document) in documents.iter().copied().enumerate() {
        let extras = db.extras(document);
        for link in &extras.explicit_links {
            for target in link
                .targets
                .iter()
                .cloned()
                .map(|uri| db.intern_document(DocumentData { uri }))
            {
                if let Some(j) = documents
                    .iter()
                    .copied()
                    .position(|document| document == target)
                {
                    graph.add_edge(nodes[j], nodes[i], ());
                    break;
                }
            }
        }
    }

    graph
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use lsp_types::Url;

    use crate::DocumentLanguage;

    use super::*;

    #[test]
    fn test_no_cycles() -> Result<()> {
        let mut db = RootDatabase::default();

        let a = db.intern_document(DocumentData::from(Url::parse("http://example.com/a.tex")?));
        let b = db.intern_document(DocumentData::from(Url::parse("http://example.com/b.tex")?));
        let c = db.intern_document(DocumentData::from(Url::parse("http://example.com/c.tex")?));

        db.upsert_document(a, Arc::new(String::new()), DocumentLanguage::Latex);

        db.upsert_document(b, Arc::new(String::new()), DocumentLanguage::Latex);

        db.upsert_document(
            c,
            Arc::new(r#"\include{b}\include{a}"#.to_string()),
            DocumentLanguage::Latex,
        );

        let ordering = ProjectOrdering::from(&db);

        assert_eq!(ordering.get(a), 2);
        assert_eq!(ordering.get(b), 1);
        assert_eq!(ordering.get(c), 0);
        Ok(())
    }

    #[test]
    fn test_cycles() -> Result<()> {
        let mut db = RootDatabase::default();

        let a = db.intern_document(DocumentData::from(Url::parse("http://example.com/a.tex")?));
        let b = db.intern_document(DocumentData::from(Url::parse("http://example.com/b.tex")?));
        let c = db.intern_document(DocumentData::from(Url::parse("http://example.com/c.tex")?));

        db.upsert_document(
            a,
            Arc::new(r#"\include{b}"#.to_string()),
            DocumentLanguage::Latex,
        );

        db.upsert_document(
            b,
            Arc::new(r#"\include{a}"#.to_string()),
            DocumentLanguage::Latex,
        );

        db.upsert_document(
            c,
            Arc::new(r#"\include{a}"#.to_string()),
            DocumentLanguage::Latex,
        );

        let ordering = ProjectOrdering::from(&db);

        assert_eq!(ordering.get(a), 1);
        assert_eq!(ordering.get(b), 2);
        assert_eq!(ordering.get(c), 0);
        Ok(())
    }

    #[test]
    fn test_multiple_roots() -> Result<()> {
        let mut db = RootDatabase::default();

        let a = db.intern_document(DocumentData::from(Url::parse("http://example.com/a.tex")?));
        let b = db.intern_document(DocumentData::from(Url::parse("http://example.com/b.tex")?));
        let c = db.intern_document(DocumentData::from(Url::parse("http://example.com/c.tex")?));
        let d = db.intern_document(DocumentData::from(Url::parse("http://example.com/d.tex")?));

        db.upsert_document(
            a,
            Arc::new(r#"\include{b}"#.to_string()),
            DocumentLanguage::Latex,
        );

        db.upsert_document(b, Arc::new(r#""#.to_string()), DocumentLanguage::Latex);

        db.upsert_document(c, Arc::new(r#""#.to_string()), DocumentLanguage::Latex);

        db.upsert_document(
            d,
            Arc::new(r#"\include{c}"#.to_string()),
            DocumentLanguage::Latex,
        );

        let ordering = ProjectOrdering::from(&db);

        assert!(ordering.get(a) < ordering.get(b));
        assert!(ordering.get(d) < ordering.get(c));
        Ok(())
    }
}
