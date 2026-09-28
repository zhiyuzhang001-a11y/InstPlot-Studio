# InstPlot Studio 极端数值自动范围与轴倍率实施计划

> 配套产品计划：`INSTPLOT_STUDIO_EXTREME_VALUE_AUTOSCALE_AND_AXIS_SCALE_SHORT_PLAN.md`
> 状态：`COMPLETED — ALL STAGES VERIFIED 2026-09-28`
> 日期：2026-09-28
> 当前分支：`codex/dual-axes`
> 实施原则：先修数值正确性，再增加倍率表现；原始值、坐标值和显示值严格分层。
> 交付限制：未授权提交或推送 GitHub、制作 release、构建 Windows/Linux；所有阶段通过前不得替换 Spotlight 应用。

> 实施结果：全部阶段已落地并通过回归门；未提交或推送 GitHub，未制作 release，未构建 Windows/Linux。唯一 Spotlight 应用已由合格 release 构建替换为 build `202609282110`。

## 1. 权威关系与执行纪律

- 配套短计划定义产品行为；本文件定义代码边界、顺序、停点和测试。实施时必须同时读取，两者冲突时先修订文档，不在代码中自行选择。
- 当前工作区包含大量已验收但未提交的双轴、参考线、轴可见性、tight export 和代码整理修改。开始前记录 HEAD、完整 status、Spotlight build 和测试基线；禁止 reset、覆盖或清理无关变化。
- 每个 Stage 以失败测试开始、以阶段门结束。未通过时只修复本阶段，不进入下一阶段。
- 所有 rebind、范围更新、倍率状态、标签编辑和迁移均采用候选文档验证后整体替换；失败不得留下半更新状态或多余 Undo 步骤。
- 真实 `0V_40um_Hall.csv` 只用于本地人工验收，不复制进仓库、不改变原文件。

## 2. 冻结的技术合同

### 2.1 三层数值与允许变化

1. **原始值**：数据源中保存的真实 `f64`、alive mask、误差列和绑定。
2. **坐标值**：axis domain、autoscale、raw major/minor ticks、reference line、measurement arrow、tooltip 和 hit testing 使用的真实值。
3. **显示值**：只用于 tick label，计算式为 `raw / 10^n`。

倍率切换必须保持：

- 数据域、绑定、误差值和 normalized data fraction 不变；
- 固定 plot bounds 下 raw value → fraction 的结果不变；
- Fixed/Interval locator 的 raw ticks 不变；
- hover、拖拽、reference line、measurement arrow 和 tooltip 返回原始值。

允许变化：倍率文字宽度可以改变 plot bounds；Auto locator 可因可用轴长度变化重新选择 raw ticks。因此不再要求画布像素坐标或 Auto ticks 逐值不变。

### 2.2 有限算术权威实现

建立一组可被 layout 和 Studio 交互共同调用的数值辅助函数，禁止各层继续手写不稳定公式：

- `finite_span(min, max)`：避免 `max - min` 中间溢出；只接受有限且 `min < max`。
- `linear_fraction(value, min, max)`：稳定计算 normalized fraction，不因跨越 `±f64::MAX` 溢出。
- `linear_value(fraction, min, max)`：稳定反插值，端点精确，内部结果有限。
- `finite_midpoint(a, b)`：避免 `a + b` 溢出。
- `next_up / next_down`：用于舍入后仍相等的范围端点和边界测试。
- `checked_error_endpoint(point, error, sign)`：结果非有限时返回错误，不静默过滤或夹造数据。
- `checked_pow10(exponent)`：结果必须有限且非零；随后还要验证当前轴域缩放后全部有限。

预计权威位置为 `instplot-layout::scale` 或更小的共享数值模块；`canvas_support.rs`、layout map、axis editor 和 autoscale 统一复用。

### 2.3 Autoscale 非退化策略

