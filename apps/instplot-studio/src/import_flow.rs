use super::*;

pub(super) fn affected_axes_for_sources(
    document: &FigureDocument,
    source_ids: &BTreeSet<String>,
) -> Vec<AxisIdentity> {
    let mut identities = Vec::new();
    for series in document.logical_series().into_iter().filter(|series| {
        series.effective_visible
            && series
                .binding
                .as_ref()
                .is_some_and(|binding| source_ids.contains(&binding.data_source_id))
    }) {
        let Some(binding) = series.axes else {
            continue;
        };
        let pair = [
            match binding.x {
                XAxisSlot::X1 => AxisIdentity::X1,
                XAxisSlot::X2 => AxisIdentity::X2,
            },
            match binding.y {
                YAxisSlot::Y1 => AxisIdentity::Y1,
                YAxisSlot::Y2 => AxisIdentity::Y2,
            },
        ];
        for identity in pair {
            if !identities.contains(&identity) {
                identities.push(identity);
            }
        }
    }
    identities
}

pub(super) fn preferred_dataset_columns(
    dataset: &DataSet,
    preferred: Option<(&str, &str)>,
) -> Option<(String, String)> {
    if dataset.columns.len() < 2 {
        return None;
    }
    let x = preferred
        .and_then(|(name, _)| {
            dataset
                .columns
                .iter()
                .position(|column| column.name == name)
        })
        .unwrap_or(0);
    let y = preferred
        .and_then(|(_, name)| {
            dataset
                .columns
                .iter()
                .position(|column| column.name == name)
        })
        .filter(|index| *index != x)
        .unwrap_or_else(|| {
            (0..dataset.columns.len())
                .find(|index| *index != x)
                .unwrap_or(1)
        });
    Some((
        dataset.columns[x].name.clone(),
        dataset.columns[y].name.clone(),
    ))
}

pub(super) fn document_with_imported_datasets(
    current: &FigureDocument,
    datasets: &[DataSet],
    imported_ids: &BTreeSet<String>,
    replace_showcase: bool,
    preferred: Option<(&str, &str)>,
) -> Result<FigureDocument, String> {
    if replace_showcase {
        let mut document =
            FigureDocument::from_datasets(datasets).map_err(|error| error.to_string())?;
        if USER_PALETTE_IDS.contains(&current.palette_id()) {
            document.set_palette(current.palette_id())?;
        }
        let affected = affected_axes_for_sources(&document, imported_ids);
        document.restore_autoscale_after_data_change(&affected)?;
        return Ok(document);
    }
    // Clearing every imported source leaves the figure/axes in place so its visual
    // formatting can be reused. The first dataset imported into that empty figure,
    // however, owns the axis semantics: stale labels from the removed data must not
    // survive. With any bound series still present we continue to preserve labels,
    // including labels customized by the user.
    let reset_axis_labels = !current.series().iter().any(|series| {
        series.visible
            && matches!(
                series.kind,
                SeriesKind::Line | SeriesKind::Scatter | SeriesKind::ErrorBar
            )
            && series.binding.is_some()
    });
    let mut affected_axes = affected_axes_for_sources(current, imported_ids);
    let mut document = current.clone();
    document
        .sync_datasets_without_autoscale(datasets)
        .map_err(|error| error.to_string())?;
    let mut replacement_axis_labels = None;
    for dataset in datasets
        .iter()
        .filter(|dataset| imported_ids.contains(&dataset.plot_id))
    {
        let already_plotted = document.series().iter().any(|series| {
            series
                .binding
                .as_ref()
                .is_some_and(|binding| binding.data_source_id == dataset.plot_id)
        });
        if already_plotted {
            continue;
        }
        let (x, y) = preferred_dataset_columns(dataset, preferred)
            .ok_or_else(|| format!("{} 需要至少两列数值数据", dataset.display_name()))?;
        let style = if dataset.kind == DataSetKind::Fit {
            SeriesCreationStyle::Line
        } else {
            SeriesCreationStyle::Scatter
        };
        document.create_series(&dataset.plot_id, &x, &y, style)?;
        if reset_axis_labels && replacement_axis_labels.is_none() {
            replacement_axis_labels = Some((x, y));
        }
    }
    if let Some((x, y)) = replacement_axis_labels {
        document.set_axis_label(AxisDimension::X, vec![LabelNode::Text(x)])?;
        document.set_axis_label(AxisDimension::Y, vec![LabelNode::Text(y)])?;
    }
    for identity in affected_axes_for_sources(&document, imported_ids) {
        if !affected_axes.contains(&identity) {
            affected_axes.push(identity);
        }
    }
    document.restore_autoscale_after_data_change(&affected_axes)?;
    Ok(document)
}
