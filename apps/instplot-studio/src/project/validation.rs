use super::*;

pub(super) fn validate_axis(
    axis: &AxisRecord,
    labels: &BTreeSet<String>,
) -> Result<(), ProjectError> {
    if !axis.minimum.is_finite() || !axis.maximum.is_finite() || axis.minimum >= axis.maximum {
        return Err(ProjectError::Validation(format!(
            "axis {} requires finite minimum < maximum",
            axis.id
        )));
    }
    if !labels.contains(&axis.label_id) {
        return Err(ProjectError::Validation(format!(
            "axis {} references unknown label {}",
            axis.id, axis.label_id
        )));
    }
    if matches!(axis.scale, AxisScale::Log10) && axis.minimum <= 0.0 {
        return Err(ProjectError::Validation(format!(
            "log axis {} requires a positive minimum",
            axis.id
        )));
    }
    match &axis.locator {
        LocatorSpec::Auto { target_count } if !(2..=20).contains(target_count) => {
            return Err(ProjectError::Validation(format!(
                "axis {} auto locator target must be within 2..=20",
                axis.id
            )));
        }
        LocatorSpec::Interval { step }
            if axis.scale != AxisScale::Linear
                || !step.is_finite()
                || *step <= 0.0
                || (axis.maximum - axis.minimum) / step > 100.0
                || (axis.minimum / step).abs() > i64::MAX as f64 / 4.0
                || (axis.maximum / step).abs() > i64::MAX as f64 / 4.0 =>
        {
            return Err(ProjectError::Validation(format!(
                "axis {} interval locator requires a positive finite linear step with at most 100 ticks",
                axis.id
            )));
        }
        LocatorSpec::Fixed { values }
            if values.is_empty()
                || values.iter().any(|value| !value.is_finite())
                || values.windows(2).any(|pair| pair[0] >= pair[1])
                || values
                    .iter()
                    .any(|value| *value < axis.minimum || *value > axis.maximum) =>
        {
            return Err(ProjectError::Validation(format!(
                "axis {} fixed locator requires finite, strictly increasing values within its range",
                axis.id
            )));
        }
        _ => {}
    }
    if let Some(step) = axis.minor_interval
        && (axis.scale != AxisScale::Linear
            || !step.is_finite()
            || step <= 0.0
            || (axis.maximum - axis.minimum) / step > 500.0
            || (axis.minimum / step).abs() > i64::MAX as f64 / 4.0
            || (axis.maximum / step).abs() > i64::MAX as f64 / 4.0)
    {
        return Err(ProjectError::Validation(format!(
            "axis {} minor interval requires a positive finite linear step with at most 500 ticks",
            axis.id
        )));
    }
    match axis.formatter {
        FormatterSpec::Decimal { precision } | FormatterSpec::Scientific { precision }
            if precision > 15 =>
        {
            return Err(ProjectError::Validation(format!(
                "axis {} formatter precision exceeds 15",
                axis.id
            )));
        }
        _ => {}
    }
    if [
        axis.appearance.tick_label_pad_pt,
        axis.appearance.label_edge_pad_pt,
        axis.appearance.label_tick_pad_pt,
    ]
    .into_iter()
    .any(|value| !value.is_finite() || value < 0.0 || value > 72.0)
    {
        return Err(ProjectError::Validation(format!(
            "axis {} has invalid label spacing",
            axis.id
        )));
    }
    Ok(())
}

