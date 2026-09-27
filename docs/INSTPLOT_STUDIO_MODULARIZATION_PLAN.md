# InstPlot Studio 模块化与复用重构计划

状态：已完成；Phase 0–10 全部通过阶段门禁
制定日期：2026-09-25
适用范围：InstPlot Studio 的代码组织、模块边界、复用能力与回归保护
核心原则：先固定行为，再迁移代码；先明确谁拥有状态、谁有权修改状态，再决定代码放在哪个文件或 crate；分阶段替换，始终保持主应用可构建、可测试、可回退。

## 1. 为什么现在需要重组

InstPlot Studio 的基础功能已经接近完整，但新增功能和修复回归的成本正在上升。当前主要问题不是单纯的代码行数，而是多个职责在同一文件、同一状态对象和同一操作流程中交叉：

- `apps/instplot-studio/src/main.rs` 约 8542 行，同时承担启动、应用状态、窗口、菜单、文件导入、对象编辑、拖拽、导出和大量测试。
- `document.rs` 约 4547 行，同时承担文档操作、自动范围、系列、图例、标注、布局映射和测试。
- `project.rs` 约 2444 行，同时承担项目格式、迁移、验证、文件读写和测试夹具。
- `label_input.rs`、`manual_data.rs`、`text_input.rs`、`ui_text.rs` 和 `workspace.rs` 仍主要作为二进制内部模块，难以被第二个应用直接复用。
- 文件导入已有底层能力，但“导入文件—建立数据集—选择 XY—生成系列—自动范围—更新界面”的完整事务仍分散在多个位置。
- 屏幕预览、PDF/PNG/SVG 导出依赖若干名称仍带 `spike` 的原型 crate；它们已被生产应用使用，却还没有形成清晰、稳定的产品级边界。
- 最近出现过删除后重新导入不自动调整范围、标签换行被界面层处理掉、数学下标规则被字符串长度猜测覆盖等问题，说明状态归属和规则来源还不够单一。

因此，本计划的目标不是“把大文件机械切小”，而是建立可理解、可测试、可复用的产品架构。

## 2. 最终目标

重组完成后应满足：

1. Studio 二进制只负责组装、品牌信息和平台启动，不再承载核心业务规则。
2. 数据导入、文本语义、文档操作、布局、渲染、导出和通用界面组件分别有清晰边界。
3. 每种用户操作只有一个权威入口，导入、删除、清除和重新导入等操作以完整事务执行。
4. 屏幕预览和所有导出格式共享同一份解析结果、布局结果和绘制描述。
5. macOS 全屏、Windows 最大化、触控板和鼠标操作由统一窗口策略处理，而不是每个小窗口各写一套规则。
6. 新建第二个绘图软件时，可以通过依赖 workspace crate 和配置品牌/功能直接复用，不复制 Studio 源码。
7. 每个阶段都能独立验收和回退，不以一次性重写换取整洁。

## 3. 本计划明确不做

- 不在重构阶段改变现有项目文件格式或破坏旧项目兼容性。
- 不借重构重新设计已经确认的界面交互。
- 不在同一批次增加双 Y 轴、inset axes 等新功能。
- 不一次性更换 GUI 框架。
- 不为了“模块数量好看”过度拆 crate；只有具有稳定职责和复用价值的边界才升级为 crate。
- 不在纯迁移提交中混入功能修复；发现问题时先增加失败测试，再单独修复。

## 4. 架构原则

### 4.1 单一职责与单向依赖

- 核心数据和规则不得依赖 `egui`、窗口句柄或平台 API。
- UI 可以调用应用服务，但业务服务不得反向读取某个窗口控件。
- 渲染与导出只接收已解析、已解析样式的文档，不猜测界面状态。
- 项目格式层只描述可持久化内容，不保存悬浮、窗口开关、输入草稿等临时状态。

### 4.2 单一事实来源

