# InstPlot Studio 双轴 Spine、参考线与测量箭头实施方案

> 状态：`COMPLETED / INDEPENDENT QA PASS`
> 建立日期：2026-09-27
> 产品与验收基准：[`INSTPLOT_STUDIO_REFERENCE_AND_MEASUREMENT_PLAN.md`](INSTPLOT_STUDIO_REFERENCE_AND_MEASUREMENT_PLAN.md)
> 双轴基准：[`INSTPLOT_STUDIO_DUAL_AXES_DESIGN.md`](INSTPLOT_STUDIO_DUAL_AXES_DESIGN.md)、[`INSTPLOT_STUDIO_DUAL_AXES_IMPLEMENTATION_PLAN.md`](INSTPLOT_STUDIO_DUAL_AXES_IMPLEMENTATION_PLAN.md)
> 执行分支：`codex/dual-axes`；起始提交 `4d45bab76787b44d9a66a775bb5fd54fb73d380a`；项目格式已升级到 schema 9。
> 交付授权：最终验收后更新唯一 Spotlight 应用并上传 GitHub；不创建 release、不打发布标签、不执行 Windows 构建。

## 1. 两份文档如何配套

本方案只回答“按什么顺序修改、修改哪些模块、运行哪些检查、何时停止”。产品行为、交互和边界以产品与验收基准为准，不能只看本方案实施。

每个阶段必须执行以下闭环：

1. 阅读产品基准中对应章节；
2. 核对本阶段允许修改的模块和明确非目标；
3. 先写或更新测试，再修改实现；
4. 运行定向测试和阶段门；
5. 回到产品基准逐项核对行为；
6. 记录命令、结果、未决问题和证据；
7. 只有技术门和行为门同时通过，才进入下一阶段。

若代码现实与产品基准冲突，暂停实现，先更新设计决策和本方案，不允许在代码里形成未记录规则。

## 2. 全局操作规范

### 2.1 Git 与工作区

- 开始前记录 `git status --short`、当前分支和 `HEAD`。
- 保留所有不属于本阶段的现有修改，不覆盖、不暂存、不提交用户内容。
- 禁止 `git reset --hard`、递归删除工作区、强制 checkout 或覆盖未知文件。
- 每个阶段只允许修改本阶段列出的职责；发现无关问题只记录，不顺手重构。
- 阶段通过后可以建立本地检查点提交；未通过不得提交为完成状态。
- 最终门通过后只 push 源码分支并建立 PR；不合并、不打 tag、不建立 release。
- 开发和自动测试期间不替换 `$HOME/Applications/InstPlot Studio.app`；最终门通过后按用户授权替换并验证唯一安装。

### 2.2 架构纪律

- Project Document 是所有已提交对象的唯一持久事实来源；AppState 只允许保存一次手势的临时 `ToolDraft`，Esc 丢弃，释放后通过一个 `EditCommand` 原子提交。
- 所有修改必须经过现有编辑命令、撤销/重做和重建 resolved scene 的链路。
- 数据坐标到画布坐标只在布局层解析；UI 不复制坐标变换。
- 预览、命中和 PDF/SVG/PNG 共用 Display List 几何。
- 单轴模式不显示颜色控件；其 spine、刻度线、刻度数字和轴标签从现有深灰有意统一为纯黑，除此之外不得改变布局或交互。
- 文字连接线、测量箭头和参考线使用不同领域对象，不通过特殊 ID 或隐藏字段互相模拟。
- 不新建第二套 palette；spine 和参考线复用现有 `PaletteRegistry`，测量箭头固定黑色且无颜色控件。
- 新增公共字段、枚举、Artist 类型或 schema 迁移必须同时有校验、round-trip 和旧项目测试。

### 2.3 测试纪律

- 不删除现有测试、不放宽断言、不忽略 Clippy、不用快照整体替换掩盖几何回归。
- 浮点测试使用明确容差；颜色和枚举使用精确断言。
- 交互测试同时检查持久状态、resolved scene 和可撤销性，不能只检查按钮响应。
- 导出测试检查同源几何和语义，不依赖人工观察作为唯一证据。
- 每阶段先运行定向测试，再运行相邻模块回归；模型或布局接口变化后运行 workspace 全量门。
- 所有按名称过滤的 `cargo test` 命令必须先用 `-- --list` 或测试目标清单确认至少匹配一个测试，并在阶段报告记录非零执行数量；“0 tests, ok”视为失败，不能作为阶段证据。

