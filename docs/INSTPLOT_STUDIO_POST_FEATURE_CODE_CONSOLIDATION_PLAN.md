# InstPlot Studio 新功能阶段后的代码收尾整合计划

> 状态：`COMPLETED — local consolidation and verification finished`  
> 制定日期：2026-09-28  
> 当前分支：`codex/dual-axes`  
> 制定时 HEAD：`6aa97c3`  
> 性质：行为冻结后的结构整理，不是新功能开发  
> 执行前提：用户确认本计划后，按阶段执行；本计划本身不授权提交 GitHub 或创建 Release。

## 1. 背景与定位

2026-09-25 完成的十阶段模块化已经建立了数据、文本、文档、布局、渲染、导出和通用 UI 边界；该成果继续有效，本轮不重做。

此后 Studio 又增加了双 X/双 Y 轴、轴绑定、空副轴占位、spine 颜色、参考线、测量箭头、更多对象编辑、坐标轴显示窗口、紧凑导出边界和出版规范导出放行等功能。这些功能目前已经通过完整测试和真实界面验收，但新增实现再次集中到少数文件：

- `apps/instplot-studio/src/app_controller.rs`：约 5604 行；
- `apps/instplot-studio/src/app_ui.rs`：约 1591 行；
- `crates/instplot-layout/src/layout.rs`：约 3214 行；
- `crates/instplot-export/src/resolve.rs`：约 944 行；
- `apps/instplot-studio/src/document.rs`：约 1949 行；
- `apps/instplot-studio/src/project.rs`：约 1436 行。

行数本身不是错误。需要整合的依据是：应用事务、窗口内容、画布交互、对象编辑、双轴布局和导出解析在若干大文件中出现多种不同的修改理由。若继续直接增加功能，回归风险和阅读成本会继续上升。

本计划因此是原模块化计划的增量收尾：保留已经验证的公共 crate 和依赖方向，只把后来新增的职责放回正确边界。

## 2. 总目标

完成后应同时满足：

1. 当前所有用户行为、项目兼容性、屏幕布局和导出结果保持不变。
2. Studio 二进制中的 `StudioApp` 协调适配层回到“连接 UI、应用事务与窗口”的角色，不继续承载领域计算和大型对象编辑器；`app_transactions.rs` 中的 `ApplicationController` 继续作为原子应用事务的唯一入口，两者不得混称或互相复制职责。
3. 画布显示、指针交互、工具状态和独立窗口调度各自有明确入口。
4. 双轴、空副轴占位、参考线、测量箭头和轴可见性只有一个权威规则来源。
5. 预览、PDF、SVG、PNG 继续共享相同的 resolve/layout 结果，不产生第二套导出算法。
6. 新增文件按职责命名；测试跟随所属模块，避免再次形成单个巨型测试文件。
7. 每一阶段结束时应用都可构建、可测试、可启动；任一阶段可以独立回退。
8. 完成后的结构能继续支持新功能，而无需再次把代码堆入 `app_controller.rs` 或 `app_ui.rs`。

## 3. 明确非目标

本轮不得顺带进行以下工作：

- 不新增 inset axes、第三条轴、四轴、数据分析或其他产品功能。
- 不修改已经确认的双轴、参考线、测量箭头和导出交互。
- 不改变项目 schema，除非移动代码时发现现有反序列化已不能工作；这种情况必须先停下并单独讨论。
- 不更换 `egui`、渲染后端、字体体系或文件选择器。
- 不重新设计窗口、按钮、术语、颜色或默认值。
- 不为了文件变短而创建没有稳定职责的 crate。
- 不升级依赖，不构建未授权的 Windows/Linux 安装包。
- 不在整合未通过本地验收前提交 GitHub、打标签或创建 Release。

## 4. 不可破坏的行为基线

执行前必须把当前状态视为冻结基线，至少覆盖：

### 4.1 数据与项目

- 文件导入、同文件多列、多文件导入、手动录入和重复测量；
- 删除单个数据源、清除全部、再次导入和自动范围；
- 手动数据作为独立数据源存在并可导出；
- 项目保存、打开、旧 schema 迁移和 round-trip；
- X/Y/error 列绑定及双轴绑定保持不变。

### 4.2 图形与对象