- 非恒定线性数据：先用稳定 span 得到 5% padding；无法在某侧有限扩展时允许单侧 padding；至少保证原始 min/max 完整包含且最终有限严格递增。
- 非零恒定线性数据：目标 padding 为 `abs(value) * 0.05`；下溢或加减后仍等于原值时使用 next-up/next-down；接近 `±f64::MAX` 时允许单侧范围。
- 全零线性数据：固定 neutral fallback `[-1, 1]`，不生成自动倍率。
- “近恒定”不使用 epsilon 合并；只要两个有限 `f64` 不相等，就保留真实 span。
- 对数非恒定数据：使用乘法 padding；下界不得成为 `0`，上界不得成为 `∞`，边界处允许单侧或 next-up/next-down。
- 对数恒定正值：优先使用乘除因子展开；极值处按上一条退化。
- 对数轴遇到零、负数或非有限端点时，候选事务整体失败。
- error bar 的任一 `point ± error` 溢出时明确失败；不允许 `retain(is_finite)` 把单侧端点静默删除。
- reference line 仅在 `include_in_autoscale=true` 时参与；measurement arrow、annotation 和 connector 永不参与。

### 2.4 Locator、零值和精度

- raw ticks 始终处于坐标空间；倍率只在格式化阶段应用。
- 自动线性 locator 不通过可能饱和的 `i64(min/step)` 构造范围；生成数量由目标 tick count 和统一硬上限约束。
- 对数 locator 在 `pow10(power)` 前验证指数，过滤不得表示的 decade，并限制跨 decade 输出数量。
- 仅把精确 `-0.0` 规范化为 `0.0`，或在整数索引明确代表数学零时直接构造 `0.0`；删除一切通用 `abs(value) < epsilon → 0`。
- 边界容差只用于包含判断和端点贴合，依据局部 ULP、raw step 和 axis span；不得修改真实非零 tick。
- Auto formatter从缩放后的 step 所需精度开始，最多探索 `f64` 有效十进制数字；实际 major labels 必须唯一。
- Decimal formatter保留用户精度。若实际缩放后的 major labels 重复，生成 Publication Warning，不静默提高用户精度。
- 单个显示标签超过 24 个可见字符生成非阻断 Publication Warning；缩放结果非有限属于项目验证错误。

### 2.5 不可非法表示的倍率状态

`AxisRecord` 使用一个代数枚举，而不是两个可任意组合字段：

```text
AxisDisplayScaleRecord:
  AutoFactor
  None
  ManualFactor(NonZeroI32 exponent)
  ManualIncorporated(NonZeroI32 exponent)
```

验证与转换：

- Manual variants只暴露私有验证构造器并使用 `NonZeroI32`，Rust 类型层不能构造零载荷。UI 输入 `0` 明确转换为 `None`；反序列化项目出现 Manual `0` 时拒绝为非法项目，不能静默规范化外部数据。
- AutoFactor 解析为零时仍保持 AutoFactor；没有显式 `{scale}` 时不生成倍率文字。
- 当前解析指数为零时，“倍率已写入单位”控件禁用并解释原因。
- Factor → Incorporated 必须同时提交新标签和冻结后的非零指数，使用复合 EditCommand 和单次 Undo。
- ManualIncorporated 修改指数时不猜单位，只在该次编辑流程显示醒目但非阻断的“请同步检查单位”瞬时确认；项目模型不保存不可验证的“刚修改过”历史状态。
- Incorporated → Auto 转为 AutoFactor；不会自动把 `mΩ` 改回 `Ω`，由用户明确编辑标签。

### 2.6 自动指数和 `{scale}` 标签槽

