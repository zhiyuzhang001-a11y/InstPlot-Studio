use super::*;

impl FigureDocument {
    pub fn axis_mode(&self) -> AxisMode {
        self.project.figure.axes[0].mode
    }

    pub fn empty_active_secondary_axis(&self) -> Option<AxisIdentity> {
        let identity = match self.project.figure.axes[0].mode {
            AxisMode::Single => return None,
            AxisMode::DualX => AxisIdentity::X2,
            AxisMode::DualY => AxisIdentity::Y2,
        };
        if self
            .axis_record_by_identity(identity)
            .is_some_and(|axis| !axis.appearance.visibility.label)
        {
            return None;
        }
        let has_visible_series = self.project.figure.axes[0]
            .series_groups
            .iter()
            .filter(|group| match identity {
                AxisIdentity::X2 => group.axes.x == XAxisSlot::X2,
                AxisIdentity::Y2 => group.axes.y == YAxisSlot::Y2,
                AxisIdentity::X1 | AxisIdentity::Y1 => false,
            })
            .any(|group| {
                group.artist_ids.iter().any(|artist_id| {
                    self.project
                        .figure
                        .artists
                        .iter()
                        .find(|artist| artist.id == *artist_id)
                        .is_some_and(|artist| {
                            matches!(
                                artist.properties,
                                ArtistProperties::Line { .. }
                                    | ArtistProperties::Scatter { .. }
                                    | ArtistProperties::ErrorBar { .. }
                            ) && self.project.artist_effectively_visible(artist)
                        })
                })
            });
        (!has_visible_series).then_some(identity)
    }

    pub fn axis_record_by_identity(&self, identity: AxisIdentity) -> Option<crate::AxisRecord> {
        let axes = &self.project.figure.axes[0];
        match identity {
            AxisIdentity::X1 => Some(axes.x.clone()),
            AxisIdentity::X2 => axes.x2.clone(),
            AxisIdentity::Y1 => Some(axes.y.clone()),
            AxisIdentity::Y2 => axes.y2.clone(),
        }
    }

    pub fn axis_identity_for_project_id(&self, project_id: &str) -> Option<AxisIdentity> {
        let axes = &self.project.figure.axes[0];
        if axes.x.id == project_id || axes.x.label_id == project_id {
            Some(AxisIdentity::X1)
        } else if axes
            .x2
            .as_ref()
            .is_some_and(|axis| axis.id == project_id || axis.label_id == project_id)
        {
            Some(AxisIdentity::X2)
        } else if axes.y.id == project_id || axes.y.label_id == project_id {
            Some(AxisIdentity::Y1)
        } else if axes
            .y2
            .as_ref()
            .is_some_and(|axis| axis.id == project_id || axis.label_id == project_id)
        {
            Some(AxisIdentity::Y2)
        } else {
            None
        }
    }

    pub fn reference_value_is_visible(
        &self,
        orientation: ReferenceOrientation,
        binding: AxisBinding,
        value: f64,
    ) -> bool {
        let identity = match orientation {
            ReferenceOrientation::Vertical => match binding.x {
                XAxisSlot::X1 => AxisIdentity::X1,
                XAxisSlot::X2 => AxisIdentity::X2,
            },
            ReferenceOrientation::Horizontal => match binding.y {
                YAxisSlot::Y1 => AxisIdentity::Y1,
                YAxisSlot::Y2 => AxisIdentity::Y2,
            },
        };
        self.axis_record_by_identity(identity)
            .is_some_and(|axis| axis_value_is_visible(&axis, value))
    }