- 坐标范围、标签、系列样式和图例内容以 `FigureDocument` 为权威。
- 已载入的原始表格数据由数据层管理，文档只保存稳定的数据集身份和列绑定。
- 当前选择、打开的小窗口和未提交输入属于 UI 临时状态。
- 自动范围、删除、重新导入等跨模块操作必须通过应用事务完成，不能由多个窗口分别修改。

### 4.3 行为优先于文件结构

- 移动代码前先建立能证明当前正确行为的测试。
- 每次只迁移一个边界，迁移前后输出和交互必须一致。
- 重构完成的判断依据是可复用 API 和端到端行为，而不是文件变短。

### 4.4 Controller 只协调事务

- `AppController` 只接收应用级 action、安排调用顺序并提交或回滚结果。
- 导入解析、自动范围、标签语义、文档修改和导出规则仍属于各自领域模块。
- Controller 不向领域服务传递可任意修改的 `&mut AppModel`，也不直接实现领域算法。
- Controller 的规模和依赖必须受测试约束，不能成为新的 `main.rs`。

## 5. 状态职责约定

这是整个重组中最先确定、以后不得随意绕过的规则。

### 5.1 持久化文档状态

`ProjectDocument` / `FigureDocument` 负责：

- 画布与坐标轴配置；
- 数据集稳定标识与 XY/error 列绑定；
- 系列、图例、标注和文字对象；
- 可保存、可撤销、可导出的用户编辑结果。

### 5.2 数据所有权与持久化契约

科研绘图项目必须以可复现为默认原则。目标契约为：

- 项目文件持久化重建最终图形所需的规范化数值数据、`DatasetId`、列绑定、单位和必要的导入元数据。
- 源文件路径、文件名、时间戳或内容摘要作为来源信息保存，但默认不是实时数据引用。
- 外部 CSV/TXT 被移动、删除或修改后，重新打开项目仍应显示保存时的数据和图形。
- “从源文件重新载入”必须是用户明确触发的操作，并在覆盖项目前显示来源变化和影响。
- 如果历史项目只保存外部引用，打开时进入明确的 missing-source 恢复流程，不得静默换用别的数据或只显示部分图形。
- 大型数据将来如需外置存储，必须采用项目管理的内容寻址资源或打包格式，并保留完整性校验，不能退化为裸路径依赖。

在 Phase 0 必须先审计当前 `.instplot` 格式实际保存了什么，再确认迁移方案；在此之前不改变已有文件格式。

### 5.3 数据会话状态

`StudioSession` 或后续的 `DataSession` 负责：

- 已解析的表格、列和数值缓存；
- 文件格式识别与规范化结果；
- 可从项目持久化数据或导入结果重建的派生缓存。

它不是图形状态的权威来源，不能单独决定坐标范围、图例文字或系列样式。

### 5.4 工作区状态

`WorkspaceState` 负责：

- 当前项目路径、未保存状态和最近操作；
- 示例图、空白项目和已导入项目之间的生命周期；
- 打开、保存、另存为的文件身份。

### 5.5 UI 临时状态

`UiState` 负责：

- 当前选中对象；
- 各独立工具窗口的开关、位置和输入草稿；
- hover、拖拽预览和临时错误提示；
- 不应进入撤销栈、项目文件或导出结果的状态。

### 5.6 应用事务

`AppController` 是跨层操作的唯一协调者，例如：

- 导入一个或多个文件；
- 切换 XY 列并自动绘图；
- 删除单个数据源；
- 清除全部并恢复真正的空白默认画布；
- 删除后重新导入并重新自动范围；
- 打开项目、迁移、验证和提交；
- 导出前验证并生成不可变快照。

事务必须返回结构化结果；失败时不得留下“数据已导入但图没更新”一类半完成状态。

推荐的数据流为：

```text
AppAction
    ↓
AppController（只协调）
    ↓
DataImporter → ParsedDataset
    ↓
DocumentCommands → ChangeSet
    ↓
验证后一次性 commit，失败则不修改现有状态
```

## 6. 候选模块边界与依赖方向

下图描述职责边界，不代表从第一天就必须建立九个 crate。建议先在现有 crate 内形成同名 Rust module，边界稳定后再决定是否提升为 workspace crate：

