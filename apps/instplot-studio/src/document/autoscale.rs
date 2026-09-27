use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataBounds {
    pub minimum: f64,
    pub maximum: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualBounds {
    pub minimum: f64,
    pub maximum: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoscalePolicy {
    pub include_error_bars: bool,
    pub include_reference_lines: bool,
    pub include_data_annotations: bool,
    pub linear_padding_fraction: f64,
    pub log_padding_factor: f64,
    /// Marker size remains a visual-space concern. It is never added to data coordinates.
    pub marker_padding_pt: f64,
}

impl Default for AutoscalePolicy {
    fn default() -> Self {
        Self {
            include_error_bars: true,
            include_reference_lines: true,
            include_data_annotations: false,
            linear_padding_fraction: 0.05,
            log_padding_factor: 1.1,
            marker_padding_pt: 0.0,
        }
    }
}

pub fn compute_data_bounds(
    project: &ProjectDocument,
    dimension: AxisDimension,
    policy: AutoscalePolicy,
) -> Result<DataBounds, String> {
    let identity = match dimension {
        AxisDimension::X => AxisIdentity::X1,
        AxisDimension::Y => AxisIdentity::Y1,
    };
    compute_axis_data_bounds(project, identity, policy)
}

pub fn compute_axis_data_bounds(
    project: &ProjectDocument,
    identity: AxisIdentity,
    policy: AutoscalePolicy,
) -> Result<DataBounds, String> {
    let mut values = Vec::new();
    for artist in &project.figure.artists {
        if !project.artist_effectively_visible(artist) {
            continue;
        }
        let Some(axes) = project.artist_axis_binding(artist) else {
            continue;
        };
        if !binding_matches_identity(axes, identity) {
            continue;
        }
        match &artist.properties {
            ArtistProperties::Line { binding, .. } | ArtistProperties::Scatter { binding, .. } => {
                let column_name = match identity {
                    AxisIdentity::X1 | AxisIdentity::X2 => &binding.x_column,
                    AxisIdentity::Y1 | AxisIdentity::Y2 => &binding.y_column,
                };
                let Some(source) = project
                    .data_sources
                    .iter()
                    .find(|source| source.id == binding.data_source_id)
                else {
                    continue;
                };
                let DataSourcePayload::Embedded { columns, alive, .. } = &source.payload else {
                    continue;
                };
                if let Some(column) = columns.iter().find(|column| column.name == *column_name) {
                    values.extend(
                        column
                            .values
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| alive.get(*index).copied().unwrap_or(true))
                            .map(|(_, value)| *value),
                    );
                }
            }
            ArtistProperties::ErrorBar {
                binding,
                x_error_column,
                y_error_column,
                ..
            } if policy.include_error_bars => {
                let (points, errors) =
                    bound_error_data(binding, x_error_column.as_deref(), y_error_column, project)
                        .map_err(|error| error.to_string())?;
                for (point, error) in points.into_iter().zip(errors) {
                    match identity {
                        AxisIdentity::X1 | AxisIdentity::X2 => {
                            values.push(point.x - error.x_minus);
                            values.push(point.x + error.x_plus);
                        }
                        AxisIdentity::Y1 | AxisIdentity::Y2 => {
                            values.push(point.y - error.y_minus);
                            values.push(point.y + error.y_plus);
                        }
                    }
                }
            }
            ArtistProperties::ReferenceLine {
                orientation,
                value,
                include_in_autoscale: true,
                ..
            } if policy.include_reference_lines
                && matches!(
                    (identity, orientation),
                    (
                        AxisIdentity::X1 | AxisIdentity::X2,
                        ReferenceOrientation::Vertical
                    ) | (
                        AxisIdentity::Y1 | AxisIdentity::Y2,
                        ReferenceOrientation::Horizontal
                    )
                ) =>
            {
                values.push(*value)
            }
            _ => {}
        }
    }
    values.retain(|value| value.is_finite());
    if values.is_empty() {
        return Err("autoscale requires at least one visible bound value".to_owned());
    }
    Ok(DataBounds {
        minimum: values.iter().copied().fold(f64::INFINITY, f64::min),
        maximum: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    })
}

fn binding_matches_identity(binding: AxisBinding, identity: AxisIdentity) -> bool {
    match identity {
        AxisIdentity::X1 => binding.x == XAxisSlot::X1,
        AxisIdentity::X2 => binding.x == XAxisSlot::X2,
        AxisIdentity::Y1 => binding.y == YAxisSlot::Y1,
        AxisIdentity::Y2 => binding.y == YAxisSlot::Y2,
    }
}

pub fn apply_visual_padding(
    bounds: DataBounds,
    scale: AxisScale,
    policy: AutoscalePolicy,
) -> Result<VisualBounds, String> {
    if scale == AxisScale::Log10 && bounds.minimum <= 0.0 {
        return Err("log autoscale requires every visible value to be positive".to_owned());
    }
    if scale == AxisScale::Log10 {
        return Ok(VisualBounds {
            minimum: bounds.minimum / policy.log_padding_factor,
            maximum: bounds.maximum * policy.log_padding_factor,
        });
    }
    let pad = if bounds.minimum == bounds.maximum {
        (bounds.minimum.abs() * policy.linear_padding_fraction).max(0.5)
    } else {
        (bounds.maximum - bounds.minimum) * policy.linear_padding_fraction
    };
    Ok(VisualBounds {
        minimum: bounds.minimum - pad,
        maximum: bounds.maximum + pad,
    })
}

pub(super) fn refresh_active_autoscales(project: &mut ProjectDocument) -> Result<(), String> {
    let identities: &[AxisIdentity] = match project.figure.axes[0].mode {
        AxisMode::Single => &[AxisIdentity::X1, AxisIdentity::Y1],
        AxisMode::DualX => &[AxisIdentity::X1, AxisIdentity::X2, AxisIdentity::Y1],
        AxisMode::DualY => &[AxisIdentity::X1, AxisIdentity::Y1, AxisIdentity::Y2],
    };
    for identity in identities {
        apply_autoscale_for_axis(project, *identity, true)?;
    }
    Ok(())
}

pub(super) fn apply_autoscale_for_axis(
    project: &mut ProjectDocument,
    identity: AxisIdentity,
    allow_empty: bool,
) -> Result<(), String> {
    let axis = axis_record(project, identity)
        .ok_or_else(|| format!("axis {identity:?} is not configured"))?;
    if !axis.autoscale {
        return Ok(());
    }
    let policy = AutoscalePolicy::default();
    let bounds = match compute_axis_data_bounds(project, identity, policy) {
        Ok(bounds) => bounds,
        Err(error) if allow_empty && error.contains("at least one visible bound value") => {
            let axis = axis_record_mut(project, identity)
                .expect("the immutable axis lookup already confirmed this identity");
            (axis.minimum, axis.maximum) = match axis.scale {
                AxisScale::Linear => (0.0, 1.0),
                AxisScale::Log10 => (1.0, 10.0),
            };
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let visual = apply_visual_padding(bounds, axis.scale, policy)?;
    let axis = axis_record_mut(project, identity)
        .expect("the immutable axis lookup already confirmed this identity");
    axis.minimum = visual.minimum;
    axis.maximum = visual.maximum;
    Ok(())
}

fn axis_record(project: &ProjectDocument, identity: AxisIdentity) -> Option<&AxisRecord> {
    let axes = &project.figure.axes[0];
    match identity {
        AxisIdentity::X1 => Some(&axes.x),
        AxisIdentity::X2 => axes.x2.as_ref(),
        AxisIdentity::Y1 => Some(&axes.y),
        AxisIdentity::Y2 => axes.y2.as_ref(),
    }
}

fn axis_record_mut(
    project: &mut ProjectDocument,
    identity: AxisIdentity,
) -> Option<&mut AxisRecord> {
    let axes = &mut project.figure.axes[0];
    match identity {
        AxisIdentity::X1 => Some(&mut axes.x),
        AxisIdentity::X2 => axes.x2.as_mut(),
        AxisIdentity::Y1 => Some(&mut axes.y),
        AxisIdentity::Y2 => axes.y2.as_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_padding_is_separate_from_data_bounds() {
        let data = DataBounds {
            minimum: 1.0,
            maximum: 3.0,
        };
        let policy = AutoscalePolicy {
            marker_padding_pt: 12.0,
            ..AutoscalePolicy::default()
        };
        let visual = apply_visual_padding(data, AxisScale::Linear, policy).unwrap();
        assert_eq!(
            data,
            DataBounds {
                minimum: 1.0,
                maximum: 3.0
            }
        );
        assert_eq!(
            visual,
            VisualBounds {
                minimum: 0.9,
                maximum: 3.1
            }
        );
    }

    #[test]
    fn log_padding_rejects_non_positive_data_before_visual_expansion() {
        let error = apply_visual_padding(
            DataBounds {
                minimum: 0.0,
                maximum: 10.0,
            },
            AxisScale::Log10,
            AutoscalePolicy::default(),
        )
        .unwrap_err();
        assert!(error.contains("positive"));
    }
}