pub(super) fn validate_artist(
    artist: &ArtistRecord,
    sources: &BTreeSet<String>,
    labels: &BTreeSet<String>,
    artists: &BTreeSet<String>,
    palette_colors: &BTreeSet<String>,
) -> Result<(), ProjectError> {
    let expected_kind = match &artist.properties {
        ArtistProperties::Line { binding, stroke } => {
            validate_binding(artist, binding, sources)?;
            validate_stroke(artist, stroke, palette_colors)?;
            ArtistKind::Line
        }
        ArtistProperties::Scatter { binding, marker } => {
            validate_binding(artist, binding, sources)?;
            if !marker.size_pt.is_finite()
                || marker.size_pt <= 0.0
                || marker.interval == 0
                || !palette_colors.contains(&marker.color_id)
            {
                return Err(ProjectError::Validation(format!(
                    "artist {} has an invalid marker style",
                    artist.id
                )));
            }
            ArtistKind::Scatter
        }
        ArtistProperties::ErrorBar {
            binding,
            x_error_column,
            y_error_column,
            cap_width_pt,
            stroke,
        } => {
            validate_binding(artist, binding, sources)?;
            validate_stroke(artist, stroke, palette_colors)?;
            if y_error_column.trim().is_empty()
                || x_error_column
                    .as_ref()
                    .is_some_and(|name| name.trim().is_empty())
                || !cap_width_pt.is_finite()
                || *cap_width_pt <= 0.0
            {
                return Err(ProjectError::Validation(format!(
                    "artist {} has an invalid error-bar binding",
                    artist.id
                )));
            }
            ArtistKind::ErrorBar
        }
        ArtistProperties::ReferenceLine {
            value,
            axes,
            stroke,
            ..
        } => {
            validate_stroke(artist, stroke, palette_colors)?;
            if !value.is_finite() || !axes.is_supported() {
                return Err(ProjectError::Validation(format!(
                    "artist {} has a non-finite reference value",
                    artist.id
                )));
            }
            ArtistKind::ReferenceLine
        }
        ArtistProperties::Annotation {
            label_id,
            x_pt,
            y_pt,
            connectors,
        } => {
            if !labels.contains(label_id)
                || !x_pt.is_finite()
                || !y_pt.is_finite()
                || connectors.iter().any(|connector| {
                    !connector.target_x.is_finite()
                        || !connector.target_y.is_finite()
                        || !connector.axes.is_supported()
                        || !connector.arrow_size_pt.is_finite()
                        || !(2.0..=18.0).contains(&connector.arrow_size_pt)
                        || validate_stroke(artist, &connector.stroke, palette_colors).is_err()
                })
            {
                return Err(ProjectError::Validation(format!(
                    "artist {} has an invalid annotation",
                    artist.id
                )));
            }
            ArtistKind::Annotation
        }
        ArtistProperties::Legend {
            entries,
            x_pt,
            y_pt,
            grid,
            ..
        } => {
            if entries.is_empty()
                || !x_pt.is_finite()
                || !y_pt.is_finite()
                || matches!(grid, LegendGrid::Rows(0) | LegendGrid::Columns(0))
                || entries.iter().any(|entry| {
                    !artists.contains(&entry.artist_id) || !labels.contains(&entry.label_id)
                })
            {
                return Err(ProjectError::Validation(format!(
                    "artist {} has invalid legend entries or placement",
                    artist.id
                )));
            }
            ArtistKind::Legend
        }
    };
    if artist.kind != expected_kind {
        return Err(ProjectError::Validation(format!(
            "artist {} kind does not match its properties",
            artist.id
        )));
    }
    Ok(())
}

pub(super) fn validate_binding(
    artist: &ArtistRecord,
    binding: &DataBinding,
    sources: &BTreeSet<String>,
) -> Result<(), ProjectError> {
    if !sources.contains(&binding.data_source_id)
        || binding.x_column.trim().is_empty()
        || binding.y_column.trim().is_empty()
    {
        return Err(ProjectError::Validation(format!(
            "artist {} has an invalid data binding",
            artist.id
        )));
    }
    Ok(())
}

pub(super) fn validate_stroke(
    artist: &ArtistRecord,
    stroke: &StrokeStyle,
    palette_colors: &BTreeSet<String>,
) -> Result<(), ProjectError> {
    if !palette_colors.contains(&stroke.color_id)
        || !stroke.width_pt.is_finite()
        || stroke.width_pt <= 0.0
        || stroke
            .dash_pt
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(ProjectError::Validation(format!(
            "artist {} has an invalid stroke style",
            artist.id
        )));
    }
    Ok(())
}

