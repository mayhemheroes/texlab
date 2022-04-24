use rowan::ast::AstNode;

use crate::syntax::latex;

use super::Extras;

pub fn analyze_command(extras: &mut Extras, node: latex::SyntaxNode) -> Option<()> {
    let command = latex::GenericCommand::cast(node)?;
    extras.command_names.insert(command.name()?.text().into());
    Some(())
}

pub fn analyze_command_definition(extras: &mut Extras, node: latex::SyntaxNode) -> Option<()> {
    let definition = latex::CommandDefinition::cast(node)?;
    extras
        .command_names
        .insert(definition.name()?.command()?.text().into());
    Some(())
}