## 3. 执行前基线命令

Phase 0 启动后依次执行：

```bash
git status --short
git branch --show-current
git rev-parse HEAD
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release -p instplot-studio
cargo run --release --locked -p instplot-studio -- --product-info
git diff --check
```

基线报告记录当前提交和修改、工具链版本、测试结果、单轴/双轴/标注对象基线。当前安装应用只用于对比，不覆盖，也不作为源码事实来源。任一命令失败时停止，先判断既有问题或环境问题。

## 4. 阶段执行表

### Phase 0：基线、范围冻结和任务清单

状态：`DONE`

设计输入：产品基准第 1、2、8、9 节。

执行：

- 运行第 3 节全部命令。
- 建立 QA 记录，记录各阶段状态、提交、命令和证据。
- 固定单轴、双 Y、双 X、图例、误差棒、annotation connector 和 reference line 的现有基线。
- 列出当前工作区用户修改并划出任务允许触碰的文件。
- 确认不包含四轴、reference band、tick/label 联动着色或 GitHub 发布。

允许修改：计划和 QA 记录；不得修改产品代码。

完成门：基线全绿，工作区归属明确，产品基准无未决冲突。

### Phase 1：schema 9、领域类型和迁移

状态：`DONE`

设计输入：产品基准第 2、4 节。

主要文件：

- `apps/instplot-studio/src/project.rs`
- `apps/instplot-studio/src/project/migration.rs`
- `apps/instplot-studio/src/project/validation.rs`
- `apps/instplot-studio/src/project/project_tests.rs`
- `apps/instplot-studio/src/project/fixture.rs`
- 必要的 re-export 文件

执行顺序：

1. 先增加 schema 8 → 9 迁移 fixture 和失败测试。
2. 定义双轴 spine 颜色字段，默认 `object-black`；处理 `AxisAppearanceRecord` 因 String 字段不再可 `Copy` 的调用点。
3. 定义连接线箭头头型；文字连接线方向吸附不持久化，因此不增加连接线方向约束字段。
4. 定义 `MeasurementArrow` 持久对象：双端点、规范 AxisBinding、固定黑色 StrokeStyle、端点箭头、头型、自由/水平/垂直约束、专属 `Option<label_id>` 和 point 单位 label offset。
5. 给现有 `ReferenceLine` 增加 `include_in_autoscale`：旧项目迁移为 `true`，新建对象默认 `false`。
6. 增加数值范围、轴绑定、颜色 ID、退化线段、箭头尺寸和可选 label 引用校验。
7. 保持现有 JSON 修补架构：各旧版本完成原有修补后统一调用 `upgrade_value_to_v9`，显式注入黑色 spine、旧连接线开放箭头、旧参考线 `include_in_autoscale=true` 及规范化绑定，最后只反序列化一次。
8. 更新迁移 provenance/诊断，不得继续声称最终只迁移到 schema 8；保留一份真实 schema 8 fixture，并补齐 schema 0–9、JSON round-trip、未知字段和非法数据测试。
9. 固定标签所有权：一个 measurement arrow 独占一个可选 label；清空只删 label，删除箭头清理 label，复制功能不实现。

禁止：连接 UI、改变布局、修改现有单轴显示。

定向门：

```bash
cargo test --locked -p instplot-studio project::
cargo fmt --all -- --check
git diff --check
```

阶段门：workspace 全量测试和 Clippy。

完成证据：迁移矩阵、真实 schema 8 fixture、round-trip、标签无 orphan、旧项目不丢字段、非法 X2/Y2 与非规范参考线绑定均被拒绝或迁移规范化。

最低必有测试名包含：`schema_eight_migrates_to_nine_with_scientific_guide_defaults`、`reference_line_binding_is_normalized_for_orientation`、`measurement_arrow_label_ownership_round_trips`。

建议本地提交：`feat: add schema 9 drawing primitives`

### Phase 2：文档操作、休眠与自动范围

状态：`DONE`

设计输入：产品基准第 2.2、2.3、4.3 节。

主要文件：

