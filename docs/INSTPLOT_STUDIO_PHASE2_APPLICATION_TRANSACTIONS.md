# InstPlot Studio Phase 2 应用事务验收记录

日期：2026-09-25
结论：通过；可以进入 Phase 3。

## 1. 状态所有权

- `FigureDocument`：坐标、系列、样式、图例、标注和持久化数据的权威状态。
- `StudioSession`：从项目数据或新导入数据重建的非权威缓存。
- `WorkspaceState`：项目路径、来源和显示名称。
- `EditHistory`：文档撤销、重做和保存点。
- `StudioApp` 的选择、窗口、输入草稿、hover 和拖拽字段：仅为 UI 临时状态。
- `ResolvedFigure` 与 `PublicationReport`：每次成功事务后生成的派生状态。

## 2. 唯一事务入口

新增 `app_transactions.rs`，由 `ApplicationController::execute(AppAction)` 统一处理：

- 新建项目；
- 打开 Lite 交换包；
- 导入一个或多个文件；
- 提交手动录入生成的数据；
- 删除文件和清除全部数据；
- 打开、保存项目；
- 撤销、重做；
- 普通文档编辑；
- PDF、PNG 导出。

事务先在候选文档、候选会话、候选工作区和候选撤销历史上执行，完成验证、布局和出版检查后才返回 `AppOutcome`。`StudioApp::commit_app_outcome` 是 document/session/workspace/history/derived state 的唯一批量替换点；UI 只提供动作参数并处理成功后的选择与消息。

导入所需的 XY 偏好作为 `AppAction` 参数传入，不允许事务控制器读取 UI 草稿。领域计算仍由 `StudioSession`、`FigureDocument`、编辑命令和导出服务完成，控制器只安排调用顺序与提交。

## 3. 回归保护

新增或改造的结果导向测试覆盖：

- 失败导入不修改当前文档、工作区或会话；
- 删除数据时文档、会话和撤销历史同步提交；
- 清除后再次导入恢复系列、自动范围和标签；
- 保存与打开共享同一状态迁移并保持保存点；
- 撤销/重做同步重建派生会话；
- 现有批量导入、先拟合后来源重试、手动重复测量和多 XY 录入继续通过。

## 4. 阶段门槛证据

- `cargo fmt --all -- --check`：通过。
- `cargo test --locked --workspace`：215 个测试通过，0 个失败，1 个显式忽略的视觉检查测试。
- `cargo clippy --locked --all-targets -- -D warnings`：通过。
- 二进制 UI 中没有绕过事务入口的 `EditHistory::execute/undo/redo`，权威状态赋值集中在 `commit_app_outcome`。

## 5. Phase 3 入口约束

- `ApplicationController` 已建立事务边界，但文件导入与手动数据的规范化实现仍分别位于 `StudioSession` 和二进制 `manual_data.rs`；Phase 3 要统一为不依赖 GUI 的数据管线。
- `CommitPreparedData` 是 Phase 2 的过渡动作。Phase 3 完成后，手动录入应直接产生统一的 `ParsedDataset`/规范数据集，不再由 UI 先组装完整文档。
- 当前普通导入默认嵌入项目，离线恢复契约继续有效；`External` 历史兼容路径仍需在 Phase 3 明确恢复诊断。