```text
instplot-studio（薄二进制：启动、品牌、菜单组装）
  ├─ instplot-ui（通用窗口、控件、主题、交互策略）
  └─ instplot-app（用例与事务协调）
       ├─ instplot-data（文件/粘贴数据导入、列识别、重复测量）
       ├─ instplot-document（文档命令、撤销、自动范围、对象操作）
       ├─ instplot-text（标签语法、数学语义、字体与帮助）
       └─ instplot-export（导出请求、格式、原子写入）
            └─ instplot-render（统一绘制描述与后端）
                 ├─ instplot-layout（现有布局 crate）
                 └─ instplot-model（稳定的数据与样式类型）
```

依赖只能大体向下，禁止核心 crate 引用 Studio 的窗口或品牌代码。

### 6.1 Crate 提升规则

一个内部 module 只有满足下列至少一项，才考虑提升为独立 crate：

1. 已出现第二个真实 consumer；
2. 需要通过编译边界阻止错误依赖；
3. 可以在不启动 Studio 的情况下完整独立测试；
4. 独立编译或缓存能带来可测量的收益；
5. 公共 API 已相对稳定，预计不会在相邻阶段频繁重写。

不满足条件时优先保留为 `instplot_app::data`、`instplot_app::document`、`instplot_app::text` 等内部模块。Phase 9 的第二个真实应用是检验并促进 crate 提取的主要时点。

### 6.2 `instplot-model`

存放跨模块共享且稳定的纯数据类型、标识、单位、颜色、marker、线型和错误类型。不得包含文件对话框、GUI 控件或导出后端。

### 6.3 `instplot-data`

统一处理 CSV/TSV/TXT/表格文件和粘贴数据：编码、分隔符、数值清洗、列识别、重复测量、均值/标准差及 error 列。所有入口共享同一套规范化规则。

### 6.4 `instplot-text`

负责普通文字、数学变量、希腊变量、数学符号、上下标、换行、转义和字体选择。屏幕与导出必须使用同一语义树，禁止 UI 再用字符串长度猜测斜体或正体。

### 6.5 `instplot-document`

按领域拆为 `axes`、`series`、`legend`、`annotation`、`autoscale`、`datasets`、`commands` 和 `undo`。对外暴露意图明确的命令，不允许 UI 直接拼接内部状态。

### 6.6 `instplot-render` 与 `instplot-export`

将已进入生产路径的 render/export 原型升级为正式 workspace crate。预览、PDF、PNG 和 SVG 从同一不可变绘制描述生成；导出使用临时文件加原子替换，失败不破坏旧文件。

### 6.7 `instplot-ui`

沉淀主题、尺寸、按钮、输入行、数据文件卡片、对象编辑窗口、可移动窗口策略和跨平台窗口约束。它提供组件和策略，不包含 Studio 专属工作流。

## 7. 计划中的稳定用例接口

接口名称允许在实现时微调，但职责不得重新散开。领域服务返回结果或变更描述，不接收可以任意改写整个应用的 `&mut AppModel`：

```rust
AppController::execute(&mut self, action: AppAction) -> Result<AppOutcome, AppError>
DataImporter::import_files(paths, ImportOptions) -> Result<Vec<ParsedDataset>, ImportError>
DataImporter::import_pasted_columns(ManualInput) -> Result<Vec<ParsedDataset>, ImportError>
DocumentCommands::set_axis_label(axis, LabelSpec)
DocumentCommands::remove_dataset(dataset_id) -> ChangeSet
DocumentCommands::clear_all() -> ChangeSet
LabelSyntax::parse(source) -> LabelDocument
LabelSyntax::validate(source) -> Vec<LabelDiagnostic>
ExportService::export(&FigureSnapshot, ExportRequest) -> ExportReport
```

`AppAction` 至少覆盖 `ImportFiles`、`ImportPastedData`、`RemoveDataset`、`ClearAll`、`SetAxisLabel`、`OpenProject`、`SaveProject` 和 `ExportFigure`。`AppOutcome` 负责携带提交后的变更、诊断和需要 UI 呈现的后续动作；领域模块不直接打开窗口或写消息栏。

