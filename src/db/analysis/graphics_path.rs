use rowan::ast::AstNode;

use crate::syntax::latex;

use super::Extras;

pub fn analyze_graphics_path(extras: &mut Extras, node: latex::SyntaxNode) -> Option<()> {
    let definition = latex::GraphicsPath::cast(node)?;
    for path in definition
        .path_list()
        .filter_map(|group| group.key())
        .map(|path| path.to_string())
    {
        extras.graphics_paths.insert(path);
    }

    Some(())
}
