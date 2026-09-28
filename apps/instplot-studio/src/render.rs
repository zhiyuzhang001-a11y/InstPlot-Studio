use instplot_export::{ResolvedBounds, ResolvedDisplayList, resolve, resolve_tight};

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

/// Resolve a separate, tightly cropped page for file export. The ordinary
/// preview and all document coordinates retain the existing canvas semantics.
pub fn resolve_document_for_export(
    document: &FigureDocument,
) -> Result<ResolvedFigure, DocumentLayoutError> {
    let layout = document.layout_figure()?;
    let axes = layout.result.axes;
    let display = resolve_tight(
        &layout.result.display_list,
        ResolvedBounds {
            min_x: axes.x as f32,
            min_y: axes.y as f32,
            max_x: axes.right() as f32,
            max_y: axes.bottom() as f32,
        },
        3.0,
    );
    Ok(ResolvedFigure { layout, display })
}

#[cfg(test)]
mod tests {
    use super::*;
    use instplot_export::ResolvedItem;
    use instplot_render::{DisplayItem, NodeId};

    #[test]
    fn one_resolution_contains_layout_identity_and_backend_items() {
        let resolved = resolve_document(&FigureDocument::fixed()).unwrap();
        assert_eq!(
            resolved.layout.project_ids.get(&NodeId(15)).unwrap(),
            "node-15"
        );
        assert!(resolved.display.width >= resolved.layout.result.axes.width as f32);
        assert!(resolved.display.geometry.ink_bounds.width() > 0.0);
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