第二个应用应通过配置品牌、菜单和功能集合来组装通用界面，例如 `Branding`、`FeatureSet` 和 `WindowPolicy`，而不是复制 `main.rs`。

## 8. 分阶段执行计划

### Phase 0：行为基线与依赖地图（P0）

工作内容：

- 记录模块依赖、公共 API、项目格式版本和生产实际使用的原型 crate。
- 为高风险流程补齐结果导向测试，不只检查对象是否存在。
- 固定代表性 PNG/PDF/SVG、文本布局和导入结果基线。
- 把现有端到端人工测试计划纳入每阶段门槛。

重点基线：首次导入、多文件导入、删除一个、清除全部、删除后重新导入、XY 切换、手动数据、error bar、标签语义、图例拖拽、全屏与最大化。

完成门槛：不移动生产代码；所有已确认行为都有自动测试或明确人工用例；当前主分支能完整构建、Clippy 无警告。

### Phase 1：拆分二进制但不改变行为（P0）

工作内容：

- 从 `main.rs` 迁出 `startup`、`app_state`、`app_controller`、`actions` 和 `ui/*`。
- 测试跟随所属模块迁移。
- 仅做可验证的移动和可见性调整，不顺手改交互。

完成门槛：`main.rs` 只保留启动、平台初始化与顶层组装，不含数据解析、文档修改或渲染规则；约 500 行是软目标而不是硬 KPI；项目格式、截图和端到端行为无变化。

### Phase 2：集中状态所有权与应用事务（P0）

工作内容：

- 明确并验证文档、持久化数据、会话缓存、工作区和 UI 临时状态的所有权。
- 引入精简的 `AppController` 和明确的 `AppAction` / `AppOutcome` / `AppError`。
- 将导入、删除、清除、重新导入、打开和保存集中为原子事务。
- Controller 只协调 `DataImporter`、`DocumentCommands` 等领域服务，不接收或传递任意写入整个应用模型的权限。
- UI 不再分别修改 document、session 和 workspace。
- 为事务增加撤销边界、提交前验证和失败回滚规则。

完成门槛：不存在绕过 controller 的跨层写入；Controller 内没有导入解析、自动范围或渲染算法；删除/清除/重新导入流程有精确状态断言；已知坐标范围回归无法复现。

### Phase 3：提取统一数据管线（P0）

工作内容：

- 把文件导入、粘贴数据和手动重复测量统一到数据 module；满足 crate 提升规则后再提取为 `instplot-data`。
- 文件解析与“将数据加入当前图”分离，解析层不依赖 GUI。
- 落实数据所有权与持久化契约，保证项目在原始源文件缺失时仍可复现。
- 建立格式能力表和统一诊断；导入行为与 Lite 已验证能力逐项对照。
- 多文件、同名文件、同文件多组 XY、空值、科学计数法、不同分隔符和编码均进入测试矩阵。

完成门槛：所有导入入口调用同一规范化管线；独立测试程序无需 Studio UI 即可读取文件并生成标准数据集；清除后再次导入行为与首次导入一致；项目 reopen 不依赖原始 CSV 仍能恢复完整图形。

### Phase 4：提取统一文本与标签语义（P1）

工作内容：

- 将标签解析、自动括号、转义、数学语义、符号表和字体选择移入文本 module；边界稳定后再决定是否成为 `instplot-text`。
- 用明确语法规则替代长度、字符种类等启发式猜测。
- 普通文本和 `$...$` 数学文本的行为写成可查阅规则表。
- 预览和导出共用同一解析树与字体回退策略。

完成门槛：解析/格式化 round-trip 测试通过；`$v_sk$`、`$v$_sk`、比较符号、换行和转义有明确测试；屏幕与 PDF 字形、斜体/正体和换行一致。

### Phase 5：拆分文档领域、命令和项目存储（P1）

工作内容：

