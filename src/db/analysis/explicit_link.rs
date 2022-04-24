use std::sync::Arc;

use rowan::ast::AstNode;

use crate::{db::Document, syntax::latex};

use super::{
    distro_file::resolve_distro_file, AnalysisDatabase, ExplicitLink, ExplicitLinkKind, Extras,
};

pub fn analyze_include(
    extras: &mut Extras,
    db: &dyn AnalysisDatabase,
    document: Document,
    node: latex::SyntaxNode,
) -> Option<()> {
    let include = latex::Include::cast(node)?;
    let kind = match include.syntax().kind() {
        latex::LATEX_INCLUDE => ExplicitLinkKind::Latex,
        latex::BIBLATEX_INCLUDE | latex::BIBTEX_INCLUDE => ExplicitLinkKind::Bibtex,
        latex::PACKAGE_INCLUDE => ExplicitLinkKind::Package,
        latex::CLASS_INCLUDE => ExplicitLinkKind::Class,
        _ => return None,
    };

    let extensions = match kind {
        ExplicitLinkKind::Latex => &["tex"],
        ExplicitLinkKind::Bibtex => &["bib"],
        ExplicitLinkKind::Package => &["sty"],
        ExplicitLinkKind::Class => &["cls"],
    };

    let base_uri = db.base_uri(document);
    for path in include.path_list()?.keys() {
        let stem = path.to_string();
        let mut targets = vec![Arc::new(base_uri.join(&stem).ok()?)];
        for extension in extensions {
            let path = format!("{}.{}", stem, extension);
            targets.push(Arc::new(base_uri.join(&path).ok()?));
        }

        resolve_distro_file(db.distro_resolver().as_ref(), &stem, extensions)
            .into_iter()
            .for_each(|target| targets.push(Arc::new(target)));

        extras.explicit_links.push(ExplicitLink {
            kind,
            stem: stem.into(),
            stem_range: latex::small_range(&path),
            targets,
        });
    }

    Some(())
}

pub fn analyze_import(
    extras: &mut Extras,
    db: &dyn AnalysisDatabase,
    document: Document,
    node: latex::SyntaxNode,
) -> Option<()> {
    let import = latex::Import::cast(node)?;

    let base_uri = db.base_uri(document);
    let mut targets = Vec::new();
    let directory = import
        .directory()
        .and_then(|dir| dir.key())
        .and_then(|dir| base_uri.join(&dir.to_string()).ok())
        .map(Arc::new)
        .unwrap_or(base_uri);

    let file = import.file()?.key()?;
    let stem = file.to_string();
    targets.push(Arc::new(directory.join(&stem).ok()?));
    targets.push(Arc::new(directory.join(&format!("{}.tex", stem)).ok()?));

    extras.explicit_links.push(ExplicitLink {
        stem: stem.into(),
        stem_range: latex::small_range(&file),
        targets,
        kind: ExplicitLinkKind::Latex,
    });
    Some(())
}
