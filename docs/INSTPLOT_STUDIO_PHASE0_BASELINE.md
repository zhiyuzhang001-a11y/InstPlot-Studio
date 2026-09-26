# InstPlot Studio Phase 0 行为基线与依赖地图

状态：完成
完成日期：2026-09-25
对应计划：`docs/INSTPLOT_STUDIO_MODULARIZATION_PLAN.md` Phase 0
范围：只建立架构事实、行为保护和阶段门槛；未移动生产代码。

## 1. Phase 0 结论

当前代码可以进入 Phase 1 的机械拆分，但必须保留下列约束：

- 普通数据导入必须继续以嵌入数据保存，项目不能依赖原始 CSV/TXT 才能恢复。
- `FigureDocument` 是可持久化图形状态的唯一权威；`StudioSession` 是可重建缓存。
- 导入、删除、清空、重新导入和打开项目目前仍由 `StudioApp` 跨多个状态对象协调，这是 Phase 2 的主要治理目标。
- 屏幕预览、PDF 和 PNG 已通过 `resolve_document` 共享 resolved display list；这个单一管线必须在拆分中保留。
- 当前测试基线通过，可以作为后续各阶段的回归门槛。

## 2. 当前 workspace 与依赖地图

当前正式 workspace member 只有：

```text
workspace
  ├─ apps/instplot-studio
  └─ crates/instplot-layout
```

生产应用的关键依赖方向为：

```text
instplot-studio binary (main.rs)
  ├─ instplot-studio library
  │    ├─ document / editing / project / session
  │    ├─ semantic / palette / publication / handoff
  │    ├─ render / preview / export
  │    └─ instplot-layout
  ├─ instplot-core + instplot-io
  │    └─ 固定到 InstPlot Lite commit 80ad374...
  └─ production-used prototype crates
       ├─ export-backend-spike
       ├─ studio-render-spike
       ├─ text-shaping-spike
       └─ ui-shell-spike
```

已确认的边界问题：

- `main.rs` 仍同时包含应用状态、文件对话框、导入事务、窗口、画布交互和大量 UI 测试。
- `document.rs` 同时包含领域操作、自动范围、项目数据同步、布局转换和显示列表输入构建。
- `project.rs` 同时包含 schema、验证、迁移、fixture、原子文件 IO 和来源状态检查。
- `label_input.rs`、`manual_data.rs`、`text_input.rs`、`ui_text.rs`、`workspace.rs` 是 binary 内部模块，当前不能被另一个应用直接依赖。
- 多个 `spike` crate 虽不属于正式 workspace member，却已经位于生产构建路径。

## 3. 状态所有权与实际写入点

### 3.1 持久化图形状态

当前权威对象是 `FigureDocument`，内部私有持有 `ProjectDocument`。它拥有：

- 画布、坐标轴、标签、系列、图例和标注；
- 数据源记录、嵌入数值、列绑定和来源信息；
- palette、typography、导出偏好和 provenance。

主要写入入口位于 `document.rs`。`EditCommand` 将 UI 意图映射到这些入口，`EditHistory` 通过候选文档、验证和提交提供撤销/重做边界。

### 3.2 非权威数据会话

`StudioSession` 持有 `Vec<DataSet>`，用于导入、侧栏列选择和运行时访问。它可以由项目的 `DataSourceRecord` 重建，不应成为项目内容的唯一来源。

当前 `StudioSession::import_data_file` 负责：

- 调用 Lite 的共享 `instplot_io::read_data_file`；
- 修正特定数值 CSV 表头；
- 处理 source/fit 身份、重复 ID 和重新导入；
- 在失败时避免改变现有 session。

### 3.3 工作区状态

`WorkspaceState` 只负责项目来源、项目路径、显示名称和示例图是否应被首次真实导入替换。它不应保存数据或图形规则。

### 3.4 UI 与派生状态

`StudioApp` 当前还直接持有：

- selection、窗口开关、输入草稿和拖拽状态；
- `StudioSession`、`FigureDocument`、`WorkspaceState` 和 `EditHistory`；
- 派生的 `ResolvedFigure`、publication report、消息和状态栏内容。

`ResolvedFigure` 和 publication report 必须保持为可重建派生状态，不进入项目 schema。

## 4. 高风险工作流现状

### 4.1 数据导入

当前实际流程：

```text
StudioApp::load_data_paths
  → clone candidate session/document
  → StudioSession::import_data_file
  → document_with_imported_datasets
  → resolve preview 验证
  → commit session/document/resolved
  → 更新 workspace、selection、publication 和消息
```

单个文件使用 trial state，失败不会提交半完成状态；批量导入允许有效文件成功并汇总无效文件错误。完整事务仍在 `main.rs`，是 Phase 2 提取 `AppAction`/controller orchestration 的直接依据。

### 4.2 删除、清空与重新导入

删除通过 `EditCommand::DeleteDataSource(s)` 进入 `FigureDocument`，清理关联 artist、axes artist IDs 和 legend entries。删除最后一个来源时会重置空画布的数据相关坐标状态。

UI 随后还需要从文档重建 session，并同步 selection、输入框、resolved preview 和 publication report。当前这些后续操作尚未形成单一应用事务。

已存在并通过的关键回归测试：

- `clearing_all_data_is_undoable_and_leaves_a_valid_empty_figure`
- `importing_after_clear_all_rebuilds_visible_series_and_autoscales`
- `deleting_the_only_file_then_reimporting_it_fits_every_value`
- `data_source_deletion_requires_explicit_cascade_and_cleans_dependencies`