- 按 axes/series/legend/annotation/autoscale/datasets 拆分文档操作。
- 将项目 schema、迁移、验证、文件 IO 和测试 fixture 分离。
- 冻结现有 schema 兼容承诺；新字段必须有默认值和迁移测试。
- 将自动范围拆为 `compute_data_bounds`、`apply_autoscale_policy` 和 `apply_visual_padding` 等纯步骤。
- 默认数据范围包含数据点与 error bar；annotation 默认不进入 data bounds；marker 的 pt 尺寸只转化为可控的视觉 padding，不当作数据坐标。
- 用显式策略表达行为，例如 `AutoscalePolicy { include_error_bars, include_data_annotations, marker_padding, ... }`。

完成门槛：旧项目 fixture 全部可打开并 round-trip；领域操作可脱离 GUI 单测；UI 不直接修改 schema 字段；数据范围与视觉留白有分别可验证的结果。

### Phase 6：产品化渲染与导出管线（P1）

目标管线：

```text
FigureDocument + DataSession
             ↓ Resolve
        ResolvedFigure
             ↓ Layout
      DisplayList / Scene
             ↓
  egui preview | PDF | SVG | PNG
```

工作内容：

- 将生产正在依赖的 `studio-render-spike`、`text-shaping-spike`、`export-backend-spike` 和必要的 shell 能力逐步升级为正式边界；是否独立成 crate 仍按提升规则判断。
- 建立统一的 `ResolvedFigure`、布局结果和 `DisplayList` / scene。
- backend 只绘制已经决定好的内容，不再执行标签解析、自动范围、图例布局、字体语义或边距决策。
- 预览和各种导出只选择后端，不重复解释文档。
- 建立裁边、画布尺寸、图外图例和透明背景的可视基线。

完成门槛：相同 display list 在预览/PDF/PNG/SVG 中几何一致；导出失败保持原文件；生产应用不再依赖命名为 `spike` 的核心路径。

### Phase 7：统一 UI 设计系统与窗口策略（P1）

工作内容：

- 建立统一 theme/spacing/typography/control metrics。
- 统一独立小窗口的移动、置顶、关闭、全屏和 Windows 最大化规则。
- 提取按钮行、紧凑表单、文件卡片、符号面板和对象编辑器等组件。
- 用户可见文字只从本地化资源获取，删除散落硬编码。

完成门槛：通用组件有独立展示页或测试 harness；同类控件对齐一致；macOS 普通/全屏和 Windows 普通/最大化矩阵通过。

### Phase 8：形成可复用 UI 外壳（P2）

工作内容：

- 暴露稳定的主题、窗口、文件栏、画布和对象编辑组件。
- 用 `Branding`、`FeatureSet` 和服务 trait 配置产品差异。
- Studio 专属菜单、版本和产品名称留在应用层。

完成门槛：组件文档说明状态归属、事件和尺寸约束；另一个应用能选择性启用导入、编辑和导出功能。

### Phase 9：用第二个应用证明复用成立（P2）

工作内容：

- 新建最小 `apps/instplot-demo` 或 `examples/reusable-plot-editor`。
- 使用不同产品名和功能组合。
- 直接依赖正式 crate，禁止复制 Studio 源文件。
- 完成“导入文件—选 XY—编辑标签—导出图片”的真实流程。

完成门槛：示例应用可独立构建和运行；复用代码通过依赖获得；Studio 与示例应用同时通过测试。

### Phase 10：清理、文档和持续集成（P2）

工作内容：

- 删除所有调用方已迁移的旧适配器和重复实现。
- 补齐 crate README、API 示例、架构决策记录和扩展指南。
- CI 固定格式化、Clippy、单测、文档测试、关键可视基线和兼容性检查。
- 测量构建时间和依赖数量，合并没有实际边界价值的过细模块。

完成门槛：没有死代码和双实现；从文档可以完成一个新应用的最小组装；CI 可阻止边界倒退。

## 9. 每阶段统一执行流程