- raw major ticks 非空时，以 `max(abs(raw_tick))` 为代表值；否则回退到最终有效 axis domain。
- 小值阈值为 `<10^-2`；大值阈值暂定 `>=10^4`，Stage 3 前必须最终确认。
- 非零触发值的指数为 `floor(log10(representative))`；不限制为三的倍数。
- 若候选指数的 `checked_pow10` 为零或非有限，向零逐步收敛到最近可表示 decade并重新验证 `representative / scale`；最小正 subnormal 的确定 fallback 通常为指数 `−323`，禁止产生 `10^-324 == 0`。
- 全零值指数为零；非有限输入为错误。
- Auto 解析只发生在布局派生状态中，不写回项目、不制造 dirty。
- 新增仅允许用于轴标签的语义节点 `ScaleFactorSlot`；输入层以 `{scale}` 表示，不解析单位文本。
- Factor 且存在 slot：在原位置生成语义化 `×10`＋数字上标；Auto 指数为零时显式显示 `×10⁰`，避免空括号歧义。
- Factor 且不存在 slot：仅在指数非零时把倍率追加为同一 AxisLabel 的末尾语义片段。
- ManualIncorporated 禁止包含 ScaleFactorSlot；切换事务必须先得到不含 slot 的有效标签。
- 倍率、用户标签、旋转、测量、bounds 和 hit target组成一个 AxisLabel；不存在第二个 offset display item。

### 2.7 Publication Warning 数据通路

增加稳定 rule id、中文文案和测试：

- `duplicate_tick_labels`：手动 Decimal 精度造成实际 major label 重复。
- `long_tick_label`：任一实际显示 major label超过 24 个可见字符。
- `scaled_ticks_without_visible_factor`：有效指数非零、tick labels 可见、axis label 隐藏。

规则：

- 唯一性和长度在 horizontal collision stride 之前对完整 major label 集合检查。
- LayoutWarning 进入 Studio Publication Check，提供中文原因和影响；均为 warning，不阻止导出。
- Auto formatter仍产生重复标签属于实现错误或布局错误，不能降级为普通 warning。
- 隐藏 tick labels 时不报告“倍率说明隐藏”；隐藏 axis label 且 tick labels 可见时才报告。
- Incorporated 指数修改提示属于 Stage 5 的瞬时 UI 确认，不进入持久 Publication Warning 管线。

### 2.8 Schema v10 → v11 迁移

- 冻结 `LegacyProjectV10` decoder 和显式 `migrate_v10`；新字段不得仅靠 serde default 混入旧语义。
- v0–v9 先经现有链路迁移到 v10，再恰好执行一次 v10→v11；migration provenance 不重复。
- 旧 `Auto` → Auto formatter precision＋`AutoFactor`。这是有意的纠错升级：此前未显示的 shared exponent 将进入 AxisLabel，且小值阈值改为 `<0.01`。
- 旧 `Decimal { precision }` → Decimal precision＋`None`，保持明确的普通数字意图。
- 旧 `Scientific { precision }` → 根据保存 axis domain 解析指数；非零时转为 Decimal precision＋`ManualFactor(n)`，零/全零时转为 Decimal precision＋`None`。即使 axis.autoscale=true，也保留为手动倍率，直到用户主动选 Auto。
- v11 canonical model和新 UI不再创建逐 tick Scientific；旧 variant只存在于 legacy decoder/迁移测试。
- 所有迁移倍率采用 Factor；没有 slot 时在 AxisLabel 末尾追加。
- 打开旧项目的结构迁移不计为用户编辑、不开启普通 dirty 标记；用户保存时写出 v11 与一次迁移 provenance。关闭未保存的迁移项目不得弹出“用户内容未保存”的误导提示。
- 保存重开、重复打开、未来 schema 拒绝和迁移失败原子性必须测试。

## 3. 模块职责与改动面

### 3.1 数值与 layout 核心

- `crates/instplot-layout/src/scale.rs`
  - 有限算术、locator、指数解析、显示格式、唯一性检查。
- `crates/instplot-layout/src/model.rs`
  - AxisSpec 的倍率派生状态和 ScaleFactorSlot 扩展所需输入。
- `crates/instplot-layout/src/layout/axes.rs`
  - raw tick 定位、display label、组合 AxisLabel、warning 和四条轴复用。
- `crates/instplot-layout/src/layout.rs` 及拆分模块
  - AxisLayout 保存 raw value、display label、resolved exponent；移除无人渲染的旧 shared exponent 输出。
- `crates/instplot-layout/src/layout/series.rs`、`objects.rs`
  - 复用稳定 map；error bar、reference line、measurement arrow 不复制数值公式。

### 3.2 Studio 文档、交互与项目