- 单轴、双 X、双 Y切换及关闭副轴后的休眠绑定；
- 空副轴占位标签、真实绑定后的标签、spine、刻度和 gutter；
- 曲线、marker、line、error bar 和 legend 样本一致；
- 参考线、测量箭头、文字标注、连接线、拖拽和删除；
- 对象双击编辑、重复打开置顶以及窗口互不错误关闭。

### 4.3 布局与导出

- 图内/图外 legend、自动画布扩展和紧凑导出 margin；
- 隐藏 spine、ticks、tick labels、axis labels 后仍可导出；
- 空副轴导出时的“改为单轴并导出”和“仍然导出”；
- Publication Error/Warning 不阻止用户选择继续导出；
- PDF、SVG、PNG 的尺寸、字体、颜色、透明背景和对象边界。

### 4.4 界面环境

- macOS 普通窗口、最大化和全屏；
- 小工具窗口可移动、置顶、重新唤起和滚动；
- 左侧文件栏自由调整宽度、文件卡片及关闭操作；
- Spotlight 中只存在一个正确安装的应用。

### 4.5 临时 UI 与事务状态

以下状态不进入项目文件，但在迁移 controller、画布和窗口时必须保持现有语义：

- `dirty`、`allow_close`、`pending_action` 及未保存确认的取消/保存/放弃分支；
- `pending_export`、导出设置草稿、空副轴确认和 Publication Check 导出确认；
- managed manual file 冲突确认，以及保存取消后不得半提交；
- 手动录入窗口的只读/编辑状态、未提交草稿和数据组选择；
- `label_inputs` 与 `numeric_inputs` 的 deferred commit：输入过程中允许暂时为空，失焦/确认时才提交；
- reference/tool draft、active drag、selection candidates、context editor target 和 focus 请求；
- canvas zoom、scroll、临时 overlay 与当前 drawing tool。

对应验收必须证明：取消不会提交；关闭/重开窗口时草稿按现有规则保留或清除；一次连续拖拽只形成正确的一个 Undo group；失焦、置顶和对象删除不会遗留幽灵窗口或陈旧 target。

## 5. 目标模块边界

以下是职责目标，不要求一次性全部建立；优先在现有 crate 内建立 module，只有出现真实第二 consumer 时才提升为 crate。

### 5.1 Studio 应用协调适配层与事务层

本计划统一使用以下术语：

- **`StudioApp` 协调适配层**：当前 `app_controller.rs` 及后续子模块中的 UI/窗口/工作流 glue；
- **`ApplicationController` 事务层**：当前 `app_transactions.rs` 中的原子应用事务服务；
- **document command**：可撤销的领域修改；
- **canvas interaction**：把 `egui` 原始输入转换为命中结果和明确事件，不提交领域修改。

依赖顺序必须是：

```text
egui input
  → app_ui/canvas_interaction（坐标转换、命中、事件）
  → StudioApp 协调适配层（选择事务/命令、组织窗口反馈）
  → ApplicationController / EditCommand（验证并原子提交）
  → document
```

拖拽约束、双轴绑定、自动范围和对象删除语义只能在一个下层权威位置实现；`canvas_interaction` 与协调适配层不得各实现一份。

将当前 `app_controller.rs` 收敛为协调入口，并把实现移入同级子模块：

```text
app_controller/
  lifecycle.rs          新建、打开、保存、替换和未保存确认
  data_workflow.rs      导入、录入、数据源选择和删除事务的 UI 协调
  export_workflow.rs    导出设置、规范摘要、确认和事务触发
  axis_editor.rs        轴范围、刻度、可见性、轴绑定和 spine 编辑
  artist_editor.rs      曲线、图例、文字、参考线和测量箭头编辑调度
  tool_windows.rs       窗口打开、置顶、关闭和通用 viewport 生命周期
  interaction.rs        消费画布事件、选择事务/命令和管理工具状态
```

规则：

- 根 `app_controller.rs` 只保留 `impl StudioApp` 的组装、跨模块调用和极少量共享 helper；不得把它与 `ApplicationController` 称为同一个 Controller。
- 子模块不得自行解析文件、计算布局或直接写导出文件。
- 跨文档修改继续通过 `EditCommand`、`AppAction` 或明确事务完成。
- 如果某个 helper 同时被两个领域使用，先判断它属于 document、layout、export 还是 UI，不默认放回 controller。

### 5.2 画布与 UI 层

将 `app_ui.rs` 中不同修改理由分开：