1. 写出本阶段行为清单和不允许变化的基线。
2. 先增加或确认测试，再移动生产代码。
3. 只迁移一个依赖方向，保持提交可构建。
4. 运行单元、集成、导出和真实 UI 测试。
5. 对比产物与截图，记录任何有意变化。
6. 通过后再删除旧入口；未通过则回退本阶段，不让兼容层无限保留。
7. 阶段完成后更新本文档的状态、证据和剩余风险。

重构期间避免同时修改同一领域的用户功能；必须修复 P0 时，用独立小批次处理并补回归测试。

## 10. 测试与验收矩阵

### 10.1 单元测试

- 数据分隔、编码、列类型和数值规范化；
- 标签解析、字体语义、换行和转义；
- 自动范围、端点刻度、error bar 边界；
- 图例布局、marker/line/error bar 图例样本；
- 项目迁移和验证。

### 10.2 服务契约测试

- 导入事务成功与失败均保持状态一致；
- 删除单个数据源只移除关联对象；
- 清除全部同时清除数据、系列和旧坐标配置；
- 删除后重新导入等同于全新项目导入；
- 导出只读取不可变快照，不修改当前文档。

### 10.3 集成与产物测试

- CSV/TSV/TXT 及项目支持的其他格式矩阵；
- 多文件、同一文件多 XY、手动数据和重复测量；
- PNG/PDF/SVG 的尺寸、字体、裁边和透明度；
- 旧项目打开、保存、重新打开 round-trip。

视觉与导出基线分三级，避免跨平台字体栅格化、DPI、GPU 和抗锯齿差异造成脆弱测试：

1. Level 1：文档与布局数值快照，严格比较 axis rect、legend rect、文字 baseline、曲线数据边界等。
2. Level 2：`DisplayList` / scene 快照，严格比较绘制命令、坐标、样式和层级。
3. Level 3：最终渲染图像 diff，使用明确容差，并按平台维护必要的字体/渲染基线。

优先由 Level 1 和 Level 2 判断几何与语义是否回归；逐像素图像一致不作为跨平台首要判据。

### 10.4 真实 UI 测试

- macOS 普通窗口、全屏；
- Windows 普通窗口、最大化；
- 触控板与鼠标的侧栏拖拽、画布缩放、图例/文字拖拽；
- 多个工具窗口同时打开、切回主图不自动关闭；
- 长文件名、窄侧栏、底部自然横向滚动；
- Spotlight 中只有一个已安装应用且版本、图标正确。

真实 UI 测试继续以 `docs/INSTPLOT_STUDIO_END_TO_END_MANUAL_QA_PLAN.md` 为主清单，并为每个新模块补充对应证据。

## 11. 建议使用的工具

工具只用于验证和提速，不代替架构判断：

- `rust-analyzer`：引用迁移和模块边界检查；
- `cargo clippy --locked --all-targets -- -D warnings`：静态检查；
- `cargo test --locked --workspace` 与文档测试：基础门槛；
- `cargo nextest`：在测试量增加后加速执行；
- `cargo llvm-cov`：识别高风险流程的空白覆盖；
- `cargo semver-checks`：公共 crate API 稳定后检查破坏性变化；
- `cargo machete` 或同类工具：发现无用依赖；
- `cargo deny`：许可证、重复依赖和安全策略检查；
- 固定输入的布局/DisplayList snapshot 与带容差视觉 diff：分层检查语义、几何和最终渲染变化。

新增工具应固定版本并先在 CI 试用，不为了工具本身修改产品行为。

## 12. 风险与控制

### 12.1 UI 移动造成交互漂移

控制：先截图和真实操作基线；纯迁移阶段不调整布局；全屏和最大化单独验收。

### 12.2 多份状态继续分叉

控制：先落实状态职责和 controller 事务；禁止新代码跨层直接写入。

### 12.3 项目文件不兼容

控制：冻结 schema；保留代表性旧项目；迁移必须可重复并有 round-trip 测试。

### 12.4 crate 过多导致构建和理解成本增加

控制：严格执行 crate 提升规则；先模块后 crate，不根据预设架构图一次性创建所有 crate。

### 12.5 视觉基线大量波动

