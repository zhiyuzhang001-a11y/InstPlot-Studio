# InstPlot Studio 倍率感知范围编辑与 autoscale 生命周期实施计划

> 配套产品计划：`INSTPLOT_STUDIO_SCALED_AXIS_RANGE_EDITOR_SHORT_PLAN.md`
> 状态：`COMPLETED — ALL STAGES PASS`
> 日期：2026-09-28
> 当前分支：`codex/dual-axes`
> 首要目标：任何成功的数据变化都使受影响轴完整显示当前全部可见数据；倍率感知编辑只改变输入/显示尺度，不改变 raw 数据和导出几何。
> 交付限制：未授权提交或推送 GitHub、制作 release、构建 Windows/Linux；全部阶段通过前不替换 Spotlight 应用。

## 1. 权威关系与执行纪律

- 配套短计划定义产品行为、不变量和验收范围；本文件只定义代码边界、实施顺序、阶段门和停止条件。实施时必须同时阅读两份文件；若冲突，先修订文档，不在代码中自行选择。
- 当前工作区包含大量已有且用户需要保留的修改。Stage 0 必须记录 HEAD、完整 `git status --short`、当前 Spotlight build 和测试基线；禁止 reset、checkout 覆盖、清理无关文件或重写用户变更。
- 每个 Stage 先增加能准确失败的定向测试，再实现，最后通过本阶段门。阶段门未通过时只修当前阶段，不继续向后堆叠变更。
- 数据源、logical series、轴绑定、误差列、autoscale 和 layout 都在候选文档中完成，验证成功后才整体替换。单文件或单次录入提交失败不得留下半成品状态。
- 不重构 Undo，不新增文件格式，不增加窗口或成功提示，不重写 autoscale padding/数值边界算法。只修正现有路径的统一状态衔接和倍率感知编辑。
- 测试数据使用临时目录或现有 fixtures；不改写用户原始文件，不把临时人工 QA 数据加入仓库。

## 2. 冻结的技术契约

### 2.1 两类变化必须分开

1. **数据语义变化**：创建、替换、删除数据，更换 XY/误差列，轴迁移，数据曲线显隐。这些操作可以使受影响轴恢复 autoscale/自动刻度。
2. **纯视觉或项目生命周期变化**：颜色、marker、linestyle、图例、文字、参考线样式，以及打开、保存、导出、Undo/Redo。这些操作不能被当成新数据上下文。

### 2.2 受影响轴的唯一定义

- 根据操作前后的 logical-series `AxisBinding` 和可见性，计算受影响轴并集。
- 换列或更换数据只影响当前绑定轴；Y1→Y2 影响 Y1 和 Y2，X1→X2 影响 X1 和 X2。
- 每条受影响轴在候选文档的最终状态上聚合全部有效且可见的曲线、有限误差端点和 `include_in_autoscale=true` 的参考线；不只查看被修改的曲线。
- 完全隐藏的曲线修改时不立即改变范围，重新显示时才进入受影响轴集合。图例显隐不影响范围。
- 单轴、双 X（X1/X2/Y1）、双 Y（X1/Y1/Y2）分别处理；不构造同图 X2+Y2 状态。

### 2.3 受影响轴恢复契约

数据事务成功时，对每条受影响轴：

- `autoscale = true`；
- `locator = Auto { target_count: 6 }`；
- `minor_interval = None`；
- `formatter = Auto`；
- 保留 display-scale 模式、线性/对数轴类型、自定义标签、字体、字号、labelpad、spine、刻度外观、颜色和线宽。

空轴：

- 线性轴 `0…1`，对数轴 `1…10`；
- 保持当前单/双轴模式和副轴占位契约；
- 仅删除最后一个数据源时使用现有整图清空逻辑。

### 2.4 倍率感知编辑契约