- `apps/instplot-studio/src/document.rs`
- `apps/instplot-studio/src/document/objects.rs`
- `apps/instplot-studio/src/document/autoscale.rs`
- `apps/instplot-studio/src/document/document_tests.rs`
- `apps/instplot-studio/src/semantic.rs`
- `apps/instplot-studio/src/publication.rs`
- `apps/instplot-studio/src/document/datasets.rs`

执行顺序：

1. 增加 reference line 和 measurement arrow 的新增、修改、删除入口；本轮不实现复制。
2. 所有操作经过 `EditCommand`，形成单步撤销/重做。
3. 实现副轴关闭后的有效可见性：休眠但不删除、不改持久 `visible`。
4. 新 reference line 默认不参与范围；旧对象按迁移字段保持原行为。
5. ReferenceLine 绑定规范化：竖直为 `(selected X, Y1)`，水平为 `(X1, selected Y)`；有效可见性和 autoscale 只检查方向相关轴。
6. 显式启用 autoscale 时只扩展相关轴；对数轴拒绝非正值。
7. measurement arrow、annotation connector 和 label 不进入数据 autoscale。
8. 在图例校验、逻辑系列、`document/datasets.rs`、`semantic.rs` 和 `publication.rs` 中绝对排除 reference line 与 measurement arrow，不能只做到新建时默认不加入。
9. 扩展 palette 变更事务：spine 和 reference line 的旧系列色按序号映射，语义色保留，失败回退 `object-black`；X1/X2、Y1/Y2 独立处理，并覆盖撤销/重做。

禁止：在 UI 层直接写 `ProjectDocument`，依据位置猜测轴绑定。

定向门：

```bash
cargo test --locked -p instplot-studio document::
cargo test --locked -p instplot-studio autoscale
cargo fmt --all -- --check
git diff --check
```

完成证据：新增/删除/撤销/重做、休眠恢复、autoscale 开关和图例排除均有合同测试。

建议本地提交：`feat: add document operations for scientific guides`

### Phase 3：布局模型、几何、命中与多后端

状态：`DONE`

设计输入：产品基准第 2.3、3.2–3.4 节。

主要文件：

- `crates/instplot-layout/src/model.rs`
- `crates/instplot-layout/src/layout.rs`
- 当前 layout 测试模块
- `apps/instplot-studio/src/document.rs`
- `crates/instplot-render` 中确有必要的 Display List 消费代码
- 导出合同测试

执行顺序：

1. 扩展 layout axis appearance 接收解析后的 spine 颜色，并把默认 axis ink 从 `[45, 50, 55]` 统一为纯黑。
2. 只允许双 Y 左右、双 X 上下 spine 使用自定义色；单轴 spine、刻度线、刻度数字、轴标签和双轴非相关边强制黑色。
3. 扩展 annotation connector：开放/实心头和四种端点模式；水平/垂直/45°只在拖拽手势中吸附，不进入持久布局约束。
4. 增加 measurement arrow 正式布局结构、双端点、专属标签、层级和独立命中角色。
5. 约束逻辑在数据坐标与画布坐标之间只实现一次；处理线性、对数和反向轴。
6. 将 reference line 从普通 data series 语义中分离为独立 collection 或确定性的 pre-series pass，并增加 `SelectableRole::ReferenceLine`。
7. reference line 使用 axes clipping、位于曲线后方，并通过 line-like `path_proximity` 命中，不能用覆盖整个 axes 的矩形。
8. measurement arrow 区分起点、终点、线身和标签角色；定义重叠端点、箭头头和标签的 z-order 优先级。
9. measurement arrow 线段与箭头头裁切在 axes；标签允许进入 figure 空白但不扩大导出边界，超界产生布局警告。
10. 预览、PDF、SVG、PNG 继续消费同一 Display List。

几何约定：文字连接线和自由箭头的 Shift 0°/45°/90°只在屏幕空间临时吸附；测量箭头持久水平表示两端数据 Y 相等，持久垂直表示数据 X 相等；首版不实现任意持久角度或通用长度。退化或不可映射端点返回明确错误，不生成 NaN。

定向门：

```bash
cargo test --locked -p instplot-layout
cargo test --locked -p instplot-studio --test render_export_contract
cargo test --locked -p instplot-studio --test document_domain_contract
cargo fmt --all -- --check
git diff --check
```

阶段门：workspace 全量测试和 Clippy。