    pub fn measurement_points_are_visible(
        &self,
        binding: AxisBinding,
        start: (f64, f64),
        end: (f64, f64),
    ) -> bool {
        let x_identity = match binding.x {
            XAxisSlot::X1 => AxisIdentity::X1,
            XAxisSlot::X2 => AxisIdentity::X2,
        };
        let y_identity = match binding.y {
            YAxisSlot::Y1 => AxisIdentity::Y1,
            YAxisSlot::Y2 => AxisIdentity::Y2,
        };
        self.axis_record_by_identity(x_identity)
            .zip(self.axis_record_by_identity(y_identity))
            .is_some_and(|(x_axis, y_axis)| {
                [start, end].into_iter().all(|(x, y)| {
                    axis_value_is_visible(&x_axis, x) && axis_value_is_visible(&y_axis, y)
                })
            })
    }

    pub fn axis_visibility(&self, identity: AxisIdentity) -> Option<crate::AxisVisibilityRecord> {
        self.axis_record_by_identity(identity)
            .map(|axis| axis.appearance.visibility)
    }

    pub fn set_axis_visibility(
        &mut self,
        identity: AxisIdentity,
        visibility: crate::AxisVisibilityRecord,
    ) -> Result<(), String> {
        let mut record = self
            .axis_record_by_identity(identity)
            .ok_or_else(|| format!("axis {identity:?} is not available in the current project"))?;
        record.appearance.visibility = visibility;
        self.set_axis_record_by_identity(identity, record)
    }

