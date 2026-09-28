use super::*;

impl FigureDocument {
    pub fn set_series_color(&mut self, artist_id: &str, color_id: &str) -> Result<(), String> {
        if !self
            .project
            .palette
            .colors
            .iter()
            .any(|color| color.id == color_id)
        {
            return Err(format!("unknown palette color {color_id}"));
        }
        let mut candidate = self.project.clone();
        apply_series_color(&mut candidate, artist_id, color_id)?;
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn series_axis_binding(&self, artist_id: &str) -> Option<AxisBinding> {
        self.project
            .series_group_for_artist(artist_id)
            .map(|group| group.axes)
    }

    pub fn set_series_axis_binding(
        &mut self,
        artist_id: &str,
        axes: AxisBinding,
    ) -> Result<(), String> {
        if !axes.is_supported() {
            return Err("a series cannot use X2 and Y2 at the same time".to_owned());
        }
        let mode = self.project.figure.axes[0].mode;
        if !axes.is_enabled_in(mode) {
            return Err(format!(
                "axis combination {:?}/{:?} is not enabled in {mode:?} mode",
                axes.x, axes.y
            ));
        }
        let mut candidate = self.project.clone();
        let data_binding = candidate
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == artist_id)
            .and_then(|artist| match &artist.properties {
                ArtistProperties::Line { binding, .. }
                | ArtistProperties::Scatter { binding, .. }
                | ArtistProperties::ErrorBar { binding, .. } => Some(binding.clone()),
                _ => None,
            });
        let group_index = candidate.figure.axes[0]
            .series_groups
            .iter_mut()
            .position(|group| group.artist_ids.iter().any(|id| id == artist_id))
            .ok_or_else(|| format!("artist {artist_id} has no logical series group"))?;
        let previous_axes = candidate.figure.axes[0].series_groups[group_index].axes;
        if previous_axes == axes {
            return Ok(());
        }
        let group_ids = candidate.figure.axes[0].series_groups[group_index]
            .artist_ids
            .clone();
        let contributed_visible_data = candidate.figure.artists.iter().any(|artist| {
            group_ids.contains(&artist.id)
                && candidate.artist_effectively_visible(artist)
                && artist_binding(artist).is_some()
        });
        candidate.figure.axes[0].series_groups[group_index].axes = axes;
        let contributes_visible_data = candidate.figure.artists.iter().any(|artist| {
            group_ids.contains(&artist.id)
                && candidate.artist_effectively_visible(artist)
                && artist_binding(artist).is_some()
        });
        if let Some(binding) = data_binding {
            suggest_empty_secondary_axis_label(&mut candidate, axes, &binding);
        }
        if contributed_visible_data || contributes_visible_data {
            restore_autoscale_for_axes(
                &mut candidate,
                axis_identities_for_binding(previous_axes)
                    .into_iter()
                    .chain(axis_identities_for_binding(axes)),
            )?;
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn create_series(
        &mut self,
        data_source_id: &str,
        x_column: &str,
        y_column: &str,
        style: SeriesCreationStyle,
    ) -> Result<Vec<String>, String> {
        validate_binding_columns(&self.project, data_source_id, x_column, y_column)?;
        let source = self
            .project
            .data_sources
            .iter()
            .find(|source| source.id == data_source_id)
            .ok_or_else(|| format!("data source {data_source_id} is missing"))?;
        let role = match source.kind {
            DataSourceKind::Source => ArtistRole::Data,
            DataSourceKind::Fit => ArtistRole::Fit,
        };
        let source_label = source.label.clone();
        let color_id = default_series_color(&self.project, data_source_id, x_column, y_column);
        let marker_shape = default_series_marker(&self.project, data_source_id, x_column, y_column);
        let kinds = match style {
            SeriesCreationStyle::Line => vec![ArtistKind::Line],
            SeriesCreationStyle::Scatter => vec![ArtistKind::Scatter],
            SeriesCreationStyle::LineAndMarker => vec![ArtistKind::Line, ArtistKind::Scatter],
        };
        let mut created = Vec::new();
        let group_id = next_stable_id(&self.project, "series-group");
        let label_id = next_stable_id(&self.project, "series-label");
        self.project.semantic_registry.push(SemanticLabel {
            id: label_id.clone(),
            nodes: vec![LabelNode::Text(source_label)],
        });
        for kind in kinds {
            let id = next_stable_id(&self.project, "series");
            let binding = DataBinding {
                data_source_id: data_source_id.to_owned(),
                x_column: x_column.to_owned(),
                y_column: y_column.to_owned(),
            };
            let properties = match kind {
                ArtistKind::Line => ArtistProperties::Line {
                    binding,
                    stroke: StrokeStyle {
                        color_id: color_id.clone(),
                        width_pt: DEFAULT_CURVE_WIDTH_PT,
                        dash_pt: Vec::new(),
                    },
                },
                ArtistKind::Scatter => ArtistProperties::Scatter {
                    binding,
                    marker: MarkerStyle {
                        color_id: color_id.clone(),
                        shape: marker_shape,
                        size_pt: 4.0,
                        filled: true,
                        interval: 1,
                    },
                },
                _ => unreachable!("series creation only constructs line/scatter artists"),
            };
            self.project.figure.artists.push(ArtistRecord {
                id: id.clone(),
                kind,
                role,
                visible: true,
                properties,
            });
            self.project.figure.axes[0].artist_ids.push(id.clone());
            created.push(id);
        }
        self.project.figure.axes[0]
            .series_groups
            .push(SeriesGroupRecord {
                id: group_id,
                artist_ids: created.clone(),
                axes: AxisBinding::PRIMARY,
            });
        let legend_entry = LegendEntry {
            artist_id: created[0].clone(),
            label_id,
            visible: true,
        };
        if let Some(legend) = self
            .project
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.kind == ArtistKind::Legend)
        {
            let ArtistProperties::Legend { entries, .. } = &mut legend.properties else {
                unreachable!("legend kind and properties are validated together")
            };
            entries.push(legend_entry);
        } else {
            let legend_id = next_stable_id(&self.project, "legend");
            self.project.figure.artists.push(ArtistRecord {
                id: legend_id.clone(),
                kind: ArtistKind::Legend,
                role: ArtistRole::Legend,
                visible: true,
                properties: ArtistProperties::Legend {
                    entries: vec![legend_entry],
                    x_pt: 110.0,
                    y_pt: 12.0,
                    placement: LegendPlacement::Auto,
                    position_custom: false,
                    grid: LegendGrid::Auto,
                },
            });
            self.project.figure.axes[0].artist_ids.push(legend_id);
        }
        Ok(created)
    }

    pub fn set_series_legend_label(
        &mut self,
        artist_id: &str,
        nodes: Vec<LabelNode>,
    ) -> Result<(), String> {
        let label_id = self
            .project
            .figure
            .artists
            .iter()
            .find_map(|artist| match &artist.properties {
                ArtistProperties::Legend { entries, .. } => entries
                    .iter()
                    .find(|entry| entry.artist_id == artist_id)
                    .map(|entry| entry.label_id.clone()),
                _ => None,
            })
            .ok_or_else(|| format!("series {artist_id} has no legend entry"))?;
        self.set_semantic_label_nodes(&label_id, nodes)
    }

    pub fn create_error_bars(
        &mut self,
        data_source_id: &str,
        x_column: &str,
        y_column: &str,
        y_error_column: &str,
        x_error_column: Option<&str>,
    ) -> Result<String, String> {
        validate_binding_columns(&self.project, data_source_id, x_column, y_column)?;
        validate_error_column(&self.project, data_source_id, y_error_column)?;
        if let Some(column) = x_error_column {
            validate_error_column(&self.project, data_source_id, column)?;
        }
        let color_id = default_series_color(&self.project, data_source_id, x_column, y_column);
        let id = next_stable_id(&self.project, "error-bars");
        self.project.figure.artists.push(ArtistRecord {
            id: id.clone(),
            kind: ArtistKind::ErrorBar,
            role: ArtistRole::Data,
            visible: true,
            properties: ArtistProperties::ErrorBar {
                binding: DataBinding {
                    data_source_id: data_source_id.to_owned(),
                    x_column: x_column.to_owned(),
                    y_column: y_column.to_owned(),
                },
                x_error_column: x_error_column.map(str::to_owned),
                y_error_column: y_error_column.to_owned(),
                cap_width_pt: 4.0,
                stroke: StrokeStyle {
                    color_id,
                    width_pt: DEFAULT_ERROR_BAR_WIDTH_PT,
                    dash_pt: Vec::new(),
                },
            },
        });
        self.project.figure.axes[0].artist_ids.push(id.clone());
        let binding = DataBinding {
            data_source_id: data_source_id.to_owned(),
            x_column: x_column.to_owned(),
            y_column: y_column.to_owned(),
        };
        let matching_groups = self.project.figure.axes[0]
            .series_groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| {
                group
                    .artist_ids
                    .iter()
                    .filter_map(|artist_id| {
                        self.project
                            .figure
                            .artists
                            .iter()
                            .find(|artist| artist.id == *artist_id)
                    })
                    .any(|artist| artist_binding(artist) == Some(&binding))
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        if let [index] = matching_groups.as_slice() {
            self.project.figure.axes[0].series_groups[*index]
                .artist_ids
                .push(id.clone());
        } else {
            let group_id = next_stable_id(&self.project, "series-group");
            self.project.figure.axes[0]
                .series_groups
                .push(SeriesGroupRecord {
                    id: group_id,
                    artist_ids: vec![id.clone()],
                    axes: AxisBinding::PRIMARY,
                });
        }
        Ok(id)
    }

    pub fn duplicate_series(&mut self, artist_id: &str) -> Result<String, String> {
        let artist = self
            .project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == artist_id)
            .cloned()
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        if !matches!(
            artist.kind,
            ArtistKind::Line | ArtistKind::Scatter | ArtistKind::ErrorBar
        ) {
            return Err("only data series can be duplicated".to_owned());
        }
        let new_id = next_stable_id(&self.project, "series");
        let mut duplicate = artist;
        duplicate.id = new_id.clone();
        self.project.figure.artists.push(duplicate);
        let axes = &mut self.project.figure.axes[0];
        let position = axes
            .artist_ids
            .iter()
            .position(|id| id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is not attached to the axes"))?;
        axes.artist_ids.insert(position + 1, new_id.clone());
        let inherited_axes = self.project.figure.axes[0]
            .series_groups
            .iter()
            .find(|group| group.artist_ids.iter().any(|id| id == artist_id))
            .map(|group| group.axes)
            .unwrap_or(AxisBinding::PRIMARY);
        let group_id = next_stable_id(&self.project, "series-group");
        self.project.figure.axes[0]
            .series_groups
            .push(SeriesGroupRecord {
                id: group_id,
                artist_ids: vec![new_id.clone()],
                axes: inherited_axes,
            });
        for candidate in &mut self.project.figure.artists {
            if let ArtistProperties::Legend { entries, .. } = &mut candidate.properties
                && let Some(entry) = entries
                    .iter()
                    .find(|entry| entry.artist_id == artist_id)
                    .cloned()
            {
                entries.push(LegendEntry {
                    artist_id: new_id.clone(),
                    label_id: entry.label_id,
                    visible: entry.visible,
                });
                break;
            }
        }
        Ok(new_id)
    }

    pub fn delete_series(&mut self, artist_id: &str) -> Result<(), String> {
        let position = self
            .project
            .figure
            .artists
            .iter()
            .position(|artist| artist.id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        if self.project.figure.artists[position].kind == ArtistKind::Legend {
            return Err("the legend is managed separately from data series".to_owned());
        }
        let removed = self.project.figure.artists.remove(position);
        self.project.figure.axes[0]
            .artist_ids
            .retain(|id| id != artist_id);
        for group in &mut self.project.figure.axes[0].series_groups {
            group.artist_ids.retain(|id| id != artist_id);
        }
        self.project.figure.axes[0]
            .series_groups
            .retain(|group| !group.artist_ids.is_empty());
        prune_legend_entries(&mut self.project, &[artist_id.to_owned()]);
        self.project
            .overrides
            .retain(|override_record| override_record.target_id != artist_id);
        if let ArtistProperties::Annotation { label_id, .. } = removed.properties {
            let axes = &self.project.figure.axes[0];
            let still_used = axes.x.label_id == label_id
                || axes.y.label_id == label_id
                || self
                    .project
                    .figure
                    .artists
                    .iter()
                    .any(|artist| match &artist.properties {
                        ArtistProperties::Annotation {
                            label_id: current, ..
                        } => *current == label_id,
                        ArtistProperties::Legend { entries, .. } => {
                            entries.iter().any(|entry| entry.label_id == label_id)
                        }
                        _ => false,
                    });
            if !still_used {
                self.project
                    .semantic_registry
                    .retain(|label| label.id != label_id);
            }
        }
        Ok(())
    }

    pub fn series_style(&self, artist_id: &str) -> Option<SeriesCreationStyle> {
        let group = self.project.figure.axes[0]
            .series_groups
            .iter()
            .find(|group| group.artist_ids.iter().any(|id| id == artist_id))?;
        let grouped_ids = group.artist_ids.iter().collect::<BTreeSet<_>>();
        let selected = self
            .project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == artist_id)?;
        artist_binding(selected)?;
        let mut line = false;
        let mut scatter = false;
        for artist in &self.project.figure.artists {
            if !grouped_ids.contains(&artist.id) {
                continue;
            }
            line |= artist.kind == ArtistKind::Line;
            scatter |= artist.kind == ArtistKind::Scatter;
        }
        match (line, scatter) {
            (true, true) => Some(SeriesCreationStyle::LineAndMarker),
            (true, false) => Some(SeriesCreationStyle::Line),
            (false, true) => Some(SeriesCreationStyle::Scatter),
            (false, false) => None,
        }
    }

    pub fn set_series_style(
        &mut self,
        artist_id: &str,
        style: SeriesCreationStyle,
    ) -> Result<(), String> {
        let mut candidate = self.project.clone();
        let selected_index = candidate
            .figure
            .artists
            .iter()
            .position(|artist| artist.id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        let selected = candidate.figure.artists[selected_index].clone();
        if !matches!(selected.kind, ArtistKind::Line | ArtistKind::Scatter) {
            return Err("only a line or point series can change plot type".to_owned());
        }
        let binding = artist_binding(&selected)
            .cloned()
            .ok_or_else(|| "the selected series has no data binding".to_owned())?;
        let group_index = candidate.figure.axes[0]
            .series_groups
            .iter()
            .position(|group| group.artist_ids.iter().any(|id| id == artist_id))
            .ok_or_else(|| format!("artist {artist_id} has no logical series group"))?;
        let grouped_ids = candidate.figure.axes[0].series_groups[group_index]
            .artist_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let related = candidate
            .figure
            .artists
            .iter()
            .filter(|artist| {
                matches!(artist.kind, ArtistKind::Line | ArtistKind::Scatter)
                    && grouped_ids.contains(&artist.id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let line_style = related.iter().find_map(|artist| match &artist.properties {
            ArtistProperties::Line { stroke, .. } => Some(stroke.clone()),
            _ => None,
        });
        let marker_style = related.iter().find_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { marker, .. } => Some(marker.clone()),
            _ => None,
        });
        let fallback_color = artist_color(&selected)
            .map(str::to_owned)
            .or_else(|| line_style.as_ref().map(|stroke| stroke.color_id.clone()))
            .or_else(|| marker_style.as_ref().map(|marker| marker.color_id.clone()))
            .unwrap_or_else(|| {
                default_series_color(
                    &candidate,
                    &binding.data_source_id,
                    &binding.x_column,
                    &binding.y_column,
                )
            });
        let mut line_style = line_style.unwrap_or_else(|| StrokeStyle {
            color_id: fallback_color.clone(),
            width_pt: DEFAULT_CURVE_WIDTH_PT,
            dash_pt: Vec::new(),
        });
        let mut marker_style = marker_style.unwrap_or_else(|| MarkerStyle {
            color_id: fallback_color.clone(),
            shape: default_series_marker(
                &candidate,
                &binding.data_source_id,
                &binding.x_column,
                &binding.y_column,
            ),
            size_pt: 4.0,
            filled: true,
            interval: 1,
        });
        line_style.color_id.clone_from(&fallback_color);
        marker_style.color_id.clone_from(&fallback_color);
        let selected_kind = match style {
            SeriesCreationStyle::Line => ArtistKind::Line,
            SeriesCreationStyle::Scatter => ArtistKind::Scatter,
            SeriesCreationStyle::LineAndMarker => selected.kind,
        };
        let selected_artist = &mut candidate.figure.artists[selected_index];
        selected_artist.kind = selected_kind;
        selected_artist.properties = match selected_kind {
            ArtistKind::Line => ArtistProperties::Line {
                binding: binding.clone(),
                stroke: line_style.clone(),
            },
            ArtistKind::Scatter => ArtistProperties::Scatter {
                binding: binding.clone(),
                marker: marker_style.clone(),
            },
            _ => unreachable!(),
        };
        let removed_ids = related
            .iter()
            .filter(|artist| artist.id != artist_id)
            .map(|artist| artist.id.clone())
            .collect::<Vec<_>>();
        candidate
            .figure
            .artists
            .retain(|artist| !removed_ids.contains(&artist.id));
        candidate.figure.axes[0]
            .artist_ids
            .retain(|id| !removed_ids.contains(id));
        candidate.figure.axes[0].series_groups[group_index]
            .artist_ids
            .retain(|id| !removed_ids.contains(id));
        for artist in &mut candidate.figure.artists {
            if let ArtistProperties::Legend { entries, .. } = &mut artist.properties {
                for entry in entries {
                    if removed_ids.contains(&entry.artist_id) {
                        entry.artist_id = artist_id.to_owned();
                    }
                }
            }
        }
        candidate
            .overrides
            .retain(|record| !removed_ids.contains(&record.target_id));

        if style == SeriesCreationStyle::LineAndMarker {
            let complement_kind = if selected_kind == ArtistKind::Line {
                ArtistKind::Scatter
            } else {
                ArtistKind::Line
            };
            let complement_id = next_stable_id(&candidate, "series");
            let properties = match complement_kind {
                ArtistKind::Line => ArtistProperties::Line {
                    binding,
                    stroke: line_style,
                },
                ArtistKind::Scatter => ArtistProperties::Scatter {
                    binding,
                    marker: marker_style,
                },
                _ => unreachable!(),
            };
            candidate.figure.artists.push(ArtistRecord {
                id: complement_id.clone(),
                kind: complement_kind,
                role: selected.role,
                visible: selected.visible,
                properties,
            });
            let axes = &mut candidate.figure.axes[0].artist_ids;
            let position = axes
                .iter()
                .position(|id| id == artist_id)
                .ok_or_else(|| format!("artist {artist_id} is not attached to the axes"))?;
            axes.insert(position + 1, complement_id.clone());
            candidate.figure.axes[0].series_groups[group_index]
                .artist_ids
                .push(complement_id);
        }
        apply_series_color(&mut candidate, artist_id, &fallback_color)?;
        let selected_index = candidate
            .figure
            .artists
            .iter()
            .position(|artist| artist.id == artist_id)
            .expect("the selected artist is retained during style conversion");
        let value = serde_json::to_value(&candidate.figure.artists[selected_index].properties)
            .map_err(|error| format!("artist style cannot be recorded: {error}"))?;
        if let Some(existing) = candidate
            .overrides
            .iter_mut()
            .find(|record| record.target_id == artist_id && record.property == "artist_style")
        {
            existing.value = value;
        } else {
            candidate.overrides.push(crate::OverrideRecord {
                target_id: artist_id.to_owned(),
                property: "artist_style".to_owned(),
                value,
            });
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn set_series_visible(&mut self, artist_id: &str, visible: bool) -> Result<(), String> {
        let mut candidate = self.project.clone();
        let group = candidate
            .series_group_for_artist(artist_id)
            .map(|group| (group.axes, group.artist_ids.clone()));
        let was_effectively_visible = group.as_ref().is_some_and(|(_, group_ids)| {
            candidate.figure.artists.iter().any(|artist| {
                group_ids.contains(&artist.id)
                    && matches!(
                        artist.properties,
                        ArtistProperties::Line { .. }
                            | ArtistProperties::Scatter { .. }
                            | ArtistProperties::ErrorBar { .. }
                    )
                    && candidate.artist_effectively_visible(artist)
            })
        });
        let artist = candidate
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is missing"))?;
        artist.visible = visible;
        let is_effectively_visible = group.as_ref().is_some_and(|(_, group_ids)| {
            candidate.figure.artists.iter().any(|artist| {
                group_ids.contains(&artist.id)
                    && matches!(
                        artist.properties,
                        ArtistProperties::Line { .. }
                            | ArtistProperties::Scatter { .. }
                            | ArtistProperties::ErrorBar { .. }
                    )
                    && candidate.artist_effectively_visible(artist)
            })
        });
        if was_effectively_visible != is_effectively_visible
            && let Some((axes, _)) = group
        {
            restore_autoscale_for_axes(&mut candidate, axis_identities_for_binding(axes))?;
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn move_series(&mut self, artist_id: &str, direction: MoveDirection) -> Result<(), String> {
        let artist_ids = &mut self.project.figure.axes[0].artist_ids;
        let position = artist_ids
            .iter()
            .position(|id| id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is not attached to the axes"))?;
        let destination = match direction {
            MoveDirection::Earlier if position > 0 => position - 1,
            MoveDirection::Later if position + 1 < artist_ids.len() => position + 1,
            _ => return Ok(()),
        };
        artist_ids.swap(position, destination);
        let order = artist_ids
            .iter()
            .enumerate()
            .map(|(index, id)| (id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        for artist in &mut self.project.figure.artists {
            if let ArtistProperties::Legend { entries, .. } = &mut artist.properties {
                entries.sort_by_key(|entry| {
                    order.get(&entry.artist_id).copied().unwrap_or(usize::MAX)
                });
            }
        }
        Ok(())
    }

    pub fn rebind_series(
        &mut self,
        artist_id: &str,
        data_source_id: &str,
        x_column: &str,
        y_column: &str,
        y_error_column: Option<&str>,
    ) -> Result<(), String> {
        validate_binding_columns(&self.project, data_source_id, x_column, y_column)?;
        if let Some(error_column) = y_error_column {
            validate_column(&self.project, data_source_id, error_column)?;
        }
        let previous_binding = self
            .project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == artist_id)
            .ok_or_else(|| format!("artist {artist_id} is missing"))
            .and_then(|artist| match &artist.properties {
                ArtistProperties::Line { binding, .. }
                | ArtistProperties::Scatter { binding, .. }
                | ArtistProperties::ErrorBar { binding, .. } => Ok(binding.clone()),
                _ => Err("the selected object has no data binding".to_owned()),
            })?;
        let group = self
            .project
            .series_group_for_artist(artist_id)
            .ok_or_else(|| format!("artist {artist_id} has no logical series group"))?;
        let member_ids = group.artist_ids.clone();
        let axes = group.axes;
        let contributes_visible_data = member_ids.iter().any(|member_id| {
            self.project
                .figure
                .artists
                .iter()
                .find(|artist| artist.id == *member_id)
                .is_some_and(|artist| self.project.artist_effectively_visible(artist))
        });
        let error_columns = self
            .project
            .figure
            .artists
            .iter()
            .filter(|artist| member_ids.contains(&artist.id))
            .filter_map(|artist| match &artist.properties {
                ArtistProperties::ErrorBar {
                    x_error_column,
                    y_error_column: current_y_error,
                    ..
                } => Some((
                    x_error_column.clone(),
                    y_error_column.unwrap_or(current_y_error).to_owned(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (x_error, y_error) in &error_columns {
            validate_error_column(&self.project, data_source_id, y_error)?;
            if let Some(column) = x_error.as_deref() {
                validate_error_column(&self.project, data_source_id, column)?;
            }
        }
        let x_identity = match axes.x {
            XAxisSlot::X1 => AxisIdentity::X1,
            XAxisSlot::X2 => AxisIdentity::X2,
        };
        let y_identity = match axes.y {
            YAxisSlot::Y1 => AxisIdentity::Y1,
            YAxisSlot::Y2 => AxisIdentity::Y2,
        };
        // Keep automatically generated axis labels useful, while preserving any label the
        // user has already customized. A label is automatic only while it still consists of
        // the previous bound column name.
        let sync_x_label = axis_label_is_column(
            self.axis_label_by_identity(x_identity),
            &previous_binding.x_column,
        );
        let sync_y_label = axis_label_is_column(
            self.axis_label_by_identity(y_identity),
            &previous_binding.y_column,
        );
        let mut candidate = self.clone();
        let replacement = DataBinding {
            data_source_id: data_source_id.to_owned(),
            x_column: x_column.to_owned(),
            y_column: y_column.to_owned(),
        };
        for member_id in &member_ids {
            let artist = candidate
                .project
                .figure
                .artists
                .iter_mut()
                .find(|artist| artist.id == *member_id)
                .ok_or_else(|| format!("series member {member_id} is missing"))?;
            match &mut artist.properties {
                ArtistProperties::Line { binding, .. }
                | ArtistProperties::Scatter { binding, .. } => *binding = replacement.clone(),
                ArtistProperties::ErrorBar {
                    binding,
                    x_error_column: _,
                    y_error_column: current_y_error,
                    ..
                } => {
                    if let Some(column) = y_error_column {
                        *current_y_error = column.to_owned();
                    }
                    *binding = replacement.clone();
                }
                _ => {}
            }
        }
        if sync_x_label && previous_binding.x_column != x_column {
            candidate.set_axis_label_by_identity(
                x_identity,
                vec![LabelNode::Text(x_column.to_owned())],
            )?;
        }
        if sync_y_label && previous_binding.y_column != y_column {
            candidate.set_axis_label_by_identity(
                y_identity,
                vec![LabelNode::Text(y_column.to_owned())],
            )?;
        }
        let source_changed = previous_binding.data_source_id != data_source_id;
        let mut affected_axes = Vec::new();
        if source_changed || previous_binding.x_column != x_column {
            affected_axes.push(x_identity);
        }
        if source_changed || previous_binding.y_column != y_column || y_error_column.is_some() {
            affected_axes.push(y_identity);
        }
        if contributes_visible_data {
            restore_autoscale_for_axes(&mut candidate.project, affected_axes)?;
        }
        candidate
            .project
            .validate()
            .map_err(|error| error.to_string())?;
        *self = candidate;
        Ok(())
    }

    pub fn set_series_error_columns(
        &mut self,
        artist_id: &str,
        x_error_column: Option<&str>,
        y_error_column: Option<&str>,
    ) -> Result<(), String> {
        let binding = self
            .project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == artist_id)
            .and_then(artist_binding)
            .cloned()
            .ok_or_else(|| format!("series {artist_id} has no data binding"))?;
        if let Some(column) = y_error_column {
            validate_error_column(&self.project, &binding.data_source_id, column)?;
        }
        if let Some(column) = x_error_column {
            validate_error_column(&self.project, &binding.data_source_id, column)?;
        }
        if y_error_column.is_none() && x_error_column.is_some() {
            return Err("X error requires a Y error column".to_owned());
        }
        let mut candidate = self.clone();
        let error_ids = candidate
            .project
            .figure
            .artists
            .iter()
            .filter_map(|artist| match &artist.properties {
                ArtistProperties::ErrorBar {
                    binding: error_binding,
                    ..
                } if *error_binding == binding => Some(artist.id.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let previous_error_columns = error_ids.first().and_then(|error_id| {
            candidate
                .project
                .figure
                .artists
                .iter()
                .find(|artist| artist.id == *error_id)
                .and_then(|artist| match &artist.properties {
                    ArtistProperties::ErrorBar {
                        x_error_column,
                        y_error_column,
                        ..
                    } => Some((x_error_column.clone(), Some(y_error_column.clone()))),
                    _ => None,
                })
        });
        let (previous_x_error, previous_y_error) = previous_error_columns.unwrap_or((None, None));
        match y_error_column {
            Some(y_error_column) if let Some(error_id) = error_ids.first() => {
                let artist = candidate
                    .project
                    .figure
                    .artists
                    .iter_mut()
                    .find(|artist| artist.id == *error_id)
                    .expect("collected error artist remains present");
                let ArtistProperties::ErrorBar {
                    x_error_column: current_x,
                    y_error_column: current_y,
                    ..
                } = &mut artist.properties
                else {
                    unreachable!()
                };
                *current_x = x_error_column.map(str::to_owned);
                *current_y = y_error_column.to_owned();
                for duplicate in error_ids.iter().skip(1) {
                    candidate.delete_series(duplicate)?;
                }
            }
            Some(y_error_column) => {
                candidate.create_error_bars(
                    &binding.data_source_id,
                    &binding.x_column,
                    &binding.y_column,
                    y_error_column,
                    x_error_column,
                )?;
            }
            None => {
                for error_id in error_ids {
                    candidate.delete_series(&error_id)?;
                }
            }
        }
        let axes = candidate
            .project
            .series_group_for_artist(artist_id)
            .map(|group| group.axes)
            .ok_or_else(|| format!("artist {artist_id} has no logical series group"))?;
        let group_ids = candidate
            .project
            .series_group_for_artist(artist_id)
            .map(|group| group.artist_ids.clone())
            .expect("the logical series group was just found");
        let contributes_visible_data = candidate.project.figure.artists.iter().any(|artist| {
            group_ids.contains(&artist.id) && candidate.project.artist_effectively_visible(artist)
        });
        if contributes_visible_data {
            let identities = axis_identities_for_binding(axes);
            let mut affected_axes = Vec::new();
            if previous_x_error.as_deref() != x_error_column {
                affected_axes.push(identities[0]);
            }
            if previous_y_error.as_deref() != y_error_column {
                affected_axes.push(identities[1]);
            }
            restore_autoscale_for_axes(&mut candidate.project, affected_axes)?;
        }
        candidate
            .project
            .validate()
            .map_err(|error| error.to_string())?;
        *self = candidate;
        Ok(())
    }
}

pub(super) fn artist_color(artist: &ArtistRecord) -> Option<&str> {
    match &artist.properties {
        ArtistProperties::Line { stroke, .. } | ArtistProperties::ErrorBar { stroke, .. } => {
            Some(stroke.color_id.as_str())
        }
        ArtistProperties::Scatter { marker, .. } => Some(marker.color_id.as_str()),
        _ => None,
    }
}

pub(super) fn apply_series_color(
    project: &mut ProjectDocument,
    artist_id: &str,
    color_id: &str,
) -> Result<(), String> {
    let member_ids = project
        .series_group_for_artist(artist_id)
        .map(|group| group.artist_ids.clone())
        .ok_or_else(|| format!("artist {artist_id} has no logical series group"))?;
    let mut changed = Vec::new();
    for artist in &mut project.figure.artists {
        if !member_ids.contains(&artist.id) {
            continue;
        }
        match &mut artist.properties {
            ArtistProperties::Line { stroke, .. } | ArtistProperties::ErrorBar { stroke, .. } => {
                stroke.color_id = color_id.to_owned();
                changed.push((artist.id.clone(), artist.properties.clone()));
            }
            ArtistProperties::Scatter { marker, .. } => {
                marker.color_id = color_id.to_owned();
                changed.push((artist.id.clone(), artist.properties.clone()));
            }
            _ => {}
        }
    }
    for (target_id, properties) in changed {
        let value = serde_json::to_value(properties)
            .map_err(|error| format!("artist style cannot be recorded: {error}"))?;
        if let Some(existing) = project
            .overrides
            .iter_mut()
            .find(|record| record.target_id == target_id && record.property == "artist_style")
        {
            existing.value = value;
        } else {
            project.overrides.push(crate::OverrideRecord {
                target_id,
                property: "artist_style".to_owned(),
                value,
            });
        }
    }
    Ok(())
}

pub(super) fn normalize_series_colors(
    project: &mut ProjectDocument,
) -> Result<Vec<String>, String> {
    let groups = project.figure.axes[0].series_groups.clone();
    let fallback = palette_series_color_ids(&project.palette.id)
        .iter()
        .find(|id| project.palette.colors.iter().any(|color| color.id == **id))
        .map(|id| (*id).to_owned())
        .or_else(|| project.palette.colors.first().map(|color| color.id.clone()))
        .ok_or_else(|| "the active palette has no colours".to_owned())?;
    let mut normalized = Vec::new();
    for group in groups {
        let members = group
            .artist_ids
            .iter()
            .filter_map(|id| {
                project
                    .figure
                    .artists
                    .iter()
                    .find(|artist| artist.id == *id)
            })
            .collect::<Vec<_>>();
        let canonical = members
            .iter()
            .find(|artist| {
                artist.visible && matches!(artist.kind, ArtistKind::Line | ArtistKind::Scatter)
            })
            .and_then(|artist| artist_color(artist))
            .or_else(|| {
                members
                    .iter()
                    .find(|artist| artist.kind == ArtistKind::ErrorBar)
                    .and_then(|artist| artist_color(artist))
            })
            .unwrap_or(fallback.as_str())
            .to_owned();
        let inconsistent = members
            .iter()
            .filter_map(|artist| artist_color(artist))
            .any(|color| color != canonical);
        if inconsistent {
            let Some(member) = group.artist_ids.first() else {
                continue;
            };
            apply_series_color(project, member, &canonical)?;
            normalized.push(group.id);
        }
    }
    Ok(normalized)
}

fn suggest_empty_secondary_axis_label(
    project: &mut ProjectDocument,
    axes: AxisBinding,
    binding: &DataBinding,
) {
    let suggestion = match (axes.x, axes.y) {
        (XAxisSlot::X2, YAxisSlot::Y1) => Some((AxisIdentity::X2, &binding.x_column)),
        (XAxisSlot::X1, YAxisSlot::Y2) => Some((AxisIdentity::Y2, &binding.y_column)),
        _ => None,
    };
    let Some((identity, text)) = suggestion else {
        return;
    };
    let axes = &project.figure.axes[0];
    let label_id = match identity {
        AxisIdentity::X2 => axes.x2.as_ref().map(|axis| axis.label_id.as_str()),
        AxisIdentity::Y2 => axes.y2.as_ref().map(|axis| axis.label_id.as_str()),
        AxisIdentity::X1 | AxisIdentity::Y1 => None,
    };
    let Some(label_id) = label_id else {
        return;
    };
    if let Some(label) = project
        .semantic_registry
        .iter_mut()
        .find(|label| label.id == label_id)
        && label.nodes.iter().all(|node| match node {
            LabelNode::Text(value) => value.trim().is_empty(),
            _ => false,
        })
    {
        label.nodes = vec![LabelNode::Text(text.clone())];
    }
}
