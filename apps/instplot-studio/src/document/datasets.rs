use super::*;

impl FigureDocument {
    pub fn data_source_dependency_count(&self, data_source_id: &str) -> usize {
        let dependent_sources = dependent_source_ids(&self.project, data_source_id);
        let mut source_ids = dependent_sources.clone();
        source_ids.push(data_source_id.to_owned());
        dependent_sources.len()
            + self
                .project
                .figure
                .artists
                .iter()
                .filter(|artist| source_ids.iter().any(|id| artist_uses_source(artist, id)))
                .count()
    }

    pub fn delete_data_source(
        &mut self,
        data_source_id: &str,
        cascade: bool,
    ) -> Result<(), String> {
        let ids = vec![data_source_id.to_owned()];
        let (source_count, artist_count) = self.data_source_removal_impact(&ids)?;
        if !cascade && (source_count > 1 || artist_count > 0) {
            return Err(format!(
                "data source has {} dependent source(s) and {} bound artist(s)",
                source_count - 1,
                artist_count
            ));
        }
        self.delete_data_sources(&ids)
    }

    fn removal_source_ids(&self, data_source_ids: &[String]) -> Result<BTreeSet<String>, String> {
        if data_source_ids.is_empty() {
            return Err("no data sources selected".to_owned());
        }
        let mut source_ids = BTreeSet::new();
        for id in data_source_ids {
            if !self
                .project
                .data_sources
                .iter()
                .any(|source| source.id == *id)
            {
                return Err(format!("data source {id} is missing"));
            }
            source_ids.insert(id.clone());
            source_ids.extend(dependent_source_ids(&self.project, id));
        }
        Ok(source_ids)
    }

    pub fn data_source_removal_impact(
        &self,
        data_source_ids: &[String],
    ) -> Result<(usize, usize), String> {
        let source_ids = self.removal_source_ids(data_source_ids)?;
        let artist_count = self
            .project
            .figure
            .artists
            .iter()
            .filter(|artist| source_ids.iter().any(|id| artist_uses_source(artist, id)))
            .count();
        Ok((source_ids.len(), artist_count))
    }

    pub fn delete_data_sources(&mut self, data_source_ids: &[String]) -> Result<(), String> {
        let source_ids = self.removal_source_ids(data_source_ids)?;
        let artist_ids = self
            .project
            .figure
            .artists
            .iter()
            .filter(|artist| source_ids.iter().any(|id| artist_uses_source(artist, id)))
            .map(|artist| artist.id.clone())
            .collect::<Vec<_>>();
        self.project
            .data_sources
            .retain(|source| !source_ids.contains(&source.id));
        self.project
            .figure
            .artists
            .retain(|artist| !artist_ids.contains(&artist.id));
        self.project.figure.axes[0]
            .artist_ids
            .retain(|id| !artist_ids.contains(id));
        prune_legend_entries(&mut self.project, &artist_ids);
        if self.project.data_sources.is_empty() {
            reset_empty_axes(&mut self.project);
        } else {
            for dimension in [AxisDimension::X, AxisDimension::Y] {
                let autoscale = match dimension {
                    AxisDimension::X => self.project.figure.axes[0].x.autoscale,
                    AxisDimension::Y => self.project.figure.axes[0].y.autoscale,
                };
                if autoscale {
                    apply_autoscale(&mut self.project, dimension)?;
                }
            }
        }
        self.project.validate().map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn series(&self) -> Vec<SeriesDescriptor> {
        self.project
            .figure
            .artists
            .iter()
            .map(|artist| {
                let kind = match artist.kind {
                    crate::ArtistKind::Line => SeriesKind::Line,
                    crate::ArtistKind::Scatter => SeriesKind::Scatter,
                    crate::ArtistKind::ErrorBar => SeriesKind::ErrorBar,
                    crate::ArtistKind::ReferenceLine => SeriesKind::ReferenceLine,
                    crate::ArtistKind::Annotation => SeriesKind::Annotation,
                    crate::ArtistKind::Legend => SeriesKind::Legend,
                };
                SeriesDescriptor {
                    id: artist.id.clone(),
                    kind,
                    role: artist.role,
                    visible: artist.visible,
                    label: artist_binding(artist)
                        .and_then(|binding| {
                            self.project
                                .data_sources
                                .iter()
                                .find(|source| source.id == binding.data_source_id)
                        })
                        .map_or_else(|| artist.id.clone(), |source| source.label.clone()),
                    binding: artist_binding(artist).cloned(),
                }
            })
            .collect()
    }

    pub fn linked_series_ids(&self, artist_id: &str) -> Vec<String> {
        let Some(selected) = self
            .project
            .figure
            .artists
            .iter()
            .find(|artist| artist.id == artist_id)
        else {
            return vec![artist_id.to_owned()];
        };
        let Some(binding) = artist_binding(selected) else {
            return vec![artist_id.to_owned()];
        };
        self.project
            .figure
            .artists
            .iter()
            .filter(|artist| artist.visible && artist_binding(artist) == Some(binding))
            .map(|artist| artist.id.clone())
            .collect()
    }

    pub fn project(&self) -> &ProjectDocument {
        &self.project
    }

    pub fn from_project(project: ProjectDocument) -> Result<Self, ProjectError> {
        project.validate()?;
        Ok(Self { project })
    }

    pub fn open(path: &Path) -> Result<(Self, OpenProjectReport), ProjectError> {
        let report = open_project(path)?;
        let document = Self::from_project(report.document.clone())?;
        Ok((document, report))
    }

    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        save_project(path, &self.project)
    }