控制：迁移阶段不批准批量视觉变化；有意变化必须逐项说明并更新对应基线。

### 12.6 重构拖延现有产品修复

控制：P0 用户问题可独立修复；重构按小阶段交付，不维持长期不可运行分支。

## 13. 总体验收标准

全部计划完成时必须同时满足：

- `main.rs` 只负责启动、平台初始化与顶层组装，不含业务规则；约 500 行仅作为软目标。
- 核心数据、文本、文档、渲染和导出模块不依赖 `egui` 或 Studio 二进制；满足提升规则后形成的 crate 同样遵守此约束。
- 用户可见文字不散落在业务逻辑中。
- 文件导入只有一套解析/规范化管线和一个应用事务入口。
- 预览、PDF、PNG 和 SVG 共享同一解析与布局结果。
- 项目格式兼容现有文件，旧 fixture 全部通过；项目在原始数据源缺失时仍能完整恢复保存时的图形。
- 第二个应用不复制 Studio 源码即可完成导入、编辑和导出。
- 全部自动测试、端到端人工测试、全屏/最大化矩阵和安装验收通过。
- 不再有生产核心路径依赖 `spike` 命名实现。
- 文档能够说明如何新增一种格式、一个编辑器、一个导出后端和一个新产品。

## 14. 优先级与建议起步方式

建议顺序：

1. P0：Phase 0 行为基线。
2. P0：Phase 1 二进制机械拆分。
3. P0：Phase 2 状态所有权与应用事务。
4. P0：Phase 3 统一数据管线。
5. P1：Phase 4 文本、Phase 5 文档和 Phase 6 渲染/导出。
6. P1：Phase 7 UI 设计系统。
7. P2：Phase 8–10 复用证明、清理和 CI。

第一轮只执行 Phase 0，不立即大规模移动文件。先产出依赖地图、行为清单和失败保护，再根据证据确定 Phase 1 的最小迁移面。

## 15. 执行记录

- [x] Phase 0：行为基线与依赖地图（证据：`docs/INSTPLOT_STUDIO_PHASE0_BASELINE.md`）
- [x] Phase 1：拆分二进制但不改变行为（证据：`docs/INSTPLOT_STUDIO_PHASE1_MECHANICAL_SPLIT.md`）
- [x] Phase 2：集中状态所有权与应用事务（证据：`docs/INSTPLOT_STUDIO_PHASE2_APPLICATION_TRANSACTIONS.md`）
- [x] Phase 3：提取统一数据管线（证据：`docs/INSTPLOT_STUDIO_PHASE3_DATA_PIPELINE.md`）
- [x] Phase 4：提取统一文本与标签语义（证据：`docs/INSTPLOT_STUDIO_PHASE4_TEXT_PIPELINE.md`）
- [x] Phase 5：拆分文档领域、命令和项目存储（证据：`docs/INSTPLOT_STUDIO_PHASE5_DOCUMENT_PROJECT_DOMAINS.md`）
- [x] Phase 6：产品化渲染与导出管线（证据：`docs/INSTPLOT_STUDIO_PHASE6_RENDER_EXPORT_PIPELINE.md`）
- [x] Phase 7：统一 UI 设计系统与窗口策略（证据：`docs/INSTPLOT_STUDIO_PHASE7_UI_DESIGN_SYSTEM.md`）
- [x] Phase 8：形成可复用 UI 外壳（证据：`docs/INSTPLOT_STUDIO_PHASE8_REUSABLE_UI_SHELL.md`）
- [x] Phase 9：用第二个应用证明复用成立（证据：`docs/INSTPLOT_STUDIO_PHASE9_SECOND_APPLICATION.md`）
- [x] Phase 10：清理、文档和持续集成（证据：`docs/INSTPLOT_STUDIO_PHASE10_FINALIZATION.md`）

默认决策：新增边界先作为现有 crate 内部 module；满足提升规则后才成为仓库内部 workspace crate，暂不公开发布。示例应用只用于证明边界，待接口稳定后再决定是否作为独立产品基础。
