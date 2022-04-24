use latex::LabelReferenceRange;
use rowan::ast::AstNode;

use crate::syntax::latex;

use super::{Extras, LabelName};

pub fn analyze_label_name(extras: &mut Extras, node: latex::SyntaxNode) -> Option<()> {
    analyze_label_definition_name(extras, node.clone())
        .or_else(|| analyze_label_reference_name(extras, node.clone()))
        .or_else(|| analyze_label_reference_range_name(extras, node))
}

fn analyze_label_definition_name(extras: &mut Extras, node: latex::SyntaxNode) -> Option<()> {
    let label = latex::LabelDefinition::cast(node)?;
    let name = label.name()?.key()?;
    extras.label_names.push(LabelName {
        text: name.to_string().into(),
        range: latex::small_range(&name),
        is_definition: true,
    });
    Some(())
}

fn analyze_label_reference_name(extras: &mut Extras, node: latex::SyntaxNode) -> Option<()> {
    let label = latex::LabelReference::cast(node)?;
    for name in label.name_list()?.keys() {
        extras.label_names.push(LabelName {
            text: name.to_string().into(),
            range: latex::small_range(&name),
            is_definition: false,
        });
    }
    Some(())
}

fn analyze_label_reference_range_name(extras: &mut Extras, node: latex::SyntaxNode) -> Option<()> {
    let label = LabelReferenceRange::cast(node)?;
    if let Some(name1) = label.from().and_then(|name| name.key()) {
        extras.label_names.push(LabelName {
            text: name1.to_string().into(),
            range: latex::small_range(&name1),
            is_definition: false,
        });
    }

    if let Some(name2) = label.to().and_then(|name| name.key()) {
        extras.label_names.push(LabelName {
            text: name2.to_string().into(),
            range: latex::small_range(&name2),
            is_definition: false,
        });
    }
    Some(())
}