```text
app_ui/
  shell.rs              顶栏、侧栏、状态栏与窗口装配
  canvas_view.rs        画布尺寸、预览放置和绘制
  canvas_interaction.rs 命中、双击、选择、拖拽和工具事件
  axis_placeholders.rs  空 X2/Y2 占位标签与预览 gutter
  overlays.rs           选择框、草稿参考线、测量预览等临时覆盖层
```

规则：

- 预览几何不读取某个弹窗的临时控件值；只读取已提交文档和明确的工具草稿。
- 命中检测与视觉绘制使用同一套对象身份和坐标变换。
- 通用窗口 host、紧凑表单行和颜色选择器优先复用 `instplot-ui`，但不强行把 Studio 专属业务放入通用 crate。

工具窗口统一只统一生命周期机制，不统一错误的产品语义。Phase 0 必须先建立如下窗口状态矩阵：

- 窗口类型：singleton、按对象 identity 的多实例、临时 draft、确认框、系统文件对话框；
- 稳定身份：当前 `egui::Id`、`ViewportId`、对象 `window_key` 和持久化尺寸 key；
- 状态：open、focus、close、再次打开置顶、parent、always-on-top、尺寸和滚动；
- 草稿：关闭后保留、提交后清除、对象删除时联动关闭；
- 平台：普通窗口、最大化、全屏下的行为。

`palette-viewport`、`publication-check-viewport`、`axis-visibility-viewport` 及现有对象 `window_key` 默认保持不变。文件对话框和一次性确认框不纳入通用 viewport host；多实例对象编辑器不得被错误收敛为 singleton。

### 5.3 文档与项目层

继续使用现有 `document/{axes,datasets,objects,series,...}` 与 `project/{migration,storage,validation,...}`：

- 双轴状态、轴绑定和可见性规则归入 `document/axes.rs`；
- 曲线样式、legend 样本和批量应用归入 `document/series.rs`；
- 参考线、测量箭头、文字标注及其约束归入 `document/objects.rs`；
- 数据源生命周期和列绑定归入 `document/datasets.rs`；
- schema、默认值和兼容升级只留在 project 层；
- UI 不直接拼接持久化结构以绕过文档命令。

只有领域规则确实仍停留在 controller/UI 时才迁移；不重复拆分已经清楚的模块。

### 5.4 Layout 与 Export

`instplot-layout` 负责“在哪里”，`instplot-export` 负责“如何形成导出 scene 和写出格式”。建议在现有 crate 内形成：

```text
instplot-layout/src/
  axes.rs               主副轴矩形、gutter、spine 和 tick geometry
  legend.rs             图内/图外 legend 测量与占位
  objects.rs            标注、参考线、测量箭头边界
  bounds.rs             内容边界与视觉边界合并
  layout.rs             阶段编排与最终 LayoutResult

instplot-export/src/
  resolve/axes.rs
  resolve/series.rs
  resolve/legend.rs
  resolve/objects.rs
  resolve/bounds.rs
  resolve.rs             组装统一 ResolvedFigure
```

边界要求：

- tight export margin 只在导出边界阶段生效，不改变交互画布大小。
- 空副轴占位属于 Studio 编辑提示，不进入正式导出 scene。
- axis label/tick/legend/artist 的边界计算不能在 PDF、SVG、PNG 后端各复制一份。
- backend 不重新决定轴绑定、legend 内容、字体语义或可见性。

## 6. 阶段执行计划

### Phase 0：保护现场与冻结基线

操作：

1. 记录分支、HEAD、工具链、工作区修改、未跟踪文件和当前 Spotlight build。
2. 不丢弃当前未提交功能；在仓库外保存：
   - `git diff --binary HEAD`；
   - `git status --porcelain=v2 -z`；
   - 未跟踪文件归档及逐文件 SHA256；
   - 当前 HEAD、分支和工具链说明。
