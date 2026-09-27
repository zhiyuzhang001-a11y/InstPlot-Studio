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
    let mut values = Vec::new();
    for artist in &project.figure.artists {
        if !artist.visible {
            continue;
        }
        match &artist.properties {
            ArtistProperties::Line { binding, .. } | ArtistProperties::Scatter { binding, .. } => {
                let column_name = match dimension {
                    AxisDimension::X => &binding.x_column,
                    AxisDimension::Y => &binding.y_column,
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
                    match dimension {
                        AxisDimension::X => {
                            values.push(point.x - error.x_minus);
                            values.push(point.x + error.x_plus);
                        }
                        AxisDimension::Y => {
                            values.push(point.y - error.y_minus);
                            values.push(point.y + error.y_plus);
                        }
                    }
                }
            }
            ArtistProperties::ReferenceLine {
                orientation, value, ..
            } if policy.include_reference_lines
                && matches!(
                    (dimension, orientation),
                    (AxisDimension::X, ReferenceOrientation::Vertical)
                        | (AxisDimension::Y, ReferenceOrientation::Horizontal)
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

pub(super) fn apply_autoscale(
    project: &mut ProjectDocument,
    dimension: AxisDimension,
) -> Result<(), String> {
    let axis = match dimension {
        AxisDimension::X => &project.figure.axes[0].x,
        AxisDimension::Y => &project.figure.axes[0].y,
    };
    if !axis.autoscale {
        return Ok(());
    }
    let policy = AutoscalePolicy::default();
    let visual = apply_visual_padding(
        compute_data_bounds(project, dimension, policy)?,
        axis.scale,
        policy,
    )?;
    let axis = match dimension {
        AxisDimension::X => &mut project.figure.axes[0].x,
        AxisDimension::Y => &mut project.figure.axes[0].y,
    };
    axis.minimum = visual.minimum;
    axis.maximum = visual.maximum;
    Ok(())
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
