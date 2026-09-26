use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use instplot_core::{DataSet, DataSetKind, NumericColumn, generated_plot_id};
use instplot_io::{ImportError, read_data_file};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataDiagnostic {
    pub code: &'static str,
    pub line_number: Option<usize>,
    pub reason: String,
}

impl DataDiagnostic {
    pub fn into_import_error(self) -> ImportError {
        ImportError {
            code: self.code,
            line_number: self.line_number,
            reason: self.reason,
        }
    }
}

impl std::fmt::Display for DataDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line_number {
            Some(line) => write!(formatter, "{} (line {line}): {}", self.code, self.reason),
            None => write!(formatter, "{}: {}", self.code, self.reason),
        }
    }
}

impl From<ImportError> for DataDiagnostic {
    fn from(error: ImportError) -> Self {
        Self {
            code: error.code,
            line_number: error.line_number,
            reason: error.reason,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportOutcome {
    pub read: usize,
    pub added: usize,
    pub replaced: usize,
    pub skipped_unlinked_fits: usize,
}

/// GUI-independent entry point for every source of numeric plotting data.
pub struct DataImporter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataFormatCapability {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub supports_multiple_datasets: bool,
    pub supports_missing_values: bool,
}

pub const DATA_FORMAT_CAPABILITIES: &[DataFormatCapability] = &[
    DataFormatCapability {
        name: "Delimited text",
        extensions: &["txt", "csv", "dat", "tsv"],
        supports_multiple_datasets: true,
        supports_missing_values: true,
    },
    DataFormatCapability {
        name: "Excel workbook",
        extensions: &["xlsx", "xls"],
        supports_multiple_datasets: true,
        supports_missing_values: true,
    },
];

impl DataImporter {
    pub fn read_file(path: &Path) -> Result<Vec<DataSet>, DataDiagnostic> {
        let mut datasets = read_data_file(path).map_err(DataDiagnostic::from)?;
        normalize_numeric_csv_header(path, &mut datasets);
        Ok(datasets)
    }

    pub fn import_file(
        existing: &[DataSet],
        path: &Path,
    ) -> Result<(Vec<DataSet>, ImportOutcome), DataDiagnostic> {
        let imported = Self::read_file(path)?;
        let skipped_unlinked_fits = imported
            .iter()
            .filter(|dataset| {
                dataset.kind == DataSetKind::Fit
                    && dataset
                        .fit_link
                        .as_ref()
                        .is_some_and(|link| link.parent_dataset_id.is_none())
            })
            .count();
        let imported = imported
            .into_iter()
            .filter(|dataset| {
                dataset.kind != DataSetKind::Fit
                    || dataset
                        .fit_link
                        .as_ref()
                        .is_none_or(|link| link.parent_dataset_id.is_some())
            })
            .collect::<Vec<_>>();
        if imported.is_empty() && skipped_unlinked_fits > 0 {
            return Err(DataDiagnostic {
                code: "unsupported_aggregate_fit",
                line_number: None,
                reason: "只有多来源拟合而没有可导入的原始数据；该文件未记录拟合对应哪些来源，Studio 无法安全恢复关联".to_owned(),
            });
        }
        let imported_ids = imported
            .iter()
            .map(|dataset| dataset.plot_id.as_str())
            .collect::<BTreeSet<_>>();
        if let Some(stale) = existing
            .iter()
            .filter(|dataset| dataset.source == path)
            .find(|dataset| !imported_ids.contains(dataset.plot_id.as_str()))
        {
            return Err(DataDiagnostic {
                code: "changed_dataset_sections",
                line_number: None,
                reason: format!(
                    "原文件中的数据区“{}”已消失或变为无法关联的拟合；为防止旧曲线残留，未更新当前图。请新建图后重新导入该文件",
                    stale.display_name()
                ),
            });
        }
        if let Some(dataset) = imported.iter().find(|dataset| {
            existing.iter().any(|current| {
                current.plot_id == dataset.plot_id && current.source != dataset.source
            })
        }) {
            return Err(DataDiagnostic {
                code: "duplicate_dataset_id",
                line_number: None,
                reason: format!(
                    "数据集 ID“{}”已由其他文件使用；为防止曲线或拟合关联错位，未导入",
                    dataset.plot_id
                ),
            });
        }
        let mut datasets = existing.to_vec();
        let mut outcome = ImportOutcome {
            read: imported.len(),
            skipped_unlinked_fits,
            ..ImportOutcome::default()
        };
        for dataset in imported {
            if let Some(current) = datasets
                .iter_mut()
                .find(|current| current.plot_id == dataset.plot_id)
            {
                *current = dataset;
                outcome.replaced += 1;
            } else {
                datasets.push(dataset);
                outcome.added += 1;
            }
        }
        Ok((datasets, outcome))
    }

    pub fn import_manual(input: &ManualDataInput) -> Result<ParsedManualData, DataDiagnostic> {
        parse_manual_data(input).map_err(|reason| DataDiagnostic {
            code: "manual-input",
            line_number: None,
            reason,
        })
    }
}

/// Lite's text importer can mistake a CSV header for a data row when two or more
/// column names are numeric. Correct that narrow case without rewriting the file.
pub(crate) fn normalize_numeric_csv_header(path: &Path, datasets: &mut [DataSet]) {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
    {
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Some(first_line) = text
        .lines()
        .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
    else {
        return;
    };
    if first_line.contains('"') {
        return;
    }
    let headers = first_line.split(',').map(str::trim).collect::<Vec<_>>();
    if headers.len() < 2
        || headers.iter().any(|header| header.is_empty())
        || headers
            .iter()
            .filter(|header| header.parse::<f64>().is_ok_and(f64::is_finite))
            .count()
            < 2
    {
        return;
    }
    for dataset in datasets {
        if dataset.source != path
            || dataset.kind != DataSetKind::Source
            || dataset.columns.len() != headers.len()
            || dataset.row_count < 2
            || dataset.alive.len() != dataset.row_count
        {
            continue;
        }
        let header_was_parsed_as_data =
            headers
                .iter()
                .zip(&dataset.columns)
                .all(|(header, column)| {
                    let Some(first_value) = column.values.first() else {
                        return false;
                    };
                    match header.parse::<f64>() {
                        Ok(number) if number.is_finite() => *first_value == number,
                        _ => first_value.is_nan(),
                    }
                });
        if !header_was_parsed_as_data {
            continue;
        }
        for (column, header) in dataset.columns.iter_mut().zip(&headers) {
            column.name = (*header).to_owned();
            column.values.remove(0);
        }
        dataset.alive.remove(0);
        dataset.row_count -= 1;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorStatistic {
    StandardDeviation,
    StandardError,
}

#[derive(Clone, Debug)]
pub struct ManualAxisInput {
    pub name: String,
    pub measurements: Vec<String>,
}

impl ManualAxisInput {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            measurements: vec![String::new()],
        }
    }
}

#[derive(Clone, Debug)]
pub struct ManualYInput {
    pub axis: ManualAxisInput,
    pub x_index: usize,
}

#[derive(Clone, Debug)]
pub struct ManualDataInput {
    pub source_name: String,
    pub x_inputs: Vec<ManualAxisInput>,
    pub y_inputs: Vec<ManualYInput>,
    pub error_statistic: ErrorStatistic,
}

impl Default for ManualDataInput {
    fn default() -> Self {
        Self {
            source_name: "手动数据 1".to_owned(),
            x_inputs: vec![ManualAxisInput::new("X")],
            y_inputs: vec![ManualYInput {
                axis: ManualAxisInput::new("Y"),
                x_index: 0,
            }],
            error_statistic: ErrorStatistic::StandardDeviation,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParsedManualSeries {
    pub label: String,
    pub x_column: String,
    pub y_column: String,
    pub x_error_column: Option<String>,
    pub y_error_column: Option<String>,
}

#[derive(Debug)]
pub struct ParsedManualData {
    pub dataset: DataSet,
    pub series: Vec<ParsedManualSeries>,
}

#[derive(Clone, Debug)]
struct ParsedAxis {
    name: String,
    mean: Vec<f64>,
    error: Option<Vec<f64>>,
    raw: Vec<Vec<f64>>,
}

pub fn parse_manual_data(input: &ManualDataInput) -> Result<ParsedManualData, String> {
    if input.x_inputs.is_empty() {
        return Err("请至少保留一组 X 数据".to_owned());
    }
    if input.y_inputs.is_empty() {
        return Err("请至少保留一组 Y 数据".to_owned());
    }

    let mut used_names = BTreeSet::new();
    let mut parsed_x = Vec::with_capacity(input.x_inputs.len());
    for (index, axis) in input.x_inputs.iter().enumerate() {
        parsed_x.push(parse_axis(
            axis,
            &format!("X{}", index + 1),
            input.error_statistic,
            &mut used_names,
        )?);
    }
    let mut parsed_y = Vec::with_capacity(input.y_inputs.len());
    for (index, input_y) in input.y_inputs.iter().enumerate() {
        if input_y.x_index >= parsed_x.len() {
            return Err(format!("Y{} 没有对应的 X 数据", index + 1));
        }
        let parsed = parse_axis(
            &input_y.axis,
            &format!("Y{}", index + 1),
            input.error_statistic,
            &mut used_names,
        )?;
        let x_len = parsed_x[input_y.x_index].mean.len();
        if parsed.mean.len() != x_len {
            return Err(format!(
                "{} 有 {} 个数据，配对的 {} 有 {} 个数据",
                parsed.name,
                parsed.mean.len(),
                parsed_x[input_y.x_index].name,
                x_len
            ));
        }
        parsed_y.push(parsed);
    }

    let row_count = parsed_x
        .iter()
        .chain(parsed_y.iter())
        .map(|axis| axis.mean.len())
        .max()
        .unwrap_or(0);
    let mut columns = Vec::new();
    for axis in parsed_x.iter().chain(parsed_y.iter()) {
        columns.push(NumericColumn {
            name: axis.name.clone(),
            values: padded(&axis.mean, row_count),
        });
        if let Some(error) = &axis.error {
            columns.push(NumericColumn {
                name: error_name(&axis.name, input.error_statistic),
                values: padded(error, row_count),
            });
        }
        if axis.raw.len() > 1 {
            for (index, values) in axis.raw.iter().enumerate() {
                columns.push(NumericColumn {
                    name: format!("{} · 测量 {}", axis.name, index + 1),
                    values: padded(values, row_count),
                });
            }
        }
    }

    let source_name = nonempty(&input.source_name, "手动数据");
    let source = PathBuf::from(format!("manual-data/{source_name}.txt"));
    let plot_id = generated_plot_id(&source, &columns);
    let mut series: Vec<ParsedManualSeries> = input
        .y_inputs
        .iter()
        .enumerate()
        .map(|(index, y)| {
            let x = &parsed_x[y.x_index];
            let y = &parsed_y[index];
            ParsedManualSeries {
                label: y.name.clone(),
                x_column: x.name.clone(),
                y_column: y.name.clone(),
                x_error_column: x
                    .error
                    .as_ref()
                    .map(|_| error_name(&x.name, input.error_statistic)),
                y_error_column: y
                    .error
                    .as_ref()
                    .map(|_| error_name(&y.name, input.error_statistic)),
            }
        })
        .collect();
    for specification in &mut series {
        if specification.x_error_column.is_some() && specification.y_error_column.is_none() {
            let name = unique_name(
                &format!("{} · zero error", specification.y_column),
                &mut used_names,
            );
            columns.push(NumericColumn {
                name: name.clone(),
                values: vec![0.0; row_count],
            });
            specification.y_error_column = Some(name);
        }
    }

    Ok(ParsedManualData {
        dataset: DataSet {
            source,
            label: Some(source_name),
            kind: DataSetKind::Source,
            plot_id,
            fit_link: None,
            encoding: "UTF-8".to_owned(),
            separator: "manual-columns".to_owned(),
            columns,
            row_count,
            alive: vec![true; row_count],
        },
        series,
    })
}

fn parse_axis(
    input: &ManualAxisInput,
    fallback: &str,
    statistic: ErrorStatistic,
    used_names: &mut BTreeSet<String>,
) -> Result<ParsedAxis, String> {
    if input.measurements.is_empty() {
        return Err(format!("{fallback} 没有数据输入框"));
    }
    let requested = nonempty(&input.name, fallback);
    let name = unique_name(&requested, used_names);
    let mut raw = Vec::with_capacity(input.measurements.len());
    for (index, text) in input.measurements.iter().enumerate() {
        raw.push(
            parse_numeric_column(text)
                .map_err(|error| format!("{name} 的测量 {}：{error}", index + 1))?,
        );
    }
    let expected = raw[0].len();
    if let Some((index, actual)) = raw
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, values)| (values.len() != expected).then_some((index, values.len())))
    {
        return Err(format!(
            "{name} 的测量 {} 有 {actual} 个数据，测量 1 有 {expected} 个数据",
            index + 1
        ));
    }
    let (mean, error) = summarize_measurements(&raw, statistic);
    Ok(ParsedAxis {
        name,
        mean,
        error,
        raw,
    })
}

pub fn parse_numeric_column(text: &str) -> Result<Vec<f64>, String> {
    let mut values = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        for (cell_index, cell) in line
            .split(|character: char| {
                character == '\t'
                    || character == ','
                    || character == ';'
                    || character.is_whitespace()
            })
            .filter(|cell| !cell.trim().is_empty())
            .enumerate()
        {
            let value = cell.trim().trim_matches('"');
            values.push(value.parse::<f64>().map_err(|_| {
                format!(
                    "第 {} 行第 {} 项不是数值：{value}",
                    line_index + 1,
                    cell_index + 1
                )
            })?);
        }
    }
    if values.is_empty() {
        return Err("请输入至少一个数值".to_owned());
    }
    Ok(values)
}

fn summarize_measurements(
    measurements: &[Vec<f64>],
    statistic: ErrorStatistic,
) -> (Vec<f64>, Option<Vec<f64>>) {
    if measurements.len() == 1 {
        return (measurements[0].clone(), None);
    }
    let mut means = Vec::with_capacity(measurements[0].len());
    let mut errors = Vec::with_capacity(measurements[0].len());
    for row in 0..measurements[0].len() {
        let samples = measurements
            .iter()
            .map(|measurement| measurement[row])
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        if samples.is_empty() {
            means.push(f64::NAN);
            errors.push(f64::NAN);
            continue;
        }
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        means.push(mean);
        if samples.len() < 2 {
            errors.push(f64::NAN);
            continue;
        }
        let variance = samples
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (samples.len() - 1) as f64;
        let sd = variance.sqrt();
        errors.push(match statistic {
            ErrorStatistic::StandardDeviation => sd,
            ErrorStatistic::StandardError => sd / (samples.len() as f64).sqrt(),
        });
    }
    (means, Some(errors))
}

fn error_name(name: &str, statistic: ErrorStatistic) -> String {
    let suffix = match statistic {
        ErrorStatistic::StandardDeviation => "SD",
        ErrorStatistic::StandardError => "SEM",
    };
    format!("{name} · {suffix}")
}

fn unique_name(requested: &str, used: &mut BTreeSet<String>) -> String {
    if used.insert(requested.to_owned()) {
        return requested.to_owned();
    }
    for suffix in 2.. {
        let candidate = format!("{requested} {suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
}

fn padded(values: &[f64], length: usize) -> Vec<f64> {
    values
        .iter()
        .copied()
        .chain(std::iter::repeat(f64::NAN))
        .take(length)
        .collect()
}

fn nonempty(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() { fallback } else { value }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(name: &str, measurements: &[&str]) -> ManualAxisInput {
        ManualAxisInput {
            name: name.to_owned(),
            measurements: measurements
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        }
    }

    #[test]
    fn separate_columns_accept_common_paste_separators() {
        for text in ["1\t2\n3", "1,2,3", "1;2;3", "1 2 3"] {
            assert_eq!(parse_numeric_column(text).unwrap(), vec![1.0, 2.0, 3.0]);
        }
    }

    #[test]
    fn multiple_x_and_y_groups_keep_pairings_and_raw_measurements() {
        let parsed = parse_manual_data(&ManualDataInput {
            source_name: "Trial".to_owned(),
            x_inputs: vec![axis("Field A", &["0 1"]), axis("Field B", &["10 20 30"])],
            y_inputs: vec![
                ManualYInput {
                    axis: axis("Signal A", &["2 4", "4 8"]),
                    x_index: 0,
                },
                ManualYInput {
                    axis: axis("Signal B", &["5 6 7"]),
                    x_index: 1,
                },
            ],
            error_statistic: ErrorStatistic::StandardDeviation,
        })
        .unwrap();
        assert_eq!(parsed.series.len(), 2);
        assert_eq!(parsed.series[0].x_column, "Field A");
        assert_eq!(parsed.series[1].x_column, "Field B");
        assert_eq!(
            parsed.series[0].y_error_column.as_deref(),
            Some("Signal A · SD")
        );
        let mean = parsed
            .dataset
            .columns
            .iter()
            .find(|column| column.name == "Signal A")
            .unwrap();
        assert_eq!(&mean.values[..2], &[3.0, 6.0]);
        assert!(
            parsed
                .dataset
                .columns
                .iter()
                .any(|column| column.name == "Signal A · 测量 1")
        );
    }

    #[test]
    fn repeated_x_and_y_generate_both_error_columns() {
        let parsed = parse_manual_data(&ManualDataInput {
            source_name: "Errors".to_owned(),
            x_inputs: vec![axis("X", &["1 2", "3 4"])],
            y_inputs: vec![ManualYInput {
                axis: axis("Y", &["5 7", "9 11"]),
                x_index: 0,
            }],
            error_statistic: ErrorStatistic::StandardDeviation,
        })
        .unwrap();
        assert_eq!(parsed.series[0].x_error_column.as_deref(), Some("X · SD"));
        assert_eq!(parsed.series[0].y_error_column.as_deref(), Some("Y · SD"));
    }

    #[test]
    fn standard_error_is_sample_sd_divided_by_square_root_of_count() {
        let measurements = vec![vec![2.0, 4.0], vec![4.0, 6.0], vec![6.0, 8.0]];
        let (sd_mean, sd) =
            summarize_measurements(&measurements, ErrorStatistic::StandardDeviation);
        let (sem_mean, sem) = summarize_measurements(&measurements, ErrorStatistic::StandardError);
        assert_eq!(sd_mean, vec![4.0, 6.0]);
        assert_eq!(sem_mean, sd_mean);
        for value in sd.unwrap() {
            assert!((value - 2.0).abs() < 1.0e-12);
        }
        let expected_sem = 2.0 / 3.0_f64.sqrt();
        for value in sem.unwrap() {
            assert!((value - expected_sem).abs() < 1.0e-12);
        }
    }

    #[test]
    fn mismatched_pair_lengths_name_the_affected_groups() {
        let error = parse_manual_data(&ManualDataInput {
            source_name: String::new(),
            x_inputs: vec![axis("Time", &["1 2 3"])],
            y_inputs: vec![ManualYInput {
                axis: axis("Signal", &["4 5"]),
                x_index: 0,
            }],
            error_statistic: ErrorStatistic::StandardDeviation,
        })
        .unwrap_err();
        assert!(error.contains("Signal"));
        assert!(error.contains("Time"));
    }
}