3. 在以记录 HEAD 建立的临时 worktree/临时 clone 中执行 `git apply --check <snapshot.patch>`；必要时实际应用后核对 `git status`。随后展开未跟踪文件归档并复算逐文件 SHA256。不能只生成未验证的“恢复清单”。
4. 每个迁移批次保存独立 patch/checkpoint。若采用本地 checkpoint commit，必须先获得用户同意，不推送远程。
5. 运行完整自动门禁并保存结果。
6. 建立工具窗口状态矩阵，冻结稳定 ID、singleton/多实例、草稿、置顶、关闭、尺寸和滚动行为。
7. 将最新真实 UI 验收结果关联到本计划，补充双轴、参考线、测量箭头、坐标轴显示窗口及第 4.5 节临时状态用例。
8. 保存 `lib.rs` 的 `pub use`、公共类型/函数签名、类型路径和 serde 名称清单；默认不得改变。
9. 逐函数分类 controller/UI 新增代码：协调、窗口、画布输入、领域规则、layout、export。已经确认属于 document/layout/export 的代码不得先搬入 controller 子模块再二次迁移。

门禁：

- 没有移动生产代码；
- 当前功能全部可复现；
- 所有计划迁移区域都有自动测试或明确人工用例；
- 恢复快照已实际验证，窗口矩阵和公共 API 清单已经保存；
- 任何现有失败必须先解释，不能把失败带入迁移阶段。

### Phase 1：拆分 StudioApp 协调适配层，不改变行为

顺序：

1. 生命周期与文件/项目工作流；
2. 导出工作流；
3. 数据和手动录入工作流；
4. 轴编辑器与坐标轴显示窗口；
5. artist 编辑器；
6. 参考线、测量箭头和画布事件提交。

每次只移动一个连续职责区域，调整 `pub(super)` 可见性，测试跟随迁移。先移动，后整理；不得在同一补丁中改变交互。

Phase 0 已分类为纯领域规则的函数跳过本阶段，直接在 Phase 3 一次迁入 document；已分类为 layout/export 的函数直接留待 Phase 4。不得为了机械拆分让同一代码连续搬迁两次。

门禁：

- 根 controller 不再包含完整窗口布局或大型领域编辑器；
- 不出现 controller 子模块之间的循环依赖；
- 没有新增直接修改 `ProjectDocument` 内部字段的旁路；
- 每个迁移批次通过相关定向测试、`cargo check` 和严格 Clippy。

### Phase 2：拆分画布 UI 与统一窗口策略

操作：

1. 分离 shell、canvas view、canvas interaction、placeholder 和 overlay。
2. 将独立窗口的“已打开则置顶、未打开则创建、关闭后清理”形成一个通用 host/policy。
3. 让配色、出版规范、坐标轴显示、录入数据和对象编辑窗口共享生命周期规则。
4. 只复用通用布局原语，不改变各窗口当前内容与尺寸。

统一窗口策略必须以 Phase 0 的状态矩阵为契约：保留稳定 ID、对象多实例身份、草稿清理规则和 focus flag；系统文件对话框与确认框继续走现有独立路径。

门禁：

- 画布绘制代码不处理文件/项目事务；
- 指针事件不直接实现领域规则，只产生明确事件或命令；
- 普通窗口、最大化、全屏以及重复打开/置顶真实操作通过；
- 截图和关键几何基线没有非预期变化。

### Phase 3：领域规则归位

操作：

1. 审核双轴绑定、轴可见性、空副轴判断、autoscale 和关闭副轴恢复逻辑。
2. 审核 series 的统一颜色、marker/line legend、error bar 和批量样式。
3. 审核 reference/measurement/text 的持久化状态、约束和删除语义。
4. 将仍位于 UI/controller 的纯规则迁入现有 document 子模块，并用纯单元测试覆盖。

门禁：

- 同一规则只有一个生产实现；
- UI 只编辑草稿或发送命令；
- Undo/Redo、保存重开和旧项目迁移保持一致；
- 单轴、双 X、双 Y及休眠绑定测试全部通过。

### Phase 4：拆分 Layout 与 Export Resolve

操作：

1. 先提取纯 helper 和数据结构，再拆分轴、legend、objects、bounds。
2. 保持 `LayoutResult`、`ResolvedFigure`、已有公共签名、类型路径和 serde 名称不变；确实需要删除的无 consumer API 必须先列出仓库内外已知 consumer 证据并单独批准。
3. 让预览和导出继续消费同一 layout/scene；不得建立仅为某一种格式服务的几何规则。
4. 使用固定文档比较迁移前后的轴矩形、legend rect、对象边界、scene 命令和导出尺寸。

等价比较方法固定为：