完成证据：四种箭头端点、两种头型、纯黑 axis ink、spine 边映射、参考线 path proximity、箭头四类命中、裁切、z-order 和多后端合同通过。

建议本地提交：`feat: render spine colors and scientific guides`

### Phase 4：双轴 Spine 颜色界面

状态：`DONE`

设计输入：产品基准第 2.1、3.1 节。

主要文件：

- `apps/instplot-studio/src/app_controller.rs`
- `apps/instplot-studio/src/editor_support.rs`
- `apps/instplot-studio/src/palette.rs`（只复用通用组件）
- `apps/instplot-studio/src/ui_text.rs`
- `apps/instplot-studio/src/app_tests.rs`
- `apps/instplot-studio/src/editing.rs`
- `apps/instplot-studio/src/app_transactions.rs`

执行顺序：

1. 在现有双轴设置中复用曲线颜色控件。
2. 双 Y 只显示 Y1 左/Y2 右，双 X 只显示 X1 下/X2 上。
3. 单轴无可见控件，spine、刻度线、刻度数字和轴标签全部强制纯黑。
4. 关闭副轴隐藏控件并保持休眠颜色；重开恢复。
5. 双 X 和双 Y 的颜色分别保存，不互相覆盖。
6. 扩展而非假定既有 palette 迁移：`object-black`/语义色保留，系列色按序号映射，失败回退黑色；四个休眠 spine 分别迁移且不串色。
7. 参考线使用同一颜色控件和映射规则；测量箭头不显示颜色控件并固定黑色。

定向门：

```bash
cargo test --locked -p instplot-studio spine
cargo test --locked -p instplot-studio palette
cargo test --locked -p instplot-studio repeated_tool_commands_raise
cargo fmt --all -- --check
git diff --check
```

完成证据：单轴无控件；双轴颜色往返、休眠恢复、项目重开和多格式导出一致。

建议本地提交：`feat: color active dual-axis spines`

### Phase 5：参考线创建工具与对象窗口

状态：`DONE`

设计输入：产品基准第 3.4 节。

主要文件：

- `apps/instplot-studio/src/app_ui.rs`
- `apps/instplot-studio/src/app_state.rs`
- `apps/instplot-studio/src/app_controller.rs`
- `apps/instplot-studio/src/canvas_support.rs`
- `apps/instplot-studio/src/editor_support.rs`
- `apps/instplot-studio/src/ui_text.rs`
- `apps/instplot-studio/src/app_tests.rs`
- `apps/instplot-studio/src/editing.rs`
- `apps/instplot-studio/src/app_transactions.rs`

执行顺序：

1. 增加水平/垂直参考线入口和明确的活动工具状态。
2. 双轴下显式选择相关轴，默认主轴，不按点击边缘猜测；竖直线规范化为 `(selected X, Y1)`，水平线规范化为 `(X1, selected Y)`。
3. 单击按数据坐标新增；连续添加直到 Esc、切换工具或再次触发命令。
4. 垂直线只水平拖动，水平线只垂直拖动；拖拽使用按 AxisBinding 感知的统一反向坐标 helper，覆盖副轴、对数轴和反向轴。
5. 双击或重复编辑命令置顶同一个窗口，不新建重复窗口。
6. 对象窗口包含方向、值、相关轴、曲线同源颜色、线型、线宽、autoscale 和删除；对象窗口使用独立 dispatch，不依赖普通 data series 路由。
7. 对启用 autoscale 的参考线，拖拽时冻结起始轴变换并只更新 runtime preview，释放时原子提交后再 autoscale，避免反馈抖动。
8. AppState 为连续添加保存明确 ToolDraft；Esc 丢弃草稿，单击完成后通过 EditCommand 提交。
9. 全屏使用现有嵌入回退；退出全屏后恢复统一窗口行为。

定向门：

```bash
cargo test --locked -p instplot-studio reference
cargo test --locked -p instplot-studio canvas
cargo test --locked -p instplot-studio repeated_tool_commands_raise
cargo fmt --all -- --check
git diff --check
```

完成证据：连续添加四条竖直虚线；精确位置、拖拽、轴绑定、删除、撤销、休眠和 autoscale 均通过。