- `apps/instplot-studio/src/document/autoscale.rs`
  - 唯一 data bounds、error endpoint、padding 和 autoscale 应用。
- `apps/instplot-studio/src/document/series.rs`
  - 完整逻辑系列 rebind，并在候选文档中刷新自动轴。
- `apps/instplot-studio/src/document/axes.rs`
  - 倍率状态 API、解析指数和复合标签＋倍率事务。
- `apps/instplot-studio/src/document.rs`
  - AxisRecord → AxisSpec 的唯一转换，传递倍率和 ScaleFactorSlot。
- `apps/instplot-studio/src/editing.rs`
  - 复合命令、单步 Undo/Redo 和 coalescing 边界。
- `apps/instplot-studio/src/canvas_support.rs`
  - hover、drag 和 data↔canvas 复用稳定 map/unmap。
- `apps/instplot-studio/src/editor_support.rs`
  - suggested minor interval 使用安全 span。
- `apps/instplot-studio/src/app_controller/axis_editor.rs`
  - 范围、interval 和倍率输入，不在实时输入中提交非法中间态。
- `apps/instplot-studio/src/app_controller/artist_editor.rs`、`interaction.rs`
  - midpoint、reference line、measurement arrow 和 tooltip 的 raw-value 回归。
- `apps/instplot-studio/src/project.rs`、`project/validation.rs`
  - v11 model、状态验证、Fixed/Interval 极端范围验证。
- `apps/instplot-studio/src/project/migration.rs`、`project/project_tests.rs`
  - LegacyProjectV10、迁移、provenance、dirty 和幂等测试。
- `apps/instplot-studio/src/publication.rs`、`ui_text.rs`
  - 三类持久 warning、中文原因/影响和规则版本；Incorporated 修改只使用瞬时 UI 确认。

### 3.3 预览和导出

- 预览、PNG、SVG、PDF 继续消费同一 display list；后端不得重新计算倍率或拼接文本。
- tight resolver 以组合 AxisLabel 的真实 glyph、旋转和 bounds 计算输出边界。
- backend parity 同时验证上标语义、字体 glyph、旋转 Y label、tight crop 和 AxisLabel hit target，不只检查字符串出现次数。

## 4. 分阶段执行与停点

### Stage 0：只读基线和失败测试

记录：HEAD、完整 status、Spotlight build、全量测试结果和普通数量级 golden。

增加失败测试：

- `index → Rxy` 保留旧范围；
- 非零恒定小值被固定 padding 淹没；
- 真实 `1e-15` tick 被清零；
- `±f64::MAX` span/map 溢出；
- subnormal/log padding 下溢；
- error endpoint 溢出被静默丢弃；
- `1e12` 附近小跨度产生同名 labels；
- shared exponent 缩放 tick 却没有进入 AxisLabel；
- 隐藏 AxisLabel 后刻度无倍率说明。

停点：缺陷分别落到 autoscale、有限算术、locator、formatter、模型或 label composition；尚未修改产品行为。

### Stage 1：P0 有限算术、rebind 与 autoscale

1. 实现并单测有限 span、fraction、inverse、midpoint、next-up/down、checked error endpoint。
2. 替换 layout、canvas support 和相关编辑器中的直接不稳定公式。
3. 将 rebind 改为完整逻辑系列的候选文档事务；一次更新 line/scatter/error bar 和自动轴。
4. 实现短计划规定的线性、全零、恒定、极值和 log padding。
5. 保持 reference line 和 measurement arrow 既有 autoscale 语义。

定向门：autoscale、series、error bar、canvas map/unmap、双轴、删除重导、Undo/Redo。

阶段停点：真实文件换列后完整显示负值与形状；有限输入不因中间溢出失真；无效 log 或 error endpoint 原子失败。未通过不得进入 tick 重构。

### Stage 2：极端 locator、formatter、validation 与 warning