pub(super) fn validate_payload(source: &DataSourceRecord) -> Result<(), ProjectError> {
    match &source.payload {
        DataSourcePayload::External { path, fingerprint } => {
            if path.is_empty() || fingerprint.sha256.len() != 64 {
                return Err(ProjectError::Validation(format!(
                    "external data source {} has invalid path or fingerprint",
                    source.id
                )));
            }
        }
        DataSourcePayload::Embedded {
            columns,
            row_count,
            alive,
            sha256,
        } => {
            if sha256.len() != 64
                || (!alive.is_empty() && alive.len() != *row_count)
                || columns
                    .iter()
                    .any(|column| column.values.len() != *row_count)
                || columns
                    .iter()
                    .any(|column| !column.valid.is_empty() && column.valid.len() != *row_count)
                || columns
                    .iter()
                    .flat_map(|column| &column.values)
                    .any(|value| !value.is_finite())
                || sha256 != &embedded_digest(columns, *row_count, alive)?
            {
                return Err(ProjectError::Validation(format!(
                    "embedded data source {} has inconsistent data",
                    source.id
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_source_origin(source: &DataSourceRecord) -> Result<(), ProjectError> {
    if source.kind == DataSourceKind::Fit && source.origin != DataSourceOrigin::Imported {
        return Err(ProjectError::Validation(format!(
            "fit data source {} cannot be manual",
            source.id
        )));
    }
    if source.origin == DataSourceOrigin::Imported
        && (source.manual_recipe.is_some() || source.managed_file.is_some())
    {
        return Err(ProjectError::Validation(format!(
            "imported data source {} contains manual-data metadata",
            source.id
        )));
    }
    if source.origin == DataSourceOrigin::LegacyManual && source.manual_recipe.is_some() {
        return Err(ProjectError::Validation(format!(
            "legacy manual data source {} cannot claim a recoverable recipe",
            source.id
        )));
    }
    if let Some(recipe) = &source.manual_recipe {
        if source.origin != DataSourceOrigin::Manual
            || recipe.group_id.trim().is_empty()
            || recipe.name.trim().is_empty()
            || recipe.x.name.trim().is_empty()
            || recipe.y.name.trim().is_empty()
            || recipe.x.measurements.is_empty()
            || recipe.y.measurements.is_empty()
        {
            return Err(ProjectError::Validation(format!(
                "manual data source {} has an invalid recipe",
                source.id
            )));
        }
        for (axis_name, axis) in [("X", &recipe.x), ("Y", &recipe.y)] {
            let expected = axis.measurements[0].len();
            if expected == 0
                || axis.measurements.iter().any(|measurement| {
                    measurement.len() != expected
                        || measurement.iter().any(|value| !value.is_finite())
                })
            {
                return Err(ProjectError::Validation(format!(
                    "manual data source {} has inconsistent {axis_name} measurements",
                    source.id
                )));
            }
        }
        if recipe.x.measurements[0].len() != recipe.y.measurements[0].len() {
            return Err(ProjectError::Validation(format!(
                "manual data source {} has mismatched X/Y lengths",
                source.id
            )));
        }
    }
    if let Some(managed) = &source.managed_file {
        if source.origin != DataSourceOrigin::Manual || managed.path.trim().is_empty() {
            return Err(ProjectError::Validation(format!(
                "data source {} has invalid managed-file metadata",
                source.id
            )));
        }
        if managed
            .fingerprint
            .as_ref()
            .is_some_and(|fingerprint| fingerprint.sha256.len() != 64)
        {
            return Err(ProjectError::Validation(format!(
                "data source {} has an invalid managed-file fingerprint",
                source.id
            )));
        }
    }
    Ok(())
}

pub(super) fn embedded_digest(
    columns: &[EmbeddedColumn],
    row_count: usize,
    alive: &[bool],
) -> Result<String, ProjectError> {
    let bytes = if alive.is_empty() {
        serde_json::to_vec(&(columns, row_count))?
    } else {
        serde_json::to_vec(&(columns, row_count, alive))?
    };
    Ok(hex_digest(&bytes))
}

pub(super) fn validate_finite_positive(name: &str, value: f64) -> Result<(), ProjectError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(ProjectError::Validation(format!(
            "{name} must be finite and positive"
        )))
    }
}

pub(super) fn validate_scale(name: &str, value: f64) -> Result<(), ProjectError> {
    if value.is_finite() && (0.1..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ProjectError::Validation(format!(
            "{name} must be within 0.1..=1.0"
        )))
    }
}

pub(super) fn collect_ids<'a>(
    values: impl Iterator<Item = &'a str>,
    kind: &str,
) -> Result<BTreeSet<String>, ProjectError> {
    let mut ids = BTreeSet::new();
    for id in values {
        if id.trim().is_empty() || !ids.insert(id.to_owned()) {
            return Err(ProjectError::Validation(format!(
                "{kind} ID is empty or duplicated: {id:?}"
            )));
        }
    }
    Ok(ids)
}

pub(super) fn insert_id(ids: &mut BTreeSet<String>, id: &str) -> Result<(), ProjectError> {
    if id.trim().is_empty() || !ids.insert(id.to_owned()) {
        return Err(ProjectError::Validation(format!(
            "figure node ID is empty or duplicated: {id:?}"
        )));
    }
    Ok(())
}

pub(super) fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