最低必有测试名包含：`vertical_reference_line_uses_only_selected_x_axis`、`horizontal_reference_line_uses_only_selected_y_axis`、`reference_line_autoscale_drag_commits_without_feedback`。

建议本地提交：`feat: add interactive reference line tool`

### Phase 6：文字连接线与独立测量箭头

状态：`DONE`

设计输入：产品基准第 3.2、3.3 节。

主要文件与 Phase 5 相同，并包含必要的 document/layout 测试。

执行顺序：

1. 整理文字连接线编辑区，不改变起点依附文字墨迹边界的规则；连接线只增加箭头头型和 Shift 临时吸附，不保存永久角度。
2. 将 modifier 快照随 CanvasDragEvent 传递，测试拖动途中按下/松开 Shift，controller 不跨帧读取全局按键状态。
3. 增加测量箭头工具：按下确定起点、拖拽确定终点、释放提交，Esc 取消。
4. 小于命中阈值的短点击不创建退化对象。
5. 增加起点、终点、线身和标签的独立 handle/role，使用 binding-aware 反向坐标 helper 完成端点拖拽和整体移动。
6. 精确编辑起终点 X/Y；水平模式编辑 `ΔX`，垂直模式编辑 `ΔY`，自由模式角度只读。首版不实现通用长度和任意持久角度。
7. 增加专属可选标签、数学输入、换行、中点默认位置和 point 单位 label offset；清空只删 label，删除箭头清理 label。
8. 测量箭头固定黑色，不提供颜色控件；一个“测量箭头”入口在创建前选择普通指示或水平双向预设。
9. 创建过程使用 transient ToolDraft，Esc 丢弃，释放后一个 EditCommand 原子提交；短点击不写入项目。
10. 对端点直接拖拽进行 axes clamp；精确输入允许范围外并提示。线和箭头头裁切，标签按设计允许进入 figure 空白。
11. measurement arrow 使用独立 context-editor dispatch；重复命令只置顶，普通、最大化、全屏均可操作和找回。

定向门：

```bash
cargo test --locked -p instplot-studio annotation
cargo test --locked -p instplot-studio connector
cargo test --locked -p instplot-studio measurement
cargo test --locked -p instplot-layout arrow
cargo fmt --all -- --check
git diff --check
```

完成证据：重建两条水平双向箭头和 `Δt` 标签；端点、线身、标签、Shift、精确输入、撤销和项目重开均通过。

最低必有测试名包含：`measurement_arrow_draft_commits_atomically`、`measurement_arrow_handles_have_distinct_hit_roles`、`connector_shift_snap_is_transient`、`measurement_arrow_label_cleanup_has_no_orphan`。

建议本地提交：`feat: add constrained measurement arrows`

### Phase 7：跨功能回归与导出验收

状态：`DONE`

设计输入：产品基准第 6 节完整测试矩阵。

执行矩阵：单轴纯黑轴信息；双 Y、双 X spine 颜色与 palette 映射；双轴开关往返；导入、录入、混合数据和误差棒；三种曲线图例；三种图例位置；主/副/休眠轴参考线和箭头；线性、对数、反向、自动和手动范围；100 条参考线；普通、最大化、全屏；项目重开、PDF、SVG、PNG。

