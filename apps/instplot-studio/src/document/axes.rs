use super::*;

impl FigureDocument {
    pub fn axis_mode(&self) -> AxisMode {
        self.project.figure.axes[0].mode
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
        let label = self
            .project
            .semantic_registry
            .iter_mut()
            .find(|label| label.id == label_id)
            .ok_or_else(|| format!("semantic label {label_id} is missing"))?;
        label.nodes = nodes;
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
    project.semantic_registry.push(SemanticLabel {
        id: label_id,
        nodes: vec![LabelNode::Text(String::new())],
    });
    *target = Some(record);
}
