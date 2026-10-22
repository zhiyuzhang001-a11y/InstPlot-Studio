//! A single-source edit, independent of app launch and transport UI.
use crate::lite_edit_protocol::{Column, Transfer};
use crate::{
    DataBinding, DataSourceKind, DataSourceOrigin, DataSourcePayload, DataSourceRecord,
    FigureDocument,
};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct LiteSourceUpdate {
    pub artist_id: String,
    pub binding_before: DataBinding,
    pub result: Transfer,
}

pub fn source_revision(source: &DataSourceRecord) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(source).map_err(|e| e.to_string())?)
    ))
}

pub fn make_request(
    document: &FigureDocument,
    artist_id: &str,
    session_id: String,
) -> Result<(Transfer, DataBinding), String> {
    let binding = document
        .logical_series()
        .into_iter()
        .find(|s| s.id == artist_id)
        .and_then(|s| s.binding)
        .ok_or("请先选择一条原始数据曲线")?;
    let source = document
        .project()
        .data_sources
        .iter()
        .find(|s| s.id == binding.data_source_id)
        .ok_or("数据组已不存在")?;
    if source.kind != DataSourceKind::Source
        || source.origin != DataSourceOrigin::Imported
        || source.managed_file.is_some()
    {
        return Err("本功能只编辑导入的原始数据组；拟合与手动测量数据请在原入口编辑".to_owned());
    }
    let DataSourcePayload::Embedded {
        columns,
        alive,
        row_count,
        ..
    } = &source.payload
    else {
        return Err("请先将数据嵌入当前项目".to_owned());
    };
    let request = Transfer {
        schema_version: 1,
        session_id,
        target_id: source.id.clone(),
        base_sha256: source_revision(source)?,
        label: source.label.clone(),
        x_column: binding.x_column.clone(),
        y_column: binding.y_column.clone(),
        columns: columns
            .iter()
            .map(|c| Column {
                name: c.name.clone(),
                values: c
                    .values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (c.valid.is_empty() || c.valid[i]).then_some(*v))
                    .collect(),
            })
            .collect(),
        alive: if alive.is_empty() {
            vec![true; *row_count]
        } else {
            alive.clone()
        },
    };
    request.validate()?;
    Ok((request, binding))
}
