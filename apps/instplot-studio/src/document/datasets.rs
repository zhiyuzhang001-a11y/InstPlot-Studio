use super::*;
use std::io::Write;

use atomicwrites::{AllowOverwrite, AtomicFile};

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
        for group in &mut self.project.figure.axes[0].series_groups {
            group.artist_ids.retain(|id| !artist_ids.contains(id));
        }
        self.project.figure.axes[0]
            .series_groups
            .retain(|group| !group.artist_ids.is_empty());
        prune_legend_entries(&mut self.project, &artist_ids);
        if self.project.data_sources.is_empty() {
            reset_empty_axes(&mut self.project);
        } else {
            refresh_active_autoscales(&mut self.project)?;
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
                    effective_visible: self.project.artist_effectively_visible(artist),
                    label: artist_binding(artist)
                        .and_then(|binding| {
                            self.project
                                .data_sources
                                .iter()
                                .find(|source| source.id == binding.data_source_id)
                        })
                        .map_or_else(|| artist.id.clone(), |source| source.label.clone()),
                    binding: artist_binding(artist).cloned(),
                    axes: self.project.artist_axis_binding(artist),
                }
            })
            .collect()
    }

    pub fn linked_series_ids(&self, artist_id: &str) -> Vec<String> {
        self.project
            .series_group_for_artist(artist_id)
            .map(|group| group.artist_ids.clone())
            .unwrap_or_else(|| vec![artist_id.to_owned()])
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
        self.sync_datasets_inner(datasets, true)
    }

    pub fn sync_datasets_without_autoscale(
        &mut self,
        datasets: &[DataSet],
    ) -> Result<(), ProjectError> {
        self.sync_datasets_inner(datasets, false)
    }

    fn sync_datasets_inner(
        &mut self,
        datasets: &[DataSet],
        refresh_autoscale: bool,
    ) -> Result<(), ProjectError> {
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
        let has_visible_bound_artist = candidate.figure.artists.iter().any(|artist| {
            candidate.artist_effectively_visible(artist) && artist_binding(artist).is_some()
        });
        if refresh_autoscale && has_visible_bound_artist {
            refresh_active_autoscales(&mut candidate).map_err(ProjectError::Validation)?;
        }
        self.project = candidate;
        Ok(())
    }

    pub fn set_manual_source_metadata(
        &mut self,
        data_source_id: &str,
        recipe: ManualDataRecipe,
    ) -> Result<(), String> {
        let mut candidate = self.project.clone();
        let source = candidate
            .data_sources
            .iter_mut()
            .find(|source| source.id == data_source_id)
            .ok_or_else(|| format!("data source {data_source_id} is missing"))?;
        source.origin = DataSourceOrigin::Manual;
        source.manual_recipe = Some(recipe);
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn has_unmanaged_manual_sources(&self) -> bool {
        self.project.data_sources.iter().any(|source| {
            source.origin == DataSourceOrigin::Manual && source.managed_file.is_none()
        })
    }

    pub fn configure_manual_data_files(
        &mut self,
        directory: &Path,
        format: ManagedDataFormat,
    ) -> Result<(), String> {
        let mut candidate = self.project.clone();
        let mut reserved = BTreeSet::new();
        for source in &mut candidate.data_sources {
            if source.origin != DataSourceOrigin::Manual {
                continue;
            }
            let stem = safe_file_stem(&source.label, &source.id);
            let mut path = directory.join(format!("{stem}.{}", format.extension()));
            let mut suffix = 2;
            while path.exists() || !reserved.insert(path.clone()) {
                path = directory.join(format!("{stem}-{suffix}.{}", format.extension()));
                suffix += 1;
            }
            source.managed_file = Some(ManagedDataFile {
                path: path.to_string_lossy().into_owned(),
                format,
                fingerprint: None,
            });
        }
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn save_with_managed_manual_data(&self, path: &Path) -> Result<Self, ProjectError> {
        self.save_with_managed_manual_data_policy(path, false)
    }

    pub fn overwrite_managed_manual_data(&self, path: &Path) -> Result<Self, ProjectError> {
        self.save_with_managed_manual_data_policy(path, true)
    }

    fn save_with_managed_manual_data_policy(
        &self,
        path: &Path,
        overwrite_conflicts: bool,
    ) -> Result<Self, ProjectError> {
        let mut saved = self.clone();
        // Validate every managed target before writing any of them. This prevents
        // a later conflict from leaving an earlier data file partially updated.
        if !overwrite_conflicts {
            for source in &saved.project.data_sources {
                let Some(managed) = source.managed_file.as_ref() else {
                    continue;
                };
                if source.origin != DataSourceOrigin::Manual {
                    continue;
                }
                validate_managed_target(managed)?;
            }
        }
        for source in &mut saved.project.data_sources {
            let Some(managed) = source.managed_file.clone() else {
                continue;
            };
            if source.origin != DataSourceOrigin::Manual {
                continue;
            }
            let mut dataset = embedded_dataset(source)?;
            if managed.format != ManagedDataFormat::Csv && managed.format != ManagedDataFormat::Xlsx
            {
                stabilize_text_column_names(&mut dataset);
            }
            let columns = (0..dataset.columns.len()).collect::<Vec<_>>();
            let data_path = PathBuf::from(managed.path);
            if let Some(parent) = data_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            write_dataset_atomically(&data_path, &dataset, &columns)?;
            source
                .managed_file
                .as_mut()
                .expect("managed file was cloned above")
                .fingerprint = Some(fingerprint(&data_path)?);
        }
        saved.project.validate()?;
        save_project(path, &saved.project)?;
        Ok(saved)
    }

    pub fn export_manual_data_files(
        &self,
        directory: &Path,
        format: ManagedDataFormat,
    ) -> Result<usize, ProjectError> {
        let mut exported = 0;
        let mut reserved = BTreeSet::new();
        for source in &self.project.data_sources {
            if !matches!(
                source.origin,
                DataSourceOrigin::Manual | DataSourceOrigin::LegacyManual
            ) {
                continue;
            }
            let mut dataset = embedded_dataset(source)?;
            if format != ManagedDataFormat::Csv && format != ManagedDataFormat::Xlsx {
                stabilize_text_column_names(&mut dataset);
            }
            let stem = safe_file_stem(&source.label, &source.id);
            let mut path = directory.join(format!("{stem}.{}", format.extension()));
            let mut suffix = 2;
            while path.exists() || !reserved.insert(path.clone()) {
                path = directory.join(format!("{stem}-{suffix}.{}", format.extension()));
                suffix += 1;
            }
            std::fs::create_dir_all(directory)?;
            let columns = (0..dataset.columns.len()).collect::<Vec<_>>();
            write_dataset_atomically(&path, &dataset, &columns)?;
            exported += 1;
        }
        Ok(exported)
    }
}

fn write_dataset_atomically(
    path: &Path,
    dataset: &DataSet,
    columns: &[usize],
) -> Result<(), ProjectError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("data");
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("data");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(
        ".{stem}.instplot-{}-{nonce}.{extension}",
        std::process::id()
    ));
    if let Err(error) = save_retained_rows_selected_with_fits(&temporary, dataset, columns, &[]) {
        let _ = std::fs::remove_file(&temporary);
        return Err(ProjectError::Validation(error));
    }
    let bytes = match std::fs::read(&temporary) {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            return Err(ProjectError::Io(error));
        }
    };
    let result = AtomicFile::new(path, AllowOverwrite)
        .write(|file| file.write_all(&bytes))
        .map_err(|error| ProjectError::AtomicWrite(error.to_string()));
    let _ = std::fs::remove_file(temporary);
    result
}

