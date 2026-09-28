# InstPlot Studio 新功能后整合 Phase 0 基线

> 状态：`COMPLETED`  
> 日期：2026-09-28  
> 对应计划：`docs/INSTPLOT_STUDIO_POST_FEATURE_CODE_CONSOLIDATION_PLAN.md`  
> 基线分支：`codex/dual-axes`  
> 基线 HEAD：`6aa97c3ad071a1aea0f447a184e7767e1a271efc`

## 1. 恢复基线

仓库外恢复包目录名：`InstPlot-Consolidation-Recovery-20260928-162441`。

包含：

- `tracked.patch`：`git diff --binary HEAD`；
- `status-v2.z`：NUL 分隔的 `git status --porcelain=v2 -z`；
- `untracked.tar.gz`：所有非忽略未跟踪文件；
- `untracked.sha256`：未跟踪文件逐文件 SHA256；
- `baseline.txt`：分支、HEAD、Rust/Cargo/Clippy 与安装 build。

已在从上述 HEAD 创建的临时 detached worktree 中完成：

1. `git apply --check tracked.patch`；
2. 实际应用 tracked patch；
3. 展开未跟踪归档；
4. 逐文件复算 SHA256；
5. 恢复后的工作区变动项数量与源工作区一致，均为 42。

制定时工具链：Rust/Cargo 1.98.0、Clippy 0.1.98；Spotlight build：`202609281603`。

## 2. 已知基线门禁问题及处理

### 2.1 Repository hygiene

历史 schema 8 fixture 的 `origin_path` 含本机绝对路径，导致卫生脚本假失败。现已改为仓库相对来源路径，迁移测试仍验证嵌入数据、artist、轴和 schema 迁移结果。

### 2.2 B5 Studio validator

旧验证器要求 `--open-handoff` 与旧英文文案同时位于 `main.rs`，与已经完成的模块化冲突。现改为运行两个行为测试：

- startup 参数解析必须产生 `StartupCommand::OpenHandoff`；
- `Text::OpenLite` 的中英文资源必须返回当前正式文案。

验证器不再依赖实现位于特定源文件。

## 3. StudioApp 临时状态所有权

| 状态组 | 当前权威状态 | 不可改变的语义 |
|---|---|---|
| 文档与数据 | `session`, `document`, `resolved`, `publication_report` | 文档修改经命令/事务提交，resolved 为派生结果 |
| 选择 | `selected_series`, `selected_canvas_node`, `selected_canvas_role`, `selection_candidates`, `selected_dataset` | 对象删除或数据移除后清除陈旧选择 |
| 列绑定 | `binding_x`, `binding_y`, `binding_error` | 当前数据组的编辑选择，不直接替代文档绑定 |
| 确认事务 | `pending_data_removal`, `pending_managed_save_conflict`, `pending_action`, `pending_export`, `allow_close` | 取消不得半提交，确认后一次性执行 |
| 延迟输入 | `label_inputs`, `numeric_inputs`, `x_fixed_ticks`, `y_fixed_ticks` | 暂时空值允许存在，确认/失焦时才提交 |
| 画布 | `canvas_zoom`, `canvas_scroll`, `last_canvas_figure_center`, `hover_data_coordinates`, `trackpad_scroll_active` | 不持久化，不影响正式导出几何 |
| 工具窗口 | `show_*`, `focus_*`, `axis_visibility_identity`, `context_editor_*` | 重复请求置顶，关闭只影响对应窗口 |
| 拖拽与工具 | `active_artist_drag`, `drawing_tool`, `tool_draft`, `reference_draft` | 连续拖拽形成正确 Undo group，取消草稿不创建对象 |
| 手动数据 | `manual_data`, `focus_manual_data` | 保存后成为正式数据源；编辑模式与只读状态保持 |
| 批量样式 | `marker_size_for_all`, `marker_interval_for_all`, `marker_fill_for_all` | 只控制提交范围，不成为项目数据 |
| 消息与状态 | `messages`, `status` | 诊断与短暂提示不进入项目 |

## 4. 工具窗口生命周期矩阵