    pub fn set_axis_record_by_identity(
        &mut self,
        identity: AxisIdentity,
        record: crate::AxisRecord,
    ) -> Result<(), String> {
        let current = self
            .axis_record_by_identity(identity)
            .ok_or_else(|| format!("axis {identity:?} is not available in the current project"))?;
        if record.id != current.id || record.label_id != current.label_id {
            return Err("axis identity and label identity cannot be replaced".to_owned());
        }
        let mut candidate = self.project.clone();
        let axes = &mut candidate.figure.axes[0];
        match identity {
            AxisIdentity::X1 => axes.x = record,
            AxisIdentity::X2 => axes.x2 = Some(record),
            AxisIdentity::Y1 => axes.y = record,
            AxisIdentity::Y2 => axes.y2 = Some(record),
        }
        apply_autoscale_for_axis(&mut candidate, identity, false)?;
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn set_axis_mode(&mut self, mode: AxisMode) -> Result<(), String> {
        let mut candidate = self.project.clone();
        ensure_secondary_axis(&mut candidate, mode);
        candidate.figure.axes[0].mode = mode;
        refresh_active_autoscales(&mut candidate)?;
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn axis_ranges(&self) -> AxisRanges {
        let axes = &self.project.figure.axes[0];
        AxisRanges {
            x_min: axes.x.minimum,
            x_max: axes.x.maximum,
            y_min: axes.y.minimum,
            y_max: axes.y.maximum,
        }
    }

    pub fn set_axis_ranges(&mut self, ranges: AxisRanges) -> Result<(), &'static str> {
        if !ranges.x_min.is_finite()
            || !ranges.x_max.is_finite()
            || !ranges.y_min.is_finite()
            || !ranges.y_max.is_finite()
        {
            return Err("axis ranges must be finite");
        }
        if ranges.x_min >= ranges.x_max || ranges.y_min >= ranges.y_max {
            return Err("each axis minimum must be smaller than its maximum");
        }
        let axes = &mut self.project.figure.axes[0];
        axes.x.minimum = ranges.x_min;
        axes.x.maximum = ranges.x_max;
        axes.x.autoscale = false;
        axes.y.minimum = ranges.y_min;
        axes.y.maximum = ranges.y_max;
        axes.y.autoscale = false;
        Ok(())
    }

    pub fn axis_record(&self, dimension: AxisDimension) -> crate::AxisRecord {
        let axes = &self.project.figure.axes[0];
        match dimension {
            AxisDimension::X => axes.x.clone(),
            AxisDimension::Y => axes.y.clone(),
        }
    }

    pub fn set_axis_record(
        &mut self,
        dimension: AxisDimension,
        record: crate::AxisRecord,
    ) -> Result<(), String> {
        let identity = match dimension {
            AxisDimension::X => AxisIdentity::X1,
            AxisDimension::Y => AxisIdentity::Y1,
        };
        self.set_axis_record_by_identity(identity, record)
    }

    pub fn set_axis_label(
        &mut self,
        dimension: AxisDimension,
        nodes: Vec<LabelNode>,
    ) -> Result<(), String> {
        if nodes.is_empty() {
            return Err("axis label cannot be empty".to_owned());
        }
        let identity = match dimension {
            AxisDimension::X => AxisIdentity::X1,
            AxisDimension::Y => AxisIdentity::Y1,
        };
        self.set_axis_label_by_identity(identity, nodes)
    }

    pub fn set_axis_label_by_identity(
        &mut self,
        identity: AxisIdentity,
        nodes: Vec<LabelNode>,
    ) -> Result<(), String> {
        if nodes.is_empty() {
            return Err("axis label cannot be empty".to_owned());
        }
        let label_id = self
            .axis_record_by_identity(identity)
            .ok_or_else(|| format!("axis {identity:?} is not available in the current project"))?
            .label_id;
        let mut candidate = self.project.clone();
        let label = candidate
            .semantic_registry
            .iter_mut()
            .find(|label| label.id == label_id)
            .ok_or_else(|| format!("semantic label {label_id} is missing"))?;
        label.nodes = nodes;
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn axis_label(&self, dimension: AxisDimension) -> &[LabelNode] {
        let identity = match dimension {
            AxisDimension::X => AxisIdentity::X1,
            AxisDimension::Y => AxisIdentity::Y1,
        };
        self.axis_label_by_identity(identity)
    }

    pub fn axis_label_by_identity(&self, identity: AxisIdentity) -> &[LabelNode] {
        let Some(axis) = self.axis_record_by_identity(identity) else {
            return &[];
        };
        let label_id = axis.label_id;
        self.project
            .semantic_registry
            .iter()
            .find(|label| label.id == label_id)
            .map(|label| label.nodes.as_slice())
            .unwrap_or(&[])
    }

    pub fn figure_size_mm(&self) -> (f64, f64) {
        (self.project.figure.width_mm, self.project.figure.height_mm)
    }

    pub fn set_figure_size_mm(&mut self, width: f64, height: f64) -> Result<(), String> {
        if !width.is_finite()
            || !height.is_finite()
            || !(20.0..=500.0).contains(&width)
            || !(20.0..=500.0).contains(&height)
        {
            return Err("figure width and height must be within 20..=500 mm".to_owned());
        }
        self.project.figure.width_mm = width;
        self.project.figure.height_mm = height;
        Ok(())
    }
}

fn axis_value_is_visible(axis: &crate::AxisRecord, value: f64) -> bool {
    value.is_finite()
        && value >= axis.minimum
        && value <= axis.maximum
        && (!matches!(axis.scale, crate::AxisScale::Log10) || value > 0.0)
}

fn ensure_secondary_axis(project: &mut ProjectDocument, mode: AxisMode) {
    let (axis_id, label_id) = (
        next_stable_id(project, "axis"),
        next_stable_id(project, "axis-label"),
    );
    let axes = &mut project.figure.axes[0];
    let target = match mode {
        AxisMode::Single => return,
        AxisMode::DualX => &mut axes.x2,
        AxisMode::DualY => &mut axes.y2,
    };
    if target.is_some() {
        return;
    }
    let mut record = match mode {
        AxisMode::DualX => axes.x.clone(),
        AxisMode::DualY => axes.y.clone(),
        AxisMode::Single => unreachable!(),
    };
    record.id = axis_id;
    record.label_id = label_id.clone();
    record.autoscale = true;
    record.appearance.spine_color_id = "object-black".to_owned();
    project.semantic_registry.push(SemanticLabel {
        id: label_id,
        nodes: vec![LabelNode::Text(String::new())],
    });
    *target = Some(record);
}