fn validate_managed_target(managed: &ManagedDataFile) -> Result<(), ProjectError> {
    let data_path = PathBuf::from(&managed.path);
    match (&managed.fingerprint, data_path.exists()) {
        (Some(expected), true) if fingerprint(&data_path)? != *expected => {
            Err(ProjectError::Validation(format!(
                "managed manual data file changed outside Studio: {}. Use Save As to keep both versions",
                data_path.display()
            )))
        }
        (Some(_), false) => Err(ProjectError::Validation(format!(
            "managed manual data file is missing: {}. Use Save As to choose a new location",
            data_path.display()
        ))),
        (None, true) => Err(ProjectError::Validation(format!(
            "refusing to overwrite an unrelated file: {}. Choose another folder or file format",
            data_path.display()
        ))),
        _ => Ok(()),
    }
}

fn safe_file_stem(label: &str, fallback: &str) -> String {
    let stem = label
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect::<String>();
    if stem.is_empty() {
        fallback.to_owned()
    } else {
        stem
    }
}

fn embedded_dataset(source: &DataSourceRecord) -> Result<DataSet, ProjectError> {
    let DataSourcePayload::Embedded {
        columns,
        row_count,
        alive,
        ..
    } = &source.payload
    else {
        return Err(ProjectError::Validation(format!(
            "manual data source {} is not embedded",
            source.id
        )));
    };
    Ok(DataSet {
        source: PathBuf::from(format!("embedded://{}", source.id)),
        label: Some(source.label.clone()),
        kind: DataSetKind::Source,
        plot_id: source.id.clone(),
        fit_link: None,
        encoding: "UTF-8".to_owned(),
        separator: String::new(),
        columns: columns
            .iter()
            .map(|column| NumericColumn {
                name: column.name.clone(),
                values: column
                    .values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        if column.valid.get(index).copied().unwrap_or(true) {
                            *value
                        } else {
                            f64::NAN
                        }
                    })
                    .collect(),
            })
            .collect(),
        row_count: *row_count,
        alive: if alive.is_empty() {
            vec![true; *row_count]
        } else {
            alive.clone()
        },
    })
}

fn stabilize_text_column_names(dataset: &mut DataSet) {
    let mut used = BTreeSet::new();
    for (index, column) in dataset.columns.iter_mut().enumerate() {
        let base = column
            .name
            .chars()
            .map(|character| {
                if character.is_alphanumeric() || character == '_' {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let base = base.trim_matches('_');
        let base = if base.is_empty() {
            format!("column_{}", index + 1)
        } else {
            base.to_owned()
        };
        let mut candidate = base.clone();
        let mut suffix = 2;
        while !used.insert(candidate.clone()) {
            candidate = format!("{base}_{suffix}");
            suffix += 1;
        }
        column.name = candidate;
    }
}