- raw document 是唯一真实数值；编辑器使用 `display = raw / 10^n`、`raw = display × 10^n`。
- 所有范围、Interval 和 Fixed ticks 入口共用一个无状态换算 helper；不得在 UI、EditCommand 和 document 多层重复乘除。
- AutoFactor 只消费当前 document revision 和 AxisIdentity 对应的正式 resolved exponent，不在 UI 重算指数。
- 每条轴的编辑会话独立冻结指数。未提交草稿不能被新 layout 重解释。
- 数据事务成功后取消受影响轴的范围/刻度草稿和冻结指数，用新 layout 刷新；事务失败则保留原草稿。
- 换算与 Fixed ticks 整体提交必须有限、原子且可 Undo；不修改 `{scale}` 和轴标签排版。

### 2.5 事务、历史和批量语义

- 单个文件和一次录入数据提交各自原子。
- 批量导入继续按文件原子，允许有效文件成功、无效文件报错；最终 autoscale 使用本批最终成功接受的状态。
- 文件导入与录入数据保持现有历史基线/rebase 语义。
- 换列、误差列、删除等现有编辑命令保持现有 Undo 步数。Undo/Redo 只恢复完整快照，不重新触发 autoscale。

## 3. 模块职责与预计改动面

### 3.1 Document 领域层

- `apps/instplot-studio/src/document/autoscale.rs`：保留现有 data bounds/padding 算法；增加“对指定 AxisIdentity 恢复自动状态并刷新”的权威通路，不再让各入口直接改字段。
- `apps/instplot-studio/src/document/series.rs`：logical series 的创建、rebind、误差列和轴迁移在操作前后计算受影响轴。
- `apps/instplot-studio/src/document/datasets.rs`：删除、数据源同步、依赖清理和空轴处理。
- `apps/instplot-studio/src/document/axes.rs`：现有轴记录 API、空轴契约和倍率编辑所需的原子轴更新。
- `apps/instplot-studio/src/document/autoscale_api.rs`：对外暴露意图明确的 API，区分“只刷新已自动轴”与“数据语义变化后强制恢复受影响轴”。

### 3.2 应用事务与数据入口

- `apps/instplot-studio/src/app_controller/data_workflow.rs`：首次录入、编辑已保存组、新增 XY/重复测量，以及用户自定义轴标签保护。
- `apps/instplot-studio/src/import_flow.rs`：首次导入、追加、同文件替换、删空后重导和自动列名标签。
- `apps/instplot-studio/src/app_transactions.rs`：候选 session/document、按文件原子、批量部分成功、preview/layout 验证和 rebase 语义。
- 系列可见性和轴绑定入口：复用 document helper，不复制 autoscale 规则。

### 3.3 范围编辑与 UI 状态

- `apps/instplot-studio/src/app_controller/axis_editor.rs`：X1/Y1/X2/Y2 范围、主/副间隔和 Fixed ticks 的 display-scale 换算，继续使用延迟提交。
- `apps/instplot-studio/src/editor_support.rs`：共用 raw/display helper、有限验证和输入文本格式化；如果 helper 更适合 layout 共享，放入已有数值模块而不创建重复实现。
- `apps/instplot-studio/src/app_state.rs`：按 AxisIdentity 隔离的冻结指数、dirty draft 和文档 revision 关联状态；这些均不序列化。
- 数据事务成功后的 UI outcome 处理：只取消受影响轴草稿并同步新 layout，不新增界面。

### 3.4 测试位置

- `apps/instplot-studio/src/document/document_tests/axes_and_data.rs`：受影响轴 helper、全部可见贡献者、误差端点、空轴、双 X/双 Y。
- `apps/instplot-studio/src/document/document_tests/series.rs`：rebind、轴迁移、可见性、样式操作不触发 autoscale。
- `apps/instplot-studio/src/app_tests/data_workflows.rs`：录入数据、导入、重导、混合数据源、批量部分成功、自定义标签和 rebase。
- 范围编辑器单元/交互测试：指数冻结、raw/display 往返、Fixed ticks 原子性、草稿取消和 Undo 数量。
- 现有导入格式：每类格式保留 smoke test，文本族和工作簿族各选代表进行完整 autoscale 生命周期测试，不做全组合笛卡尔积。