全量门：

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
cargo test --locked --workspace --doc
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release -p instplot-studio
cargo run --release --locked -p instplot-studio -- --product-info
git diff --check
```

完成证据：QA 报告逐项记录输入、操作、预期、实际、截图或导出、命令和提交。

建议本地提交：`test: verify scientific guide workflows`

### Phase 8：独立 agent 验收

状态：`DONE`

前提：Phase 0–7 全部通过且工作区状态已记录。

- 独立 agent 只读取设计基准、实施方案和已构建的本地 release，不把实现过程结论当作事实。
- 以真实用户操作为主，覆盖普通、最大化和全屏。
- 必测参考图重建、单轴纯黑轴信息、双轴颜色/palette 往返、参考线规范绑定与连续添加、测量箭头约束和黑色固定样式、项目重开和三种导出。
- 必须回归导入、录入、误差棒、图例、标签和窗口置顶。
- 独立 agent 不修改代码；问题返回可复现步骤、严重度和证据，由主任务修复后重新验收。

完成门：无 P0/P1；P2 必须修复或经用户明确接受并记录。结论只能是 PASS、FAIL 或 BLOCKED。

### Phase 9：用户确认后的安装与版本控制

状态：`DONE`

只有用户明确要求替换 Spotlight 后执行：

```bash
python3 scripts/install_studio_macos.py
```

验证固定路径、bundle ID、版本、构建号、签名、图标、实际启动/退出和 Spotlight 唯一结果。不得覆盖正在运行且含未保存内容的应用；若用户已声明当前界面无需保存，可按该规则关闭后替换。

用户已授权在本机验收通过后上传 GitHub 源码分支并建立 PR；本轮不合并、不打 tag、不创建 release，也不执行 Windows 构建。

## 5. 共同停止条件

出现以下任一情况立即停止当前阶段：

- schema 8 或更早项目无法打开、字段丢失或保存后不可逆改变；
- 单轴出现颜色控件，或 spine、刻度线、刻度数字、轴标签任一不是纯黑；
- X2 与 Y2 同时启用，或休眠对象被迁移到主轴；
- 双轴关闭导致颜色、参考线、箭头或连接线被删除；
- palette 切换出现无效颜色 ID、内部 `object-*` 名称、marker/line 颜色分离、spine/参考线未映射或休眠轴串色；
- 新对象进入图例、数据系列或不相关的出版规范系列检查；
- 参考线含非规范 AxisBinding，或因无关维度被错误休眠；
- 测量箭头出现颜色选择、非黑色，或使用含义不明的通用长度/持久任意角度；
- 新 reference line 默认扩展自动范围，或旧参考线迁移后悄悄改变已有范围；
- 拖拽产生 NaN、无穷值、越界对象或无法撤销的中间状态；
- 预览和任一导出格式出现不同几何、颜色、箭头或文字；
- 小窗口无法移动、无法置顶、点击画布自动关闭或全屏不可找回；
- 为通过测试需要删除断言、跳过 Clippy、复制坐标算法或建立第二份状态；
- 出现无关依赖升级、大范围重构、Windows/Linux 构建或未授权部署。

## 6. 回退与恢复

- 每阶段通过后记录本地提交、测试命令和 QA 证据。
- 阶段失败时只修复或撤回该阶段任务修改，不触碰此前检查点和用户内容。
- 不使用破坏性 Git 命令；通过明确补丁或新的修复提交恢复。
- 中断后先读取两份配套文档和 QA 记录，再从最后已通过阶段继续。
- 若模型设计必须改变，先更新产品基准和本方案，再编码。
- `.app` 不是成果事实来源；源码、迁移、测试和报告必须足以重新生成应用。

## 7. 最终完成定义

只有同时满足以下条件才能标记 `COMPLETED`：

1. Phase 0–8 全部通过，并有命令和操作证据；
2. 产品基准全部验收项完成，无未记录行为差异；
3. schema 0–9 迁移和 schema 9 round-trip 通过；
4. 单轴无颜色设置，spine、刻度线、刻度数字和轴标签始终纯黑；
5. 双 X、双 Y 的 spine 颜色、关闭休眠和重开恢复正确；
6. 参考线使用曲线同源色板且默认黑色，测量箭头固定黑色；二者能够重建目标示例并正确保存、导出；
7. 导入、录入、误差棒、图例、标签和窗口行为没有 P0/P1 回归；
8. 独立 agent 给出 PASS；
9. Phase 9 按本轮授权完成 Spotlight 唯一安装与 GitHub 源码上传，不包含 release、tag、合并或 Windows 构建。

## 8. 当前进度

- [x] 产品讨论完成；
- [x] 产品与验收基准建立；
- [x] 实施方案建立；
- [x] Phase 0：基线、范围冻结和任务清单；
- [x] Phase 1：schema 9、领域类型和迁移；
- [x] Phase 2：文档操作、休眠与自动范围；
- [x] Phase 3：布局模型、几何、命中与多后端；
- [x] Phase 4：双轴 Spine 颜色界面；
- [x] Phase 5：参考线创建工具与对象窗口；
- [x] Phase 6：文字连接线与独立测量箭头；
- [x] Phase 7：跨功能回归与导出验收；
- [x] Phase 8：独立 agent 真实窗口验收；
- [x] Phase 9：Spotlight 唯一安装验证及 GitHub 源码上传完成；PR #2 已建立，未合并、未发布。
