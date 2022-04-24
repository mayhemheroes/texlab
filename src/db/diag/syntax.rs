use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};
use rowan::{ast::AstNode, TextRange};

use crate::{
    db::{Document, SyntaxTree},
    syntax::{
        bibtex::{self, HasDelimiters, HasType},
        latex,
    },
    LineIndexExt,
};

use super::DiagnosticsDatabase;

pub fn analyze_latex_static(
    db: &dyn DiagnosticsDatabase,
    document: Document,
) -> im::Vector<Diagnostic> {
    let mut diags = im::Vector::new();
    if !db
        .lookup_intern_document(document)
        .uri
        .as_str()
        .ends_with(".tex")
    {
        return diags;
    }

    if let SyntaxTree::Latex(green) = db.syntax_tree(document) {
        for node in latex::SyntaxNode::new_root(green).descendants() {
            analyze_environment(db, document, &mut diags, node.clone())
                .or_else(|| analyze_curly_group(db, document, &mut diags, node.clone()))
                .or_else(|| {
                    if node.kind() == latex::ERROR && node.first_token()?.text() == "}" {
                        diags.push_back(Diagnostic {
                            range: db
                                .line_index(document)
                                .line_col_lsp_range(node.text_range()),
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: Some(NumberOrString::Number(1)),
                            code_description: None,
                            source: Some("texlab".to_string()),
                            message: "Unexpected \"}\"".to_string(),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                        Some(())
                    } else {
                        None
                    }
                });
        }
    }

    diags
}

fn analyze_environment(
    db: &dyn DiagnosticsDatabase,
    document: Document,
    diags: &mut im::Vector<Diagnostic>,
    node: latex::SyntaxNode,
) -> Option<()> {
    let environment = latex::Environment::cast(node)?;
    let name1 = environment.begin()?.name()?.key()?;
    let name2 = environment.end()?.name()?.key()?;
    if name1 != name2 {
        diags.push_back(Diagnostic {
            range: db
                .line_index(document)
                .line_col_lsp_range(latex::small_range(&name1)),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::Number(3)),
            code_description: None,
            source: Some("texlab".to_string()),
            message: "Mismatched environment".to_string(),
            related_information: None,
            tags: None,
            data: None,
        });
    }
    Some(())
}

fn analyze_curly_group(
    db: &dyn DiagnosticsDatabase,
    document: Document,
    diags: &mut im::Vector<Diagnostic>,
    node: latex::SyntaxNode,
) -> Option<()> {
    if !matches!(
        node.kind(),
        latex::CURLY_GROUP
            | latex::CURLY_GROUP_COMMAND
            | latex::CURLY_GROUP_KEY_VALUE
            | latex::CURLY_GROUP_WORD
            | latex::CURLY_GROUP_WORD_LIST
    ) {
        return None;
    }

    let is_inside_verbatim_environment = node
        .ancestors()
        .filter_map(latex::Environment::cast)
        .filter_map(|env| env.begin())
        .filter_map(|begin| begin.name())
        .filter_map(|name| name.key())
        .any(|name| {
            ["asy", "lstlisting", "minted", "verbatim"].contains(&name.to_string().as_str())
        });

    if !is_inside_verbatim_environment
        && !node
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .any(|token| token.kind() == latex::R_CURLY)
    {
        diags.push_back(Diagnostic {
            range: db
                .line_index(document)
                .line_col_lsp_range(TextRange::empty(node.text_range().end())),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::Number(2)),
            code_description: None,
            source: Some("texlab".to_string()),
            message: "Missing \"}\" inserted".to_string(),
            related_information: None,
            tags: None,
            data: None,
        });
    }

    Some(())
}

pub fn analyze_bibtex_static(
    db: &dyn DiagnosticsDatabase,
    document: Document,
) -> im::Vector<Diagnostic> {
    let mut diags = im::Vector::new();
    if let SyntaxTree::Bibtex(green) = db.syntax_tree(document) {
        for node in bibtex::SyntaxNode::new_root(green).descendants() {
            analyze_entry(db, document, &mut diags, node.clone())
                .or_else(|| analyze_field(db, document, &mut diags, node));
        }
    }

    diags
}

fn analyze_entry(
    db: &dyn DiagnosticsDatabase,
    document: Document,
    diags: &mut im::Vector<Diagnostic>,
    node: bibtex::SyntaxNode,
) -> Option<()> {
    let entry = bibtex::Entry::cast(node)?;
    if entry.left_delimiter().is_none() {
        diags.push_back(Diagnostic {
            range: db
                .line_index(document)
                .line_col_lsp_range(entry.ty()?.text_range()),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::Number(4)),
            code_description: None,
            source: Some("texlab".to_string()),
            message: "Expecting a curly bracket: \"{\"".to_string(),
            related_information: None,
            tags: None,
            data: None,
        });
        return Some(());
    }

    if entry.key().is_none() {
        diags.push_back(Diagnostic {
            range: db
                .line_index(document)
                .line_col_lsp_range(entry.left_delimiter()?.text_range()),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::Number(5)),
            code_description: None,
            source: Some("texlab".to_string()),
            message: "Expecting a key".to_string(),
            related_information: None,
            tags: None,
            data: None,
        });
        return Some(());
    }

    if entry.key().is_none() {
        diags.push_back(Diagnostic {
            range: db
                .line_index(document)
                .line_col_lsp_range(entry.right_delimiter()?.text_range()),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::Number(6)),
            code_description: None,
            source: Some("texlab".to_string()),
            message: "Expecting a curly bracket: \"}\"".to_string(),
            related_information: None,
            tags: None,
            data: None,
        });
        return Some(());
    }

    Some(())
}

fn analyze_field(
    db: &dyn DiagnosticsDatabase,
    document: Document,
    diags: &mut im::Vector<Diagnostic>,
    node: bibtex::SyntaxNode,
) -> Option<()> {
    let field = bibtex::Field::cast(node)?;
    if field.equality_sign().is_none() {
        diags.push_back(Diagnostic {
            range: db
                .line_index(document)
                .line_col_lsp_range(TextRange::empty(field.name()?.text_range().end())),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::Number(7)),
            code_description: None,
            source: Some("texlab".to_string()),
            message: "Expecting an equality sign: \"=\"".to_string(),
            related_information: None,
            tags: None,
            data: None,
        });
        return Some(());
    }

    if field.value().is_none() {
        diags.push_back(Diagnostic {
            range: db
                .line_index(document)
                .line_col_lsp_range(TextRange::empty(field.equality_sign()?.text_range().end())),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::Number(8)),
            code_description: None,
            source: Some("texlab".to_string()),
            message: "Expecting a field value".to_string(),
            related_information: None,
            tags: None,
            data: None,
        });
        return Some(());
    }

    Some(())
}