## 4. 分阶段执行与停点

### Stage 0：只读基线与入口清单

1. 记录 HEAD、分支、完整工作区状态和 Spotlight build。
2. 执行当前完整 Rust 基线，单独记录已知失败，不用本任务修复无关问题。
3. 列出所有数据语义变化入口和 X1/Y1/X2/Y2 现有范围/刻度编辑入口，形成代码级检查表。
4. 保存现场复现结论：录入数据更新成功、旧手动 Y1 范围未失效、旧固定主刻度仍存在。

阶段门：尚未修改产品代码；入口清单能对应短计划 6.3 的每一项，已知工作区变更得到保护。

### Stage 1：先锁定失败契约

新增失败测试：

- 手动范围＋固定主/副刻度＋固定 formatter → 修改录入数据到新数量级。
- 手动范围 → 同文件重导、换 X、换 Y、同时换 XY。
- 多曲线共轴时修改一条，范围仍包含其他全部可见曲线。
- Y1→Y2、X1→X2 后旧轴和新轴同时正确；某副轴变空时使用空轴契约。
- 隐藏曲线修改不扰动当前范围，重新显示时重算。
- X/Y/XY error 添加、替换、删除；误差端点溢出原子失败。
- 参考线纳入/不纳入 autoscale；移动参考线不强制打开手动轴的 autoscale。
- 自定义轴标签后修改录入数据不被覆盖；自动列名标签仍随换列更新。
- 一好一坏批量导入、fit 先于来源文件、导入＋录入数据混合。
- 数据提交时存在受影响轴未提交的倍率范围草稿。

阶段门：每个失败都对应明确合同，不是测试框架或 fixture 问题；现有通过测试未被修改成错误预期。

### Stage 2：Document 层统一受影响轴与强制 autoscale

1. 建立受影响 AxisIdentity 的去重集合和操作前/后绑定并集 helper。
2. 建立唯一的“数据语义变化后恢复自动状态并重算” API，同时保留“只刷新已开启自动的轴”现有 API，避免参考线等视觉对象误用强制通路。
3. 恢复 autoscale、locator、minor interval 和 formatter，保留 display-scale、axis scale、语义标签和外观。
4. 聚合全部最终可见贡献者，处理空轴、误差端点和参考线。
5. 保持对数轴类型；不合法新数据使候选事务失败，不静默转线性。

定向门：Document 单元测试覆盖手动→自动、全部可见贡献者、旧/新轴并集、空轴、双 X/双 Y、误差和参考线。

停点：helper 必须无 UI 依赖，不修改 autoscale padding 算法，无关轴的完整 `AxisRecord` 字节级不变。

### Stage 3：接入导入与录入数据事务

1. 录入数据首次提交、编辑已有组、新增 XY/重复测量统一调用 Document helper。
2. 移除会绕过新契约的 `sync_datasets_without_autoscale` 用法，或将其明确限定为候选文档的中间步骤，最终必须且只能执行一次强制 autoscale。
3. 保护自定义轴标签；仅自动列名标签随换列变化。
4. 文件导入、同文件替换、追加和删空后重导统一调用同一 helper。
5. 批量导入保持按文件候选事务；有效文件可提交，无效文件不破坏已成功文件，最终文档使用最终成功数据计算范围。
6. 保持现有 rebase 语义，不为导入/录入额外生成 Undo 命令。

定向门：录入数据小→大、大→小、重复测量/SD/SE、全清空后重录，文件首导/追加/重导/多文件，混合数据源，批量部分成功，自定义标签。

停点：现场问题必须在无 UI 的 app transaction 测试中通过；数据内容、数据源稳定 ID、配色、marker、linestyle、图例和保存绑定不得变化。

### Stage 4：接入曲线绑定、误差、可见性和删除