- `LayoutResult` 与 scene/display-list：结构化比较；浮点使用明确且足够小的固定容差；
- PNG：比较尺寸、透明度，并使用有记录的像素差阈值/基线；
- SVG：规范化非语义顺序与浮点格式后，比较元素、属性和关键几何；
- PDF：不比较原始字节；解析 page box、页面数量、文本/路径数量及关键几何，因为元数据和对象顺序可能非确定。

门禁：

- PDF/SVG/PNG 后端不包含布局决策；
- tight export margin 不改变屏幕画布；
- 图外 legend、隐藏轴元素、双轴与 error bar 的边界测试通过；
- 代表性导出产物无非预期视觉变化。

### Phase 5：测试归位与公共接口清理

操作：

1. 将 `app_tests.rs` 和 `document_tests.rs` 按所属领域迁入对应测试模块。
2. 区分纯单元测试、服务契约测试、集成测试和真实 UI 验收，删除只重复内部实现的断言。
3. 删除迁移后无调用的 helper、兼容入口和重复本地化字符串。
4. 检查 `lib.rs` 的 `pub use`：只公开真实 consumer 需要的 API。
5. 更新模块 README、扩展指南和架构说明。

门禁：

- 测试数量不能因迁移而无解释减少；
- 第二应用继续只通过公共 API 完成 CSV → XY → 标签 → SVG；
- `cargo check -p instplot-demo`、workspace doc tests 和 Phase 0 公共 API 清单对比通过；Demo 只证明其覆盖的 API 子集，不能替代完整公共签名检查；
- 无 dead code、unused dependency、重复实现或 UI 泄漏进核心 crate；
- 从文档可以定位新增轴规则、对象类型、导出边界和工具窗口应修改的位置。

### Phase 6：完整回归与本地封板

操作：

1. 运行全部自动门禁。
2. 在真实应用中执行核心工作流、普通/最大化/全屏窗口矩阵。
3. 使用独立 agent 按计划与真实用户流程检查，不做代码风格审查替代功能验收。
4. 从 release 构建替换唯一 Spotlight 应用，验证版本、签名、架构和启动。
5. 更新本计划状态、阶段证据、剩余风险和最终模块地图。

门禁：

- 所有自动检查通过；
- 独立功能 QA 无 P0/P1；
- 当前功能基线没有回归；
- 工作区中没有测试输出、用户数据、本机路径或无用途临时文件；
- 未经用户再次明确授权，不提交 GitHub、不创建 Release。

## 7. 每个迁移批次的固定流程

每个批次必须按以下顺序执行：

1. 写明目标职责、源位置、目标位置和明确非目标。
2. 先运行该区域现有定向测试；缺少行为保护时先补测试。
3. 只移动一个职责，保持 API 或增加最薄适配层。
4. 运行格式检查、定向测试、`cargo check` 和 Clippy。
5. 检查 diff，确认没有默认值、文案、布局常量或 schema 意外变化。
6. 完成相邻领域回归后才删除旧入口。
7. 记录阶段结果，再进入下一个批次。

如果迁移暴露真实产品 bug：

- 先保留能证明旧行为的基线；
- 将 bug 记录为独立修复；
- 除 P0 阻断外，不在纯迁移补丁中同时改变行为；
- P0 必须先写失败测试，再以单独的小批次修复。

## 8. 自动门禁

