use super::*;

impl FigureDocument {
    pub fn artist_record(&self, artist_id: &str) -> Option<ArtistRecord> {
        self.project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == artist_id)
            .cloned()
    }

    pub fn add_annotation(&mut self, nodes: Vec<LabelNode>) -> Result<String, String> {
        if nodes.is_empty() {
            return Err("annotation text cannot be empty".to_owned());
        }
        let mut candidate = self.project.clone();
        let artist_id = next_stable_id(&candidate, "annotation");
        let label_id = next_stable_id(&candidate, "label-annotation");
        let existing_notes = candidate
            .figure
            .artists
            .iter()
            .filter(|artist| artist.visible && artist.kind == ArtistKind::Annotation)
            .count();
        let width_pt = candidate.figure.width_mm * 72.0 / 25.4;
        let height_pt = candidate.figure.height_mm * 72.0 / 25.4;
        // Automatic legends begin near the upper edge. New annotations start
        // in the lower-left region so the first drag reliably selects the text
        // the user just created instead of an overlapping legend.
        let x_pt = (width_pt * 0.18).clamp(12.0, width_pt - 12.0);
        let y_pt =
            (height_pt * 0.78 - (existing_notes % 8) as f64 * 14.0).clamp(12.0, height_pt - 12.0);
        candidate.semantic_registry.push(SemanticLabel {
            id: label_id.clone(),
            nodes,
        });
        candidate.figure.artists.push(ArtistRecord {
            id: artist_id.clone(),
            kind: ArtistKind::Annotation,
            role: ArtistRole::Annotation,
            visible: true,
            properties: ArtistProperties::Annotation {
                label_id,
                x_pt,
                y_pt,
                connectors: Vec::new(),
            },
        });
        candidate.figure.axes[0].artist_ids.push(artist_id.clone());
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(artist_id)
    }

    pub fn add_reference_line(
        &mut self,
        orientation: ReferenceOrientation,
        value: f64,
        axes: AxisBinding,
    ) -> Result<String, String> {
        let mut candidate = self.project.clone();
        let artist_id = next_stable_id(&candidate, "reference-line");
        let axes = match orientation {
            ReferenceOrientation::Vertical => AxisBinding {
                x: axes.x,
                y: YAxisSlot::Y1,
            },
            ReferenceOrientation::Horizontal => AxisBinding {
                x: XAxisSlot::X1,
                y: axes.y,
            },
        };
        candidate.figure.artists.push(ArtistRecord {
            id: artist_id.clone(),
            kind: ArtistKind::ReferenceLine,
            role: ArtistRole::Reference,
            visible: true,
            properties: ArtistProperties::ReferenceLine {
                orientation,
                value,
                axes,
                stroke: StrokeStyle {
                    color_id: "object-black".to_owned(),
                    width_pt: 0.9,
                    dash_pt: vec![4.0, 3.0],
                },
                include_in_autoscale: false,
            },
        });
        candidate.figure.axes[0].artist_ids.push(artist_id.clone());
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(artist_id)
    }

    pub fn add_measurement_arrow(&mut self, spec: MeasurementArrowSpec) -> Result<String, String> {
        let mut candidate = self.project.clone();
        let artist_id = next_stable_id(&candidate, "measurement-arrow");
        let label_id = spec.label_nodes.map(|nodes| {
            let label_id = next_stable_id(&candidate, "label-measurement");
            candidate.semantic_registry.push(SemanticLabel {
                id: label_id.clone(),
                nodes,
            });
            label_id
        });
        let (mut end_x, mut end_y) = spec.end;
        match spec.constraint {
            MeasurementConstraint::Free => {}
            MeasurementConstraint::Horizontal => end_y = spec.start.1,
            MeasurementConstraint::Vertical => end_x = spec.start.0,
        }
        candidate.figure.artists.push(ArtistRecord {
            id: artist_id.clone(),
            kind: ArtistKind::MeasurementArrow,
            role: ArtistRole::Annotation,
            visible: true,
            properties: ArtistProperties::MeasurementArrow {
                start_x: spec.start.0,
                start_y: spec.start.1,
                end_x,
                end_y,
                axes: spec.axes,
                stroke: StrokeStyle {
                    color_id: "object-black".to_owned(),
                    width_pt: 0.9,
                    dash_pt: Vec::new(),
                },
                start_arrow: spec.start_arrow,
                end_arrow: spec.end_arrow,
                arrow_head: ArrowHead::Open,
                arrow_size_pt: 5.0,
                constraint: spec.constraint,
                label_id,
                label_offset_x_pt: 0.0,
                label_offset_y_pt: -8.0,
            },
        });
        candidate.figure.axes[0].artist_ids.push(artist_id.clone());
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(artist_id)
    }

    pub fn set_artist_record(&mut self, mut record: ArtistRecord) -> Result<(), String> {
        match &mut record.properties {
            ArtistProperties::ReferenceLine {
                orientation, axes, ..
            } => match orientation {
                ReferenceOrientation::Vertical => axes.y = YAxisSlot::Y1,
                ReferenceOrientation::Horizontal => axes.x = XAxisSlot::X1,
            },
            ArtistProperties::MeasurementArrow {
                start_x,
                start_y,
                end_x,
                end_y,
                stroke,
                constraint,
                ..
            } => {
                stroke.color_id = "object-black".to_owned();
                match constraint {
                    MeasurementConstraint::Free => {}
                    MeasurementConstraint::Horizontal => *end_y = *start_y,
                    MeasurementConstraint::Vertical => *end_x = *start_x,
                }
            }
            _ => {}
        }
        let mut candidate = self.project.clone();
        let artist = candidate
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.id == record.id)
            .ok_or_else(|| format!("artist {} is missing", record.id))?;
        if artist.kind != record.kind {
            return Err("artist kind cannot be replaced".to_owned());
        }
        *artist = record.clone();
        let value = serde_json::to_value(&record.properties)
            .map_err(|error| format!("artist style cannot be recorded: {error}"))?;
        if let Some(existing) = candidate
            .overrides
            .iter_mut()
            .find(|item| item.target_id == record.id && item.property == "artist_style")
        {
            existing.value = value;
        } else {
            candidate.overrides.push(crate::OverrideRecord {
                target_id: record.id,
                property: "artist_style".to_owned(),
                value,
            });
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn delete_drawing_object(&mut self, artist_id: &str) -> Result<(), String> {
        let position = self
            .project
            .figure
            .artists
            .iter()
            .position(|artist| artist.id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        if matches!(
            self.project.figure.artists[position].kind,
            ArtistKind::Line | ArtistKind::Scatter | ArtistKind::ErrorBar | ArtistKind::Legend
        ) {
            return Err("the selected artist is not a removable drawing object".to_owned());
        }
        let refresh_autoscale = matches!(
            self.project.figure.artists[position].properties,
            ArtistProperties::ReferenceLine {
                include_in_autoscale: true,
                ..
            }
        );
        let removed = self.project.figure.artists.remove(position);
        self.project.figure.axes[0]
            .artist_ids
            .retain(|id| id != artist_id);
        self.project
            .overrides
            .retain(|record| record.target_id != artist_id);
        let owned_label = match removed.properties {
            ArtistProperties::Annotation { label_id, .. } => Some(label_id),
            ArtistProperties::MeasurementArrow { label_id, .. } => label_id,
            _ => None,
        };
        if let Some(label_id) = owned_label {
            self.project
                .semantic_registry
                .retain(|label| label.id != label_id);
        }
        if refresh_autoscale {
            refresh_active_autoscales(&mut self.project)?;
        }
        self.project.validate().map_err(|error| error.to_string())
    }

    pub fn set_measurement_arrow_label(
        &mut self,
        artist_id: &str,
        nodes: Option<Vec<LabelNode>>,
    ) -> Result<(), String> {
        let mut candidate = self.project.clone();
        let position = candidate
            .figure
            .artists
            .iter()
            .position(|artist| artist.id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        let old_label_id = match &candidate.figure.artists[position].properties {
            ArtistProperties::MeasurementArrow { label_id, .. } => label_id.clone(),
            _ => return Err("the selected artist is not a measurement arrow".to_owned()),
        };
        let new_label_id = match nodes {
            Some(nodes) if nodes.is_empty() => {
                return Err("measurement label cannot be empty".to_owned());
            }
            Some(nodes) => {
                let label_id = old_label_id
                    .clone()
                    .unwrap_or_else(|| next_stable_id(&candidate, "label-measurement"));
                if let Some(label) = candidate
                    .semantic_registry
                    .iter_mut()
                    .find(|label| label.id == label_id)
                {
                    label.nodes = nodes;
                } else {
                    candidate.semantic_registry.push(SemanticLabel {
                        id: label_id.clone(),
                        nodes,
                    });
                }
                Some(label_id)
            }
            None => None,
        };
        if let ArtistProperties::MeasurementArrow { label_id, .. } =
            &mut candidate.figure.artists[position].properties
        {
            *label_id = new_label_id.clone();
        }
        if new_label_id.is_none()
            && let Some(old_label_id) = old_label_id
        {
            candidate
                .semantic_registry
                .retain(|label| label.id != old_label_id);
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn semantic_label_nodes(&self, label_id: &str) -> Option<&[LabelNode]> {
        self.project
            .semantic_registry
            .iter()
            .find(|label| label.id == label_id)
            .map(|label| label.nodes.as_slice())
    }

    pub fn set_semantic_label_nodes(
        &mut self,
        label_id: &str,
        nodes: Vec<LabelNode>,
    ) -> Result<(), String> {
        if nodes.is_empty() {
            return Err("semantic label cannot be empty".to_owned());
        }
        let label = self
            .project
            .semantic_registry
            .iter_mut()
            .find(|label| label.id == label_id)
            .ok_or_else(|| format!("semantic label {label_id} is missing"))?;
        label.nodes = nodes;
        Ok(())
    }

    pub fn palette_colors(&self) -> &[PaletteColor] {
        &self.project.palette.colors
    }

    pub fn palette_id(&self) -> &str {
        &self.project.palette.id
    }

    pub fn series_palette_colors(&self) -> Vec<&PaletteColor> {
        palette_series_color_ids(&self.project.palette.id)
            .iter()
            .filter_map(|id| {
                self.project
                    .palette
                    .colors
                    .iter()
                    .find(|color| color.id == *id)
            })
            .collect()
    }

    /// Changes the active colour scheme and deterministically recolours every
    /// visual XY series. Line, marker, error bar, and linked fit styles share
    /// one colour; independent Y columns in one source remain distinct.
    pub fn set_palette(&mut self, palette_id: &str) -> Result<(), String> {
        let registry = builtin_palette_registry(palette_id)
            .ok_or_else(|| format!("unknown built-in palette {palette_id}"))?;
        let series_colors = palette_series_color_ids(palette_id);
        if series_colors.is_empty() {
            return Err("the selected palette has no series colours".to_owned());
        }

        let mut candidate = self.project.clone();
        let old_series_colors = palette_series_color_ids(&candidate.palette.id);
        let available_colors = registry
            .colors
            .iter()
            .map(|color| color.id.clone())
            .collect::<BTreeSet<_>>();
        let migrate_guide_color = |old: &str| -> String {
            if let Some(index) = old_series_colors.iter().position(|color| *color == old) {
                return series_colors[index % series_colors.len()].to_owned();
            }
            if available_colors.contains(old) {
                return old.to_owned();
            }
            "object-black".to_owned()
        };
        for axes in &mut candidate.figure.axes {
            axes.x.appearance.spine_color_id =
                migrate_guide_color(&axes.x.appearance.spine_color_id);
            axes.y.appearance.spine_color_id =
                migrate_guide_color(&axes.y.appearance.spine_color_id);
            if let Some(axis) = &mut axes.x2 {
                axis.appearance.spine_color_id =
                    migrate_guide_color(&axis.appearance.spine_color_id);
            }
            if let Some(axis) = &mut axes.y2 {
                axis.appearance.spine_color_id =
                    migrate_guide_color(&axis.appearance.spine_color_id);
            }
        }
        candidate.palette = registry;
        let mut visual_series_colors = BTreeMap::<String, String>::new();
        let binding_keys = candidate
            .figure
            .artists
            .iter()
            .filter_map(|artist| {
                artist_binding(artist)
                    .map(|binding| (artist.id.clone(), series_color_key(&candidate, binding)))
            })
            .collect::<BTreeMap<_, _>>();
        let mut changed_properties = BTreeMap::new();
        for artist in &mut candidate.figure.artists {
            let role_color = match artist.role {
                ArtistRole::Theory => Some("neutral-primary"),
                ArtistRole::Reference => Some("neutral-secondary"),
                ArtistRole::Baseline => Some("object-black"),
                _ => None,
            };
            match &mut artist.properties {
                ArtistProperties::Line { stroke, .. }
                | ArtistProperties::ErrorBar { stroke, .. } => {
                    let color = role_color.or_else(|| {
                        let key = binding_keys.get(&artist.id)?;
                        let next_index = visual_series_colors.len() % series_colors.len();
                        Some(
                            visual_series_colors
                                .entry(key.clone())
                                .or_insert_with(|| series_colors[next_index].to_owned())
                                .as_str(),
                        )
                    });
                    if let Some(color) = color {
                        stroke.color_id = color.to_owned();
                    }
                }
                ArtistProperties::Scatter { marker, .. } => {
                    let color = role_color.or_else(|| {
                        let key = binding_keys.get(&artist.id)?;
                        let next_index = visual_series_colors.len() % series_colors.len();
                        Some(
                            visual_series_colors
                                .entry(key.clone())
                                .or_insert_with(|| series_colors[next_index].to_owned())
                                .as_str(),
                        )
                    });
                    if let Some(color) = color {
                        marker.color_id = color.to_owned();
                    }
                }
                ArtistProperties::ReferenceLine { stroke, .. } => {
                    stroke.color_id = migrate_guide_color(&stroke.color_id);
                }
                ArtistProperties::Annotation { connectors, .. } => {
                    for connector in connectors {
                        connector.stroke.color_id = migrate_guide_color(&connector.stroke.color_id);
                    }
                }
                ArtistProperties::MeasurementArrow { stroke, .. } => {
                    stroke.color_id = "object-black".to_owned();
                }
                ArtistProperties::Legend { .. } => {}
            }
            changed_properties.insert(
                artist.id.clone(),
                serde_json::to_value(&artist.properties)
                    .map_err(|error| format!("artist style cannot be recorded: {error}"))?,
            );
        }
        for record in &mut candidate.overrides {
            if record.property == "artist_style"
                && let Some(value) = changed_properties.get(&record.target_id)
            {
                record.value = value.clone();
            }
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn export_preferences(&self) -> &crate::ExportPreferences {
        &self.project.export_preferences
    }

    pub fn set_export_preferences(
        &mut self,
        preferences: crate::ExportPreferences,
    ) -> Result<(), String> {
        let mut candidate = self.project.clone();
        candidate.export_preferences = preferences;
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn set_all_marker_density(&mut self, size_pt: f64, interval: usize) -> Result<(), String> {
        if !size_pt.is_finite() || size_pt <= 0.0 || interval == 0 {
            return Err("marker size and interval must be positive".to_owned());
        }
        let mut candidate = self.project.clone();
        for artist in &mut candidate.figure.artists {
            if let ArtistProperties::Scatter { marker, .. } = &mut artist.properties {
                marker.size_pt = size_pt;
                marker.interval = interval;
            }
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn set_all_marker_sizes(&mut self, size_pt: f64) -> Result<(), String> {
        if !size_pt.is_finite() || size_pt <= 0.0 {
            return Err("marker size must be positive".to_owned());
        }
        let mut candidate = self.project.clone();
        for artist in &mut candidate.figure.artists {
            if let ArtistProperties::Scatter { marker, .. } = &mut artist.properties {
                marker.size_pt = size_pt;
            }
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn set_all_marker_intervals(&mut self, interval: usize) -> Result<(), String> {
        if interval == 0 {
            return Err("marker interval must be positive".to_owned());
        }
        let mut candidate = self.project.clone();
        for artist in &mut candidate.figure.artists {
            if let ArtistProperties::Scatter { marker, .. } = &mut artist.properties {
                marker.interval = interval;
            }
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn set_all_marker_filled(&mut self, filled: bool) -> Result<(), String> {
        let mut candidate = self.project.clone();
        for artist in &mut candidate.figure.artists {
            if let ArtistProperties::Scatter { marker, .. } = &mut artist.properties {
                marker.filled = filled;
            }
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }
}