### 4.3 项目打开与保存

保存通过 `FigureDocument::save` → `save_project`，使用临时文件原子替换，并为已有合法项目保留 backup。打开失败时可从 backup 恢复为只读来源。

打开成功后，`StudioApp` 仍负责依次替换 document、session、history、resolved、selection、workspace 和 diagnostics；这同样属于 Phase 2 的事务边界。

### 4.4 渲染与导出

当前共用路径为：

```text
FigureDocument
  → layout_figure
  → export_backend_spike::resolve
  → ResolvedFigure { layout, display }
  ├─ EguiPreviewAdapter
  ├─ PDF backend
  └─ PNG raster backend
```

PDF/PNG 保存已使用原子写入。当前后端共享 resolved display list 是必须保留的正确基础；Phase 6 只应正式化边界，不能重新产生多套解析或布局逻辑。

## 5. 数据所有权与项目可复现性审计

当前 schema 版本为 6，`DataSourcePayload` 支持两种形式：

- `Embedded`：保存列名、数值、逐单元有效性、行启用状态和摘要。
- `External`：只保存路径和文件指纹。

当前普通 CSV/TXT/XLSX 导入经 `FigureDocument::from_datasets` 或 `sync_datasets` 后使用 `Embedded`。`origin_path` 仅保存来源信息，不参与恢复数值。新增契约测试已证明：

1. 导入普通 CSV；
2. 保存 `.instplot`；
3. 删除原始 CSV；
4. 重新打开项目；
5. 数据列、启用状态和渲染结果仍可完整恢复。

`External` 当前没有生产创建调用方，只保留在 schema/API 和兼容测试中。若打开的历史项目包含 External 来源，文件缺失或改变时会警告且不载入 session。这不满足长期的完全可复现目标，因此 Phase 3 必须决定：

- 打开后显式嵌入并迁移；或
- 采用受项目管理、带完整性校验的打包资源。

在作出兼容方案前不得直接删除 `External` variant。

## 6. 自动行为基线

### 6.1 测试门槛

2026-09-25 实际执行：

```text
cargo test --locked --workspace
结果：211 passed, 0 failed, 1 ignored

cargo clippy --version
结果：clippy 0.1.98，Rust 1.98 toolchain

cargo clippy --locked --all-targets -- -D warnings
结果：通过，0 warnings

cargo fmt --all -- --check
结果：通过
```

唯一 ignored 测试是显式请求输出临时标签视觉检查 PDF 的人工辅助测试，不属于失败或跳过的产品行为。

### 6.2 新增持久化契约

新增：

`apps/instplot-studio/tests/phase0_persistence_contract.rs`

它从公开 API 验证普通导入项目在源文件删除后仍可恢复并解析为非空 display list，避免只在 `main.rs` 私有测试中验证实现细节。

### 6.3 P0 代表性产物

执行：

```text
cargo run --locked -p instplot-studio --example p0_baseline -- OUTPUT_DIRECTORY
```

五组代表性输入全部生成 `.instplot`、handoff、PDF 和 PNG：

- single-source：1 dataset，3 rows；
- source-fit：2 datasets，6 rows；
- multi-source：2 datasets，8 rows；
- missing-value：1 dataset，4 rows；
- disabled-row：1 dataset，5 rows，其中 4 rows enabled。

本阶段只记录结构和成功条件，不把 PDF/PNG 字节长度作为跨平台硬断言。后续基线优先比较布局数值和 display list，最终图像使用容差。

## 7. Phase 1 不得越过的边界

Phase 1 只做机械拆分，必须遵守：

- 不改变项目 schema、导入行为、界面布局或用户可见文字。
- 不引入最终 crate 拆分；先形成 binary 内部 module。
- 不趁移动代码重写 `StudioApp` 状态模型。
- 每次移动后运行 workspace tests、Clippy 和格式检查。
- `load_data_paths` 等跨状态事务可以整体移动，但其内部行为留到 Phase 2 再重构。
- 私有 UI 测试随所属模块移动，测试断言不得削弱。

## 8. 已知风险登记

- P0-R1：`StudioApp` 同时持有权威状态、派生状态和 UI 临时状态，跨层写入仍多。
- P0-R2：完整导入事务位于 `main.rs`，库 API 只有较低层的 session/document 能力。
- P0-R3：删除和打开项目后的状态同步依赖 UI 层按正确顺序执行。
- P0-R4：`External` schema 兼容路径无法在源文件缺失时恢复数据。
- P0-R5：生产渲染/导出依赖多个被 workspace 排除且带 `spike` 名称的 crate。
- P0-R6：大量关键应用行为测试位于 binary 私有 tests，机械拆分时容易误删或降低覆盖。
- P0-R7：标签和手动数据规则仍是 binary-only module，第二个 consumer 尚不能直接使用。

这些风险不是 Phase 0 的失败项，而是后续阶段的输入。任何阶段若使上述风险扩大，应停止进入下一阶段。

## 9. Phase 0 完成判定

- [x] 当前模块和外部依赖已记录。
- [x] 权威、缓存、工作区、UI 和派生状态已区分。
- [x] 导入、删除、打开、保存、渲染和导出路径已定位。
- [x] 普通导入的离线可复现性已有公开 API 契约测试。
- [x] 项目 External 兼容风险已明确登记。
- [x] P0 代表性产物已成功生成。
- [x] workspace tests、Clippy 和格式检查全部通过。

结论：Phase 0 通过，可以开始 Phase 1；Phase 1 仅做可验证的机械拆分。
