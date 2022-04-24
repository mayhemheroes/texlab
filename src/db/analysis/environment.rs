use rowan::ast::AstNode;

use crate::syntax::latex;

use super::Extras;

pub fn analyze_begin(extras: &mut Extras, node: latex::SyntaxNode) -> Option<()> {
    let begin = latex::Begin::cast(node)?;
    let name = begin.name()?.key()?.to_string();
    extras.environment_names.insert(name);
    Some(())
}