1. rebind X/Y/数据源使用旧/新轴并集，更新完整 logical series。
2. 轴迁移同时更新离开轴和到达轴；空副轴恢复空轴范围。
3. 误差列添加/替换/删除后重算对应 X/Y 轴；有限误差端点全部纳入。
4. 数据曲线显隐改变范围贡献者；图例显隐和纯样式操作不触发。
5. 删除单组/多组/文件/依赖 fit/最后数据源后，分别处理其余贡献者、空轴和整图清空。
6. 参考线继续只在 `include_in_autoscale=true` 且轴已自动时刷新；其编辑不得强制打开手动轴。

定向门：rebind、双轴迁移、多曲线共轴、隐藏/显示、X/Y/XY error、参考线、删除与 Undo/Redo 快照。

停点：无关轴不变；每个可 Undo 操作保持现有步数；Undo/Redo 不再次 autoscale。

### Stage 5：倍率感知范围与刻度编辑

1. 实现唯一 raw/display 换算 helper，覆盖指数 `0`、常规正负指数、可表示边界和原子失败。
2. 列出并逐一接入现有 X1/Y1/X2/Y2 范围、Interval 和 Fixed ticks 入口；不借机为副轴新增现在不存在的 locator 编辑器。
3. AutoFactor 使用正式 resolved exponent；ManualFactor/ManualIncorporated 使用冻结指数；None/指数零使用 raw 尺度。
4. 每轴独立管理焦点、dirty draft、Escape、Enter/失焦提交和会话结束；不序列化 UI 草稿。
5. Fixed ticks 整体解析、整体换算、整体验证并单次提交；任一 token 失败则文档不变。
6. 保证窗口刷新、切换轴/倍率/轴模式、项目打开和 Undo/Redo 不将旧草稿按新指数提交。

实施入口矩阵说明：现行 X1/Y1/X2/Y2 快速编辑器公开范围、Interval 主刻度和副刻度，均已接入统一换算。Fixed locator 当前没有对外启用的编辑入口，按本计划“不新增现有不存在的 locator 编辑器”的约束保留项目读取/保存/布局和数据变化后恢复 Auto，不将旧的未调用完整编辑器误报为已交付 UI。

定向门：`-3.9…9.5`、主/副 interval、Fixed ticks、log 正值、极端指数、大偏置小跨度、no-op、失败原子性和 Undo 次数；分别验证双 X 与双 Y，不创建四轴状态。

停点：raw document、数据绑定、误差端点、参考线和导出 display list 不因编辑尺度被二次缩放。

### Stage 6：数据事务与倍率草稿的竞态衔接

1. 在数据事务 outcome 中传递或可确定受影响 AxisIdentity 的信息，不通过窗口标题或控件状态反推。
2. 事务成功：取消受影响轴未提交草稿、结束冻结指数，等新 layout 后刷新系数文本。
3. 事务失败：保留 document/session 和原 UI 草稿，错误信息不改写输入。
4. 不受影响轴的焦点、草稿和冻结指数保持不变。

定向门：成功/失败数据提交，单轴/双 X/双 Y，受影响/无关轴，焦点、dirty draft、冻结指数和 Undo/rebase。

### Stage 7：全量回归、真实操作与本地交付

1. 运行定向测试、全 workspace 测试、format、check 和 Clippy。
2. 检查所有现有文件格式 smoke tests；文本族和工作簿族各执行一条完整数据生命周期。
3. 真实 UI 操作：
   - 手动范围＋固定刻度 → 修改录入数据到新量级；
   - 手动范围 → 同文件重导/换 XY；
   - 多曲线共轴、误差棒、隐藏/显示和双 X/双 Y 轴迁移；
   - `×10⁻³` 范围/间隔/Fixed ticks 编辑及中途数据更新；
   - 普通、最大化和全屏下的现有窗口操作。
4. 保存重开、Undo/Redo、PNG/SVG/PDF 导出；确认打开/保存/导出不触发新 autoscale，导出几何不受编辑尺度影响。
5. 使用独立 agent 按短计划 6.4 和本计划 Stage 7 执行最终黑盒验收，不仅审查代码。
6. 只有前述门全部通过，才进行 release 构建并替换唯一本地 Spotlight 应用；替换后再做一次启动与核心现场回归。