| 窗口 | 类型 | 稳定 ID / key | open/focus 状态 | 关闭与草稿规则 |
|---|---|---|---|---|
| 配色方案 | singleton tool viewport | `palette-window`, `palette-viewport` | `show_palette`, `focus_palette` | 再次点击置顶；关闭只清 open |
| 出版规范检查 | singleton tool viewport | `publication-check-window`, `publication-check-viewport` | `show_inspector`, `focus_inspector` | 再次点击置顶；关闭不修改图 |
| 坐标轴显示 | singleton tool viewport | `axis-visibility-window`, `axis-visibility-viewport` | `show_axis_visibility`, `focus_axis_visibility` | 保留当前轴选择；关闭不提交额外动作 |
| 录入数据 | singleton stateful tool viewport | `manual-data-window`, `manual-data-viewport` | `manual_data.open`, `focus_manual_data` | 草稿、只读/编辑状态按现有 `ManualDataState` 规则处理 |
| 参考线草稿 | transient draft viewport | `reference-draft-window`, `reference-draft-viewport` | `reference_draft`, `focus_reference_draft` | 添加后提交；取消或拒绝后清除并结束工具回合 |
| 对象编辑器 | identity-based multi-instance | `context-object-editor-{selected_id}-{role}` | `context_editor_targets`, `context_editor_focus_target` | 对象删除时对应窗口关闭；再次双击同对象置顶 |
| 对象选择器 | embedded chooser | `context-object-chooser` | selection candidates | 选择后打开目标编辑器；不属于独立 viewport host |
| 未保存确认 | modal/confirmation | 标题资源 | `pending_action`, `allow_close` | 取消清 pending；保存/放弃后执行原操作 |
| 导出确认 | modal/confirmation | 导出标题 | `pending_export` | 取消不写文件；空副轴可切单轴或继续导出 |
| 删除数据确认 | modal/confirmation | 删除标题 | `pending_data_removal` | 取消不修改数据；确认经事务删除 |
| managed file 冲突 | modal/confirmation | `手动数据文件冲突` | `pending_managed_save_conflict` | 取消不覆盖；确认后执行指定保存 |
| 文件选择器 | native system dialog | 平台管理 | 无通用 open/focus 状态 | 不纳入工具窗口 host |

所有独立 tool viewport 在主窗口全屏时继续使用当前 embedded 路径，在普通/最大化状态使用独立 viewport；本轮不得改变 stable ID、parent、focus 或持久化尺寸 key。

## 5. Controller/UI 函数职责分类

### 5.1 StudioApp 协调与窗口模块

- `lifecycle.rs`：`new`, `open_paths`, `open_lite_handoff`, `open_project`, `open_project_path`, `new_project`, `save_project`, `execute_project_save`, `request_replacement`, `perform_action`, `unsaved_dialog`。
- `data_workflow.rs`：`open_data`, `load_data_paths`, `export_manual_data`, `select_dataset`, `prepare_manual_data_window`, `manual_data_window`, `manual_data_fields`, `insert_manual_data`, `data_removal_dialog`, `series_tree`, `data_source_controls`, `column_controls_for_group`。
- `export_workflow.rs`：`export_pdf`, `export_pdf_to`, `export_png`, `export_svg`, `export_dialog`, `export_confirmation_text`。
- `axis_editor.rs`：`figure_size_editor`, `fit_canvas`, `axis_ranges_editor`, `axis_editor`, `axis_quick_editor_by_identity`, `axis_label_editor*`, `sync_axis_editors`, `axis_visibility_fields`, `axis_visibility_window`，以及纯 UI 的 axis binding editors。
- `artist_editor.rs`：selection/context target 管理、annotation/measurement label ownership、`add_text_annotation`, `inspector`, `palette_window`, `palette_fields`, `context_editor*`, `artist_editor`, semantic/label input editors。
- `tool_windows.rs`：palette/publication/axis/manual/reference/context viewport 的公共生命周期机制；只接管 host，不接管窗口业务内容。
- `interaction.rs`：`handle_drawing_tool_event`, `handle_artist_drag`；只消费 `CanvasToolEvent`/`CanvasDragEvent` 并选择命令或事务。
- 通用协调：messages、`commit_app_outcome`, `execute_document_edit*`, undo/redo。