仓库固定 Rust 1.98.0。每个大阶段至少执行：

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --doc
cargo clippy --locked --all-targets -- -D warnings
cargo check --locked -p instplot-demo
git diff --check
```

最终阶段增加：

```bash
cargo build --locked --release -p instplot-studio
python3 scripts/check_repository_hygiene.py
python3 scripts/validate_b5_studio.py
```

制定计划时已确认一个 Phase 0 基线问题：`check_repository_hygiene.py` 当前会因
`apps/instplot-studio/tests/fixtures/project-v8-single-source.instplot` 中保存的历史
`/Users/...` 来源路径而失败。执行时必须先判断该路径是否属于迁移语义测试：优先把 fixture
改为平台无关的稳定占位来源，并保持迁移断言；只有能够证明该值必须原样存在时，才为卫生检查
增加范围精确且有说明的 fixture 例外。不得直接忽略全部 fixture 或删除对应迁移覆盖。

第二个 Phase 0 基线问题是 `scripts/validate_b5_studio.py` 的 `studio-launch-contract`
仍要求 `--open-handoff` 与旧文案 `Open from Lite…` 同时出现在 `main.rs`。当前实现已分别迁移到
`startup.rs` 和 `ui_text.rs`，英文文案为 `Open from InstPlot Lite`，因此脚本会产生假失败。
必须把该检查改成行为/模块级验证：复用或调用启动参数解析测试以及 `Text::OpenLite` 本地化测试，
不得继续依赖某个字符串位于 `main.rs`。

若 Cargo shim 行为异常，按仓库规则使用：

```bash
rustup run 1.98.0-aarch64-apple-darwin cargo clippy --version
rustup run 1.98.0-aarch64-apple-darwin cargo clippy --locked --all-targets -- -D warnings
```

## 9. 真实功能验收重点

除执行 `docs/INSTPLOT_STUDIO_END_TO_END_MANUAL_QA_PLAN.md` 外，本轮重点检查：

1. 单轴导入、删除、清空、再次导入和自动范围。
2. 双 X/双 Y先开后绑定、先导入后开、关闭再打开和项目重开。
3. 空副轴占位标签、绑定后的真实 label/pad、切回单轴画布恢复。
4. 参考线添加确认、继续添加、取消、选择、移动和删除。
5. 测量箭头端点、双向箭头、约束拖拽、标签清空和删除。
6. 曲线/marker/line/error bar 与 legend 样本、颜色和批量样式一致。
7. 坐标轴显示窗口的滚动、紧凑布局、置顶和同窗口编辑。
8. 隐藏轴元素后导出、空副轴导出选择、Publication Error 继续导出。
9. 图外 legend、全部轴元素隐藏以及文字/对象存在时的紧凑导出 margin。
10. 普通窗口、最大化、全屏和多个工具窗口同时存在。

## 10. 完成标准

本轮只有同时满足以下条件才算完成：

- 新增功能已落入职责正确的模块，而不是简单把一个大文件切成若干无边界片段；
- `StudioApp` 协调适配层主要表现为 UI、窗口与事务连接，完整窗口和纯领域算法已迁出；`ApplicationController` 继续作为原子应用事务的唯一入口；
- `app_ui` 的 shell、画布绘制、交互和覆盖层边界清楚；
- layout/export 对轴、legend、objects 和 bounds 的职责可独立定位与测试；
- 没有第二套双轴、边界计算、导出 margin 或对象编辑规则；
- 项目 schema、屏幕行为和导出结果没有未批准变化；
- 全部自动门禁和真实 UI 验收通过；
- 独立 agent 功能验收通过；
- 文档、模块地图与实际源码一致；
- 是否提交 GitHub、如何组织提交和是否发布，由用户在本地验收后另行决定。

## 11. 软性规模指标

这些指标用于发现职责是否仍然过度集中，不是为了追求行数：

- 根 `app_controller.rs` 目标约 800–1200 行；超出时必须能说明剩余内容为何都属于协调层。
- 根 `app_ui.rs` 目标约 500–800 行；画布绘制和交互不应继续混为一个长函数。
- `layout.rs` 与 `resolve.rs` 应成为编排 facade；轴、legend、objects、bounds 可分别定位。
- 单个函数超过约 250 行时必须审查是否包含多个事务或多种 UI 区域。
- 不以增加 crate 数量作为成功指标；优先使用现有 crate 内部 module。
- 模块依赖方向、依赖扇出、跨层引用和函数职责优先于行数；不得为了达到目标数字制造只含 re-export、转发调用或无独立语义的碎片文件。

## 12. 执行记录

- [x] Phase 0：保护现场与冻结基线（证据：`docs/INSTPLOT_STUDIO_POST_FEATURE_PHASE0_BASELINE.md`）
- [x] Phase 1：拆分 StudioApp 协调适配层（证据：`docs/INSTPLOT_STUDIO_POST_FEATURE_PHASE1_CONTROLLER_SPLIT.md`）
- [x] Phase 2：拆分画布 UI 与统一窗口策略（证据：`docs/INSTPLOT_STUDIO_POST_FEATURE_PHASE2_TO_5_EVIDENCE.md`）
- [x] Phase 3：领域规则归位（证据：同上）
- [x] Phase 4：拆分 Layout 与 Export Resolve（证据：同上）
- [x] Phase 5：测试归位与公共接口清理（证据：同上）
- [x] Phase 6：完整回归与本地封板（证据：`docs/INSTPLOT_STUDIO_POST_FEATURE_CONSOLIDATION_FINAL_REPORT.md`）
