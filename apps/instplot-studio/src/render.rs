use export_backend_spike::{ResolvedDisplayList, resolve};

use crate::{DocumentLayout, DocumentLayoutError, FigureDocument};

/// The single formal layout and resolved graphics result consumed by every backend.
#[derive(Clone, Debug)]
pub struct ResolvedFigure {
    pub layout: DocumentLayout,
    pub display: ResolvedDisplayList,
}

pub fn resolve_document(document: &FigureDocument) -> Result<ResolvedFigure, DocumentLayoutError> {
    let layout = document.layout_figure()?;
    let display = resolve(&layout.result.display_list);
    Ok(ResolvedFigure { layout, display })
}

#[cfg(test)]
mod tests {
    use super::*;
    use export_backend_spike::ResolvedItem;
    use studio_render_spike::{DisplayItem, NodeId};

    #[test]
    fn one_resolution_contains_layout_identity_and_backend_items() {
        let resolved = resolve_document(&FigureDocument::fixed()).unwrap();
        assert_eq!(
            resolved.layout.project_ids.get(&NodeId(15)).unwrap(),
            "node-15"
        );
        assert_eq!(
            resolved.display.width,
            resolved.layout.result.display_list.width.get() as f32
        );
        assert!(resolved.display.items.iter().any(|item| matches!(
            item,
            ResolvedItem::Graphics(DisplayItem::Path { source, .. }) if *source == NodeId(11)
        )));
        assert!(resolved.display.items.iter().any(|item| matches!(
            item,
            ResolvedItem::Text(text) if text.source == NodeId(4) && text.rotation_degrees == -90.0
        )));
    }
}