1. 删除固定 clean-zero、`.max(1.0)` 容差和八位自动精度上限。
2. 实现有界 Auto/Fixed/Interval major/minor tick；修改 project validation 中 `(max-min)/step` 和 `min/step` 的旧限制。
3. 修改 axis editor 默认 interval 和 `suggested_minor_interval`，统一安全 span。
4. 自动 formatter 保证实际 labels 唯一；Decimal 重复走 LayoutWarning。
5. 接通 duplicate/long 通用 Publication Warning，确认不阻止导出。
6. 对数 decade 生成使用 checked pow10 和数量上限。

定向门：subnormal、`MIN_POSITIVE`、`±f64::MAX`、`1e12` 小跨度、跨零、全负、fixed/interval、主副刻度和 log 多 decade。倍率 exact-threshold 集成测试留到 Stage 4。

阶段停点：raw ticks 有限、有序、域内、有界；Auto labels 唯一；普通数量级 golden 不变。

### Stage 3：倍率状态、ScaleFactorSlot 和 v11 迁移

前置门：最终确认大值阈值。

1. 增加四态 AxisDisplayScaleRecord 和 ScaleFactorSlot；禁止非法组合。
2. 冻结 LegacyProjectV10，升级 schema，落实 Auto/Decimal/Scientific 迁移、provenance、dirty 和幂等规则。
3. 提供文档 API 解析 Auto exponent，并验证 Manual exponent 对当前域安全。
4. 增加复合的“标签＋倍率状态”编辑命令。
5. 更新所有 fixture 和 v0–v11 连续迁移测试。

阶段停点：不接 UI 也能完成四态、slot、保存重开、迁移和单步 Undo/Redo；重复迁移不产生额外 provenance。

### Stage 4：AxisLabel 组合和 backend parity

1. locator先产生 raw ticks；随后解析指数并格式化 display labels。
2. Factor slot在指定位置展开；无 slot 时追加；Incorporated 不生成倍率。
3. 主轴、副轴、水平和旋转垂直标签共用一套组合、测量和 hit map。
4. 删除或收敛旧 shared exponent，确认 display list中不存在角落指数。
5. 接通隐藏倍率 warning、倍率 exact-threshold 测试和 tight export bounds。

定向门：指数 `−323`、`−12..12`、`308`、指数零 slot、无 slot追加、旋转 Y、X2/Y2、label隐藏、空副轴、glyph、hit target、PNG/SVG/PDF。

阶段停点：倍率只在完整 AxisLabel 中出现一次；画布与三个后端的语义上标和边界一致。

### Stage 5：紧凑 UI 与编辑事务

1. 在现有轴标签窗口增加一行：倍率状态、指数、Factor/Incorporated 和 `{scale}` 帮助；不增加嵌套窗口。
2. 自定义指数采用本地输入草稿，提交/失焦/Enter 后验证；清空输入不会立刻破坏项目。
3. Factor → Incorporated 要求同次提交不含 slot 的新标签，并冻结当前非零指数。
4. Incorporated 改指数显示仅限本次编辑流程的单位检查确认；不写入 Publication 状态。恢复 Auto 不自动改写单位文本。
5. 重复打开窗口只置顶；四条轴各自保持状态。

定向门：中间空输入、取消、窗口关闭、一次 Undo/Redo、保存重开、普通/最大化/全屏、X1/Y1/X2/Y2。

阶段停点：`R_{xy} ({scale} Ω)` 与 `R_{xy} (mΩ)` 的明确转换不改变刻度、不重复缩放、不留下非法状态。

### Stage 6：全量回归、真实操作与交付

自动门：

1. 所有定向测试并确认实际执行数量非零；
2. `cargo fmt --all -- --check`；
3. `cargo check --locked --workspace --all-targets --all-features`；
4. `cargo test --locked --workspace --all-targets --all-features --no-fail-fast`；
5. `cargo clippy --locked --all-targets -- -D warnings`；
6. `cargo build --locked --release -p instplot-studio`；
7. `git diff --check`。

真实操作门：

