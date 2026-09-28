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
                    let endpoints = match identity {
                        AxisIdentity::X1 | AxisIdentity::X2 => {
                            checked_error_endpoints(point.x, error.x_minus, error.x_plus, "X")?
                        }
                        AxisIdentity::Y1 | AxisIdentity::Y2 => {
                            checked_error_endpoints(point.y, error.y_minus, error.y_plus, "Y")?
                        }
                    };
                    values.extend(endpoints);
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

fn checked_error_endpoints(
    value: f64,
    minus: f64,
    plus: f64,
    dimension: &str,
) -> Result<[f64; 2], String> {
    let lower = value - minus;
    let upper = value + plus;
    if !lower.is_finite() || !upper.is_finite() {
        return Err(format!(
            "{dimension} error-bar endpoint exceeds the finite numeric range"
        ));
    }
    Ok([lower, upper])
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
    if !bounds.minimum.is_finite() || !bounds.maximum.is_finite() || bounds.minimum > bounds.maximum
    {
        return Err("autoscale bounds must be finite and ordered".to_owned());
    }
    if scale == AxisScale::Log10 && bounds.minimum <= 0.0 {
        return Err("log autoscale requires every visible value to be positive".to_owned());
    }
    if scale == AxisScale::Log10 {
        if !policy.log_padding_factor.is_finite() || policy.log_padding_factor <= 1.0 {
            return Err(
                "log autoscale padding factor must be finite and greater than one".to_owned(),
            );
        }
        let padded_minimum = bounds.minimum / policy.log_padding_factor;
        let padded_maximum = bounds.maximum * policy.log_padding_factor;
        let mut minimum = if padded_minimum.is_finite() && padded_minimum > 0.0 {
            padded_minimum
        } else {
            bounds.minimum
        };
        let mut maximum = if padded_maximum.is_finite() {
            padded_maximum
        } else {
            bounds.maximum
        };
        if minimum >= bounds.minimum {
            let adjacent = bounds.minimum.next_down();
            if adjacent.is_finite() && adjacent > 0.0 {
                minimum = adjacent;
            }
        }
        if maximum <= bounds.maximum {
            let adjacent = bounds.maximum.next_up();
            if adjacent.is_finite() {
                maximum = adjacent;
            }
        }
        if minimum < maximum {
            return Ok(VisualBounds { minimum, maximum });
        }
        return Err("log autoscale cannot produce finite visual bounds".to_owned());
    }
    if !policy.linear_padding_fraction.is_finite() || policy.linear_padding_fraction < 0.0 {
        return Err("linear autoscale padding fraction must be finite and non-negative".to_owned());
    }
    if bounds.minimum == bounds.maximum {
        if bounds.minimum == 0.0 {
            return Ok(VisualBounds {
                minimum: -1.0,
                maximum: 1.0,
            });
        }
        let pad = bounds.minimum.abs() * policy.linear_padding_fraction;
        let padded_minimum = bounds.minimum - pad;
        let padded_maximum = bounds.maximum + pad;
        let minimum = if padded_minimum.is_finite() && padded_minimum < bounds.minimum {
            padded_minimum
        } else {
            bounds.minimum.next_down()
        };
        let maximum = if padded_maximum.is_finite() && padded_maximum > bounds.maximum {
            padded_maximum
        } else {
            bounds.maximum.next_up()
        };
        if minimum.is_finite() && maximum.is_finite() && minimum < maximum {
            return Ok(VisualBounds { minimum, maximum });
        }
        // At either finite endpoint only one side can expand. Keeping the
        // endpoint itself is valid as long as the other side creates a range.
        let minimum = if bounds.minimum > -f64::MAX {
            bounds.minimum.next_down()
        } else {
            bounds.minimum
        };
        let maximum = if bounds.maximum < f64::MAX {
            bounds.maximum.next_up()
        } else {
            bounds.maximum
        };
        if minimum.is_finite() && maximum.is_finite() && minimum < maximum {
            return Ok(VisualBounds { minimum, maximum });
        }
        return Err("constant value cannot be expanded within the finite numeric range".to_owned());
    }
    let span = bounds.maximum - bounds.minimum;
    let (candidate_minimum, candidate_maximum) = if span.is_finite() {
        let pad = span * policy.linear_padding_fraction;
        (bounds.minimum - pad, bounds.maximum + pad)
    } else {
        let scale = bounds.minimum.abs().max(bounds.maximum.abs());
        let scaled_minimum = bounds.minimum / scale;
        let scaled_maximum = bounds.maximum / scale;
        let scaled_pad = (scaled_maximum - scaled_minimum) * policy.linear_padding_fraction;
        (
            (scaled_minimum - scaled_pad) * scale,
            (scaled_maximum + scaled_pad) * scale,
        )
    };
    let mut minimum = if candidate_minimum.is_finite() {
        candidate_minimum
    } else {
        bounds.minimum
    };
    let mut maximum = if candidate_maximum.is_finite() {
        candidate_maximum
    } else {
        bounds.maximum
    };
    if policy.linear_padding_fraction > 0.0 && minimum >= bounds.minimum {
        let adjacent = bounds.minimum.next_down();
        if adjacent.is_finite() {
            minimum = adjacent;
        }
    }
    if policy.linear_padding_fraction > 0.0 && maximum <= bounds.maximum {
        let adjacent = bounds.maximum.next_up();
        if adjacent.is_finite() {
            maximum = adjacent;
        }
    }
    if minimum < maximum {
        Ok(VisualBounds { minimum, maximum })
    } else {
        Err("linear autoscale cannot produce finite visual bounds".to_owned())
    }
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

/// Restores the data-dependent state for axes whose visible data context changed.
///
/// This is deliberately separate from `refresh_active_autoscales`: ordinary visual
/// edits must continue to respect a user-disabled autoscale, while importing,
/// replacing, rebinding, moving, showing, hiding, or deleting data invalidates the
/// previous range and tick choices for the affected axes.
pub(super) fn restore_autoscale_for_axes(
    project: &mut ProjectDocument,
    identities: impl IntoIterator<Item = AxisIdentity>,
) -> Result<(), String> {
    let mut unique = Vec::new();
    for identity in identities {
        if !unique.contains(&identity) {
            unique.push(identity);
        }
    }
    for identity in unique {
        let Some(axis) = axis_record_mut(project, identity) else {
            continue;
        };
        axis.autoscale = true;
        axis.locator = LocatorSpec::Auto { target_count: 6 };
        axis.minor_interval = None;
        axis.formatter = FormatterSpec::Auto;
        apply_autoscale_for_axis(project, identity, true)?;
    }
    Ok(())
}

pub(super) const fn axis_identities_for_binding(binding: AxisBinding) -> [AxisIdentity; 2] {
    let x = match binding.x {
        XAxisSlot::X1 => AxisIdentity::X1,
        XAxisSlot::X2 => AxisIdentity::X2,
    };
    let y = match binding.y {
        YAxisSlot::Y1 => AxisIdentity::Y1,
        YAxisSlot::Y2 => AxisIdentity::Y2,
    };
    [x, y]
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

    #[test]
    fn constant_tiny_values_receive_local_not_unit_scale_padding() {
        let visual = apply_visual_padding(
            DataBounds {
                minimum: 1.0e-12,
                maximum: 1.0e-12,
            },
            AxisScale::Linear,
            AutoscalePolicy::default(),
        )
        .unwrap();
        assert!(visual.minimum < 1.0e-12);
        assert!(visual.maximum > 1.0e-12);
        assert!(visual.maximum - visual.minimum < 1.0e-12, "{visual:?}");
    }

    #[test]
    fn extreme_finite_linear_bounds_remain_finite() {
        let visual = apply_visual_padding(
            DataBounds {
                minimum: -f64::MAX,
                maximum: f64::MAX,
            },
            AxisScale::Linear,
            AutoscalePolicy::default(),
        )
        .unwrap();
        assert!(visual.minimum.is_finite());
        assert!(visual.maximum.is_finite());
        assert!(visual.minimum < visual.maximum);
    }

    #[test]
    fn minimum_subnormal_log_bound_does_not_pad_to_zero() {
        let visual = apply_visual_padding(
            DataBounds {
                minimum: f64::from_bits(1),
                maximum: f64::from_bits(4),
            },
            AxisScale::Log10,
            AutoscalePolicy::default(),
        )
        .unwrap();
        assert!(visual.minimum.is_finite() && visual.minimum > 0.0);
        assert!(visual.maximum.is_finite() && visual.maximum >= f64::from_bits(4));
    }

    #[test]
    fn constant_finite_endpoints_expand_on_the_available_side() {
        for value in [-f64::MAX, f64::MAX] {
            let visual = apply_visual_padding(
                DataBounds {
                    minimum: value,
                    maximum: value,
                },
                AxisScale::Linear,
                AutoscalePolicy::default(),
            )
            .unwrap();
            assert!(visual.minimum.is_finite() && visual.maximum.is_finite());
            assert!(visual.minimum < visual.maximum, "{value}: {visual:?}");
            assert!(visual.minimum <= value && visual.maximum >= value);
        }
    }

    #[test]
    fn constant_minimum_subnormal_log_value_expands_upward() {
        let value = f64::from_bits(1);
        let visual = apply_visual_padding(
            DataBounds {
                minimum: value,
                maximum: value,
            },
            AxisScale::Log10,
            AutoscalePolicy::default(),
        )
        .unwrap();
        assert!(visual.minimum > 0.0 && visual.minimum.is_finite());
        assert!(visual.maximum.is_finite() && visual.maximum > value);
        assert!(visual.minimum <= value);
    }

    #[test]
    fn error_endpoint_overflow_is_explicit_instead_of_silently_dropped() {
        let error = checked_error_endpoints(f64::MAX, 0.0, f64::MAX, "Y").unwrap_err();
        assert!(error.contains("finite numeric range"));
    }
}