### 5.2 直接进入领域模块、不得中转两次

- `axis_value_is_visible`, `reference_value_is_visible`, `measurement_points_are_visible`：审查后归入 axes/objects 的可见性规则或保留为纯 UI 警告 helper；不得机械搬入 controller 子模块后再迁移。
- `explicit_axis_unit_conflicts`, `explicit_column_unit`：单位/轴绑定领域规则候选，Phase 3 一次归位。
- reference/measurement 的约束、autoscale 包含关系和删除语义：归入 `document/objects.rs`。
- 双轴状态、空副轴判断、休眠绑定和关闭副轴恢复：归入 `document/axes.rs`。
- legend 样本、marker/line/error bar 组合及批量样式：归入 `document/series.rs`。

### 5.3 app_ui 分类

- `secondary_axis_placeholder_*` → `app_ui/axis_placeholders.rs`；仅编辑提示，不进入导出。
- `reference_tool_allows_object_interaction`, `opens_axis_visibility_from_blank_double_click` 及 pointer/hit 转换 → `app_ui/canvas_interaction.rs`。
- 顶栏、侧栏、消息栏和窗口装配 → `app_ui/shell.rs`。
- preview 放置、缩放和画布绘制 → `app_ui/canvas_view.rs`。
- selection、tool draft 和 reference draft 的临时绘制 → `app_ui/overlays.rs`。

## 6. 公共 API 冻结基线

基线快照覆盖：

- `apps/instplot-studio/src/lib.rs`；
- `crates/instplot-layout/src/lib.rs`；
- `crates/instplot-export/src/lib.rs`；
- `crates/instplot-ui/src/lib.rs`；
- project/document 中所有 `serde(rename/default/alias/tag)` 声明。

现有公共分组包括 data import、document/autoscale、editing、export、handoff、palette、preview、project schema/types、publication、render、semantic policy、session 和 text editing；公共类型路径、函数签名与 serde 名称默认不得改变。仓库内第二 consumer 只证明它使用的子集，不能作为删除其他 API 的充分理由。

任何删除或改名必须另列：符号、仓库内引用、已知外部文档/示例、兼容替代和批准记录。

## 7. 自动与真实验收基线

自动门禁以整合计划第 8 节为准。真实操作以：

- `docs/INSTPLOT_STUDIO_END_TO_END_MANUAL_QA_PLAN.md`；
- `docs/INSTPLOT_STUDIO_POST_REFACTOR_REAL_WORLD_QA_PLAN.md`；
- 整合计划第 9 节新增功能重点矩阵

共同构成。Phase 0 不以“测试数量相同”替代行为覆盖，也不以截图相似替代状态与导出检查。

Phase 0 自动门禁结果：

- `cargo fmt --all -- --check`：PASS；
- workspace 全 target/feature check：PASS；
- workspace 全 target/feature tests：PASS（Studio lib 179 passed、1 ignored；Studio main 87 passed；其余 workspace 与集成测试全部通过）；
- workspace doc tests：PASS；
- strict Clippy：PASS；
- `cargo check --locked -p instplot-demo`：PASS；
- `git diff --check`：PASS；
- repository hygiene：PASS；
- B5 Studio validator：19/19 PASS，包括真实 create/import handoff、project check 和 publication check；
- release 构建：PASS。

最新真实 UI 基线沿用 2026-09-28 build `202609281603` 的完成验收：普通窗口、最大化、全屏、坐标轴显示窗口滚动与 Spotlight 唯一应用均通过。该证据只作为迁移前基线；Phase 2 与 Phase 6 仍必须重新执行窗口和完整功能验收。

## 8. Phase 0 完成门

- [x] 仓库外恢复包已建立并实际恢复验证。
- [x] 工具窗口状态矩阵已建立。
- [x] 临时 UI/事务状态所有权已记录。
- [x] Controller/UI 新增函数已分类，避免双重搬迁。
- [x] 公共 API 与 serde 冻结范围已确定。
- [x] 两个已知假失败门禁已有精确修复和定向测试。
- [x] 完整自动门禁通过并保存结果。
- [x] 最新真实 UI 基线与本阶段记录关联完成。
