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

    pub fn set_artist_record(&mut self, record: ArtistRecord) -> Result<(), String> {
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
    /// data family. A fit retains its source colour, while theory/reference
    /// roles continue to use semantic neutral ink.
    pub fn set_palette(&mut self, palette_id: &str) -> Result<(), String> {
        let registry = builtin_palette_registry(palette_id)
            .ok_or_else(|| format!("unknown built-in palette {palette_id}"))?;
        let series_colors = palette_series_color_ids(palette_id);
        if series_colors.is_empty() {
            return Err("the selected palette has no series colours".to_owned());
        }

        let mut candidate = self.project.clone();
        candidate.palette = registry;
        let mut family_colors = BTreeMap::<String, String>::new();
        for source in &candidate.data_sources {
            let family_id = source
                .fit
                .as_ref()
                .map_or(source.id.as_str(), |fit| fit.parent_data_source_id.as_str());
            if !family_colors.contains_key(family_id) {
                let index = family_colors.len() % series_colors.len();
                family_colors.insert(family_id.to_owned(), series_colors[index].to_owned());
            }
        }

        let source_families = candidate
            .data_sources
            .iter()
            .map(|source| {
                let family_id = source
                    .fit
                    .as_ref()
                    .map_or(source.id.as_str(), |fit| fit.parent_data_source_id.as_str());
                (source.id.clone(), family_id.to_owned())
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
                ArtistProperties::Line { binding, stroke }
                | ArtistProperties::ErrorBar {
                    binding, stroke, ..
                } => {
                    let color = role_color.or_else(|| {
                        source_families
                            .get(&binding.data_source_id)
                            .and_then(|family| family_colors.get(family))
                            .map(String::as_str)
                    });
                    if let Some(color) = color {
                        stroke.color_id = color.to_owned();
                    }
                }
                ArtistProperties::Scatter { binding, marker } => {
                    let color = role_color.or_else(|| {
                        source_families
                            .get(&binding.data_source_id)
                            .and_then(|family| family_colors.get(family))
                            .map(String::as_str)
                    });
                    if let Some(color) = color {
                        marker.color_id = color.to_owned();
                    }
                }
                ArtistProperties::ReferenceLine { stroke, .. } => {
                    stroke.color_id = role_color.unwrap_or("neutral-primary").to_owned();
                }
                ArtistProperties::Annotation { connectors, .. } => {
                    for connector in connectors {
                        connector.stroke.color_id = "object-black".to_owned();
                    }
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