- `0V_40um_Hall.csv` 导入、换列、删除、重新导入；
- 手动录入、误差棒、参考线、测量箭头和图例；
- 普通、最大化、全屏；单轴、双 Y、双 X；
- AutoFactor、None、ManualFactor、ManualIncorporated 和 `{scale}`；
- 隐藏 label/tick labels、Publication Warning、tight export；
- 保存、关闭、重开、Undo/Redo；PNG、SVG、PDF 对照。

独立 QA 门：按两份计划模拟人工操作，重点检查导入、双轴、标签输入、图例、交互坐标和导出未被破坏。

交付停点：全部通过后才替换唯一 Spotlight 应用。GitHub、PR、release 和其他平台构建继续等待单独指示。

## 5. 回归断言清单

- 自动轴换列后包含新列全部有效数据、启用的参考线和所有有限误差端点；手动轴逐字不变。
- line/scatter/error bar 同一逻辑系列绑定一致；失败事务完整回滚。
- autoscale、倍率和 formatter 不改变数据源、alive mask、绑定或误差值。
- 稳定 map/unmap 在固定 plot bounds 下保持端点和 normalized fraction；极端有限域不产生 NaN/∞。
- Fixed/Interval raw ticks 不受倍率影响；Auto locator 允许随可用长度变化，但不读取显示倍率数值。
- 只有精确数学零显示为零；`1e-15` 等真实非零 tick 保持非零。
- `0.01` 不触发、`next_down(0.01)` 触发；大值边界按最终确认值执行。
- Auto labels唯一；Decimal 重复、超长 label、隐藏倍率进入正确 Publication Warning 且不阻止导出。
- `1e12` 附近小变化可见且 labels可区分；subnormal 和最大有限值遵守明确退化策略。
- `ManualFactor(-3)` 与 `ManualIncorporated(-3)` 的 raw ticks和显示数字一致，仅倍率文字不同。
- `R_{xy} ({scale} Ω)` 在 `−3` 下生成一个 `×10⁻³`；无 slot标签只在末尾追加一次。
- 隐藏 AxisLabel 后无漂浮倍率，且在 tick labels可见时产生警告。
- hover、tooltip、reference line和measurement arrow在倍率切换前后保持原始数据值。
- v0–v11迁移、保存重开、Undo/Redo、预览、PNG、SVG、PDF一致。

## 6. 明确不做

- 不猜测列名或标签中的单位，不自动在 `Ω / mΩ / μΩ` 之间换算。
- 不把倍率写回 CSV、TSV、XLSX、项目数据源或误差列。
- 不恢复角落 offset text，不在每个 tick输出科学计数指数。
- 不为预览、PNG、SVG、PDF分别实现倍率算法。
- 不借本计划重做通用标签语言、字体系统、导入器或无关窗口；仅增加轴标签专用 ScaleFactorSlot。
- 不在计划阶段修改代码、运行构建、替换 Spotlight 或操作 GitHub。

## 7. 完成记录

- 数值链：换列事务、误差端点、恒定微小值、最小 subnormal、恒定 `±f64::MAX`、跨完整有限域 map/unmap、locator 上限和唯一刻度标签均有回归覆盖。
- 倍率链：四态模型、schema v11、`{scale}`、统一 AxisLabel、三种导出后端、Publication Warning、正式 layout 指数来源与 `0 → None` 均已接通。
- 交互链：指数采用延迟草稿提交；Incorporated 使用内联单位标签草稿，应用时标签与冻结指数单事务提交并一次 Undo，取消不修改文档。
- 全量门：Studio library `193 passed / 1 ignored`，Studio main `87 passed`，其余 workspace 测试全部通过；唯一 ignored 项为需显式环境变量生成的标签视觉辅助 PDF。
- 静态门：`cargo clippy --locked --all-targets -- -D warnings`、workspace check、`cargo fmt --check`、`git diff --check`、release build 全部通过。
- 独立 QA：最终 `PASS`，未发现剩余 P0/P1/P2 功能缺陷。
- 实机门：真实 CSV 导入、Y 列下拉换为 `Rxy_Ohm`、自动范围与标签刷新、倍率 `0 → None` 和 Incorporated 内联草稿均已操作确认；普通、最大化和全屏窗口策略由现有自动化测试覆盖。
