use rowan::ast::AstNode;

use crate::syntax::latex::{self, HasCurly};

use super::{Extras, TheoremEnvironment};

pub fn analyze_theorem_definition(extras: &mut Extras, node: latex::SyntaxNode) -> Option<()> {
    let theorem = latex::TheoremDefinition::cast(node)?;
    let name = theorem.name()?.key()?.to_string();
    let description = theorem.description()?;
    let description = description.content_text()?;

    extras
        .theorem_environments
        .push(TheoremEnvironment { name, description });

    Some(())
}