阶段门：所有自动测试和真实 UI 验收通过；独立 agent 结论 PASS；Spotlight 中的 build 与已验证源码一致。

## 5. 标准验证命令

从仓库根目录执行：

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo test --locked --workspace
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release -p instplot-studio
```

Clippy 必须使用仓库 `rust-toolchain.toml` 钉住的 Rust 1.98.0。如 Cargo shim 行为不清楚，先执行：

```bash
cargo clippy --version
rustup run 1.98.0-aarch64-apple-darwin cargo clippy --version
rustup component list --toolchain 1.98.0-aarch64-apple-darwin --installed
```

定向测试可使用测试名过滤，但不能代替 Stage 7 的全 workspace 门。

## 6. 停止、回退与升级条件

出现以下任一情况时停在当前 Stage，不继续实施：

- 需要改变短计划中“数据变化强制恢复受影响轴”的产品契约。
- 需要新增项目 schema、导入格式、窗口、按钮或持久化 UI 状态。
- 需要重构 Undo/历史基线或改变批量导入允许部分成功的语义。
- 需要改变 autoscale padding、对数轴有效性、误差端点或参考线的现有数值规则。
- 无关轴、无关数据源、图例、颜色、marker、linestyle、导出几何或既有保存关系发生变化。
- 不能通过候选文档＋整体替换保证失败原子性。
- 修复必须删除或改写用户已有的无关变更。

若触发停止条件，报告确切代码边界、失败测试和需要用户决定的产品问题；不使用局部补丁绕过契约。

## 7. 最终完成定义

只有同时满足以下条件，计划才可标记为 `COMPLETED`：

1. 现场问题“手动范围后修改录入数据不显示”可稳定通过。
2. 短计划 6.3 的每个入口都调用共用契约，没有单入口例外补丁。
3. 手动范围、旧 locator/minor interval/formatter 在数据变化后正确恢复自动，全部可见数据和有限误差端点在范围内。
4. 多曲线共轴、隐藏/显示、双 X/双 Y、轴迁移、空轴、参考线和混合数据源均通过。
5. 倍率感知范围、主/副 interval 和 Fixed ticks 可正确往返，不改变 raw 数据、误差几何或导出。
6. 数据提交与倍率草稿竞态正确；无关轴状态不泄漏，失败时原子回滚。
7. 所有定向门、全 workspace 门、真实 UI 验收和独立 agent 检查全部 PASS。
8. 本地 Spotlight 应用仅在合格 release 构建后替换，并完成替换后启动验证。
9. 未制作 release，未构建 Windows/Linux。用户随后已授权“需要的话可以提交 GitHub”；是否提交以最终本地验收后的仓库状态为准。

## 8. 最终实施记录

- Stage 0–6 已完成：统一指定轴强制 autoscale、导入/录入/重导、rebind、误差列、轴迁移、显隐和删除接入；倍率编辑读取正式 resolved exponent；受影响轴草稿按 outcome 精确清理。
- 同一路径文件数值变化现在在数据区结构可唯一匹配时沿用稳定身份；数据区增删、关联缺失或匹配含糊仍走原有原子拒绝保护。
- 数据变化恢复 autoscale、Auto locator、自动副刻度和 Auto formatter，同时保留自定义标签、display-scale、轴类型及外观；仅 Y error 等局部变化不扰动无关轴。
- 全 workspace、format、check、严格 Clippy、repository hygiene、diff check、B5 19 项验证和 release build 全部 PASS。
- 独立 agent 对安装应用执行真实普通/最大化/全屏操作，覆盖极小值、误差棒、倍率范围/主副间隔、数据修改、同路径重导、双 Y、保存重开及 PNG 导出，结论 PASS。
- 最终唯一 Spotlight 应用：`$HOME/Applications/InstPlot Studio.app`，版本 `0.1.0`，build `202609282305`，bundle ID `com.instplot.studio`，arm64、签名和 Spotlight 唯一索引均通过。
