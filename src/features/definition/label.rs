use lsp_types::{GotoDefinitionParams, LocationLink};

use crate::{
    db::{DocumentDatabase, SyntaxDatabase, SyntaxTree, WorkspaceDatabase},
    features::cursor::CursorContext,
    find_label_definition, render_label,
    syntax::latex,
    LineIndexExt,
};

pub fn goto_label_definition(
    context: &CursorContext<GotoDefinitionParams>,
) -> Option<Vec<LocationLink>> {
    let (name_text, name_range) = context
        .find_label_name_key()
        .or_else(|| context.find_label_name_command())?;

    let origin_selection_range = context
        .request
        .db
        .line_index(context.request.document)
        .line_col_lsp_range(name_range);

    let unit = context
        .request
        .db
        .compilation_unit(context.request.document);
    for document in unit.iter().copied() {
        if let SyntaxTree::Latex(green) = context.request.db.syntax_tree(document) {
            if let Some(definition) =
                find_label_definition(&latex::SyntaxNode::new_root(green), &name_text)
            {
                let target_selection_range = latex::small_range(&definition.name()?.key()?);
                let target_range =
                    render_label(context.request.db, &unit, &name_text, Some(definition))
                        .map(|label| label.range)
                        .unwrap_or(target_selection_range);

                let target_uri = context.request.db.lookup_intern_document(document).uri;
                let line_index = context.request.db.line_index(document);
                return Some(vec![LocationLink {
                    origin_selection_range: Some(origin_selection_range),
                    target_uri: target_uri.as_ref().clone(),
                    target_range: line_index.line_col_lsp_range(target_range),
                    target_selection_range: line_index.line_col_lsp_range(target_selection_range),
                }]);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::features::testing::FeatureTester;

    use super::*;

    #[test]
    fn test_empty_latex_document() {
        let request = FeatureTester::builder()
            .files(vec![("main.tex", "")])
            .main("main.tex")
            .line(0)
            .character(0)
            .build()
            .definition();

        let context = CursorContext::new(request);
        let actual_links = goto_label_definition(&context);

        assert!(actual_links.is_none());
    }

    #[test]
    fn test_empty_bibtex_document() {
        let request = FeatureTester::builder()
            .files(vec![("main.bib", "")])
            .main("main.bib")
            .line(0)
            .character(0)
            .build()
            .definition();

        let context = CursorContext::new(request);
        let actual_links = goto_label_definition(&context);

        assert!(actual_links.is_none());
    }
}