    pub fn sync_datasets(&mut self, datasets: &[DataSet]) -> Result<(), ProjectError> {
        let mut candidate = self.project.clone();
        for dataset in datasets
            .iter()
            .filter(|dataset| dataset.kind == DataSetKind::Source)
        {
            candidate.upsert_embedded_source(
                dataset.plot_id.clone(),
                dataset.display_name(),
                embedded_columns(dataset),
                dataset.alive.clone(),
                DataSourceKind::Source,
                None,
            )?;
            set_dataset_origin(&mut candidate, dataset);
        }
        for dataset in datasets
            .iter()
            .filter(|dataset| dataset.kind == DataSetKind::Fit)
        {
            let fit = dataset.fit_link.as_ref().ok_or_else(|| {
                ProjectError::Validation(format!(
                    "fit data source {} is missing its fit link",
                    dataset.plot_id
                ))
            })?;
            candidate.upsert_embedded_source(
                dataset.plot_id.clone(),
                dataset.display_name(),
                embedded_columns(dataset),
                dataset.alive.clone(),
                DataSourceKind::Fit,
                Some(FitIdentity {
                    parent_data_source_id: fit.parent_dataset_id.clone().ok_or_else(|| {
                        ProjectError::Validation(format!(
                            "fit data source {} is missing its parent identity",
                            dataset.plot_id
                        ))
                    })?,
                    source_x_column: fit.source_x_column.clone(),
                    source_y_column: fit.source_y_column.clone(),
                    equation: fit.equation.clone(),
                    display_equation: fit.display_equation.clone(),
                }),
            )?;
            set_dataset_origin(&mut candidate, dataset);
        }
        // After “clear all”, the document intentionally has no bound artists yet.
        // Import must be allowed to embed the new sources first; the caller then
        // creates their series and performs one autoscale over the visible data.
        // Autoscaling here would reject that valid intermediate state.
        let has_visible_bound_artist = candidate
            .figure
            .artists
            .iter()
            .any(|artist| artist.visible && artist_binding(artist).is_some());
        if has_visible_bound_artist {
            apply_autoscale(&mut candidate, AxisDimension::X).map_err(ProjectError::Validation)?;
            apply_autoscale(&mut candidate, AxisDimension::Y).map_err(ProjectError::Validation)?;
        }
        self.project = candidate;
        Ok(())
    }
}
