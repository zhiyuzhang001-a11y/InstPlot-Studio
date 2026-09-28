# InstPlot Studio 极端数值自动范围与轴倍率短计划

> 状态：`COMPLETED — VERIFIED 2026-09-28`
> 日期：2026-09-28
> 优先级：P0（自动范围与数值正确性）＋ P1（出版级轴倍率）
> 已确认案例：`0V_40um_Hall.csv` 从 `index` 切换到 `Rxy_Ohm` 后仍保留约 `0–100` 的旧 Y 范围，真实数据约为 `−0.1883–0.0713`。
> 范围：修复换列以及极小、极大、大偏置小跨度数据的自动范围、映射和刻度；增加可保存、可编辑的轴显示倍率，并把倍率作为轴标签的一部分排版。
> 非目标：猜测或换算 `Ω / mΩ / μΩ` 等单位；改变原始数据；本阶段不提交 GitHub、不制作 release。

> 完成证据：全 workspace 测试通过；严格 Clippy、format、diff check、workspace check 和 release 构建通过；独立功能 QA 最终 PASS；Spotlight build `202609282110` 已安装，并用真实 `0V_40um_Hall.csv` 验证从 `time_s` 切换到 `Rxy_Ohm` 后立即重算为约 `−0.2–0.08` 的完整范围。

## 0. 当前实现已确认的问题

- `rebind_series` 更换 X/Y 列后没有刷新仍处于自动模式的轴，旧范围会压平新数据。
- 线性恒定值使用至少 `0.5` 的固定 padding，会淹没非零小量级。
- 自动格式化只在绝对值 `< 0.001` 或 `>= 10000` 时缩放，并把指数强制为三的倍数；与本计划的小值阈值和规范化指数不一致。
- `clean_zero` 把绝对值 `< 1e-12` 的真实非零刻度直接变成零。
- tick 边界过滤以 `1.0` 作为容差最低尺度，极小范围会接受错误边界。
- 自动精度最多八位，大偏置小跨度可能产生多个同名刻度。
- 当前 shared exponent 会参与刻度缩放，却没有作为完整轴标签的一部分可靠渲染。
- `maximum - minimum`、padding、线性 map/unmap、对数扩展及 `point ± error` 在接近 `f64` 极限时仍可能溢出、下溢或静默丢值。

## 1. 固定产品规则

### 1.1 Autoscale、映射和误差端点必须对有限 `f64` 安全

- 切换 X/Y 列、改变系列轴绑定、增加或删除误差棒、系列显隐、删除后重新导入时，所有仍处于自动范围状态的相关轴立即重新计算；手动范围轴保持不变。
- 自动范围包括可见数据、明确启用 autoscale 的参考线和已启用的误差棒端点；测量箭头、普通文字和连接线不参与 autoscale。
- 数据边界、跨度、padding、线性 map/unmap 和 midpoint 使用避免中间溢出的稳定算法；不能直接假设 `maximum - minimum`、`minimum + fraction * span` 永远有限。
- 误差端点 `point ± error` 若无法表示为有限 `f64`，整个编辑或 autoscale 事务明确失败并保留旧状态；不得静默丢端点，也不得伪造夹界数据。
- 非恒定数据以真实跨度增加视觉边距；即使 padding 因舍入消失，也要用相邻可表示浮点数保证有限且严格递增的范围。
- “近恒定”不按任意 epsilon 合并：只要 `minimum != maximum`，就保留真实跨度。非零恒定值以其数量级展开；靠近 `±f64::MAX` 时允许有限的单侧展开。
- 全零线性数据没有可推导量级，采用明确的 neutral fallback `[-1, 1]`，且不自动生成倍率。
- 对数轴只接受正有限值，恒定值使用乘法展开；靠近最小正值或最大有限值时使用有限的 next-up/next-down 或单侧展开，禁止扩展到 `0` 或 `∞`。
- autoscale、倍率和 formatter 都不得修改原始数据、alive mask、绑定、误差大小或物理单位。

### 1.2 自动倍率阈值与代表值

- `0.1`、`0.01`、`100` 和 `1000` 默认直接显示。
- 自动倍率以本次实际生成的 **raw major ticks** 的 `max(abs(tick))` 为权威代表值；没有 major tick 时才回退到最终有效轴域的 `max(abs(min), abs(max))`。它不读取标签或单位。
- 小值阈值固定为代表值 `< 10^-2`；`0.01` 本身不触发。
- 大值阈值暂定为代表值 `>= 10^4`；`1000` 直接显示、`10000` 触发，实施倍率模型前必须最终确认。
- 阈值边界覆盖 `0.01`、`next_down(0.01)`、`10000` 和 `next_down(10000)`；autoscale padding 不再作为另一套倍率判断来源。
- 触发后默认指数为代表值的 `floor(log10(value))`，不限制为三的倍数；用户可手动选三倍数指数配合 SI 前缀。
- 若该指数对应的 `10^n` 下溢为零或上溢为无穷，自动模式将指数逐步向零收敛到最近的可表示 decade，并再次验证缩放值有限；最小正 subnormal 通常因此使用 `10^-323`，不得生成不可用的 `10^-324`。
- 全零轴不生成倍率；固定、Interval、Auto locator 和手动范围都使用同一代表值规则。

### 1.3 显示、零值和边界阈值严格分离

- **显示倍率阈值**只决定是否在轴标签中使用 `×10^n`。
- **零值规范化没有幅值阈值**：只规范化精确的 `-0.0`，或 tick 生成器依据数学构造明确知道该点就是零；任何真实非零值都不得因接近零而改成零。
- **边界比较容差**只处理 tick 与 min/max 的浮点舍入，依据局部值、step、跨度和 ULP；不得改变 tick 的真实数值，也不得以 `1.0` 为最低尺度。
- **恒定值范围策略**只负责构造非退化轴域，不与显示倍率、零值或边界容差共用 epsilon。
- 四类规则使用不同函数和明确名称，禁止一个通用 epsilon 同时控制范围、刻度和显示。

### 1.4 倍率状态不得存在非法组合

每条轴只能处于以下四种持久状态之一：

- `AutoFactor`：自动解析指数；非零指数在完整轴标签中显示。
- `None`：指数为零，不缩放刻度、不显示倍率。
- `ManualFactor(NonZeroI32)`：使用非零手动指数，并在完整轴标签中显示。
- `ManualIncorporated(NonZeroI32)`：使用非零手动指数，但用户已把倍率写入 `mΩ / μA / kPa` 等单位文字，不再生成倍率片段。

规则：

- Manual 状态使用非零载荷和私有验证构造器，类型层不能构造零指数。UI 输入 `0` 时明确转换为 `None`；项目反序列化出现 Manual `0` 时视为非法并拒绝，不能静默改写外部文件。
- 从 AutoFactor 或 ManualFactor 切换到“倍率已写入单位”时，将当前非零指数冻结为 ManualIncorporated，并与标签修改组成一个候选文档事务和一次 Undo。
- 当前解析指数为零时禁止进入 ManualIncorporated。
- ManualIncorporated 下修改指数不会自动改写单位；编辑当下显示瞬时确认提示，要求用户同步检查标签。项目不保存“是否刚改过”的伪历史状态，Publication Check 也不对所有 Incorporated 项目永久重复该提示。
- 重新选择自动倍率时转为 AutoFactor，并恢复生成的倍率片段。
- 手动指数只有在 `10^n` 以及当前轴域除以 `10^n` 后都有限时才有效。

### 1.5 倍率属于轴标签，使用显式位置而不猜单位

- 禁止独立角落 offset text，也禁止每个 tick 重复显示 `e-3` 或 `×10⁻³`。
- 轴标签输入支持显式 `{scale}` 语义占位符；它不是单位解析，只指定倍率片段在完整标签中的位置。
- 例如 `R_{xy} ({scale} Ω)` 在指数 `−3` 时渲染为 `R_{xy} (×10⁻³ Ω)`。
- AutoFactor/ManualFactor 中存在 `{scale}` 时在该位置生成倍率；显式占位符始终渲染，包括自动指数为零时显示 `×10⁰`，从而不会留下无法可靠处理的空格或空括号。
- 没有占位符时，仅当有效指数非零才把倍率作为同一 AxisLabel 的末尾语义片段追加；指数为零时不追加任何内容。
- ManualIncorporated 不允许保留 `{scale}`。用户将 `×10⁻³ Ω` 改成 `mΩ` 时，在同一次应用操作中删除占位符、修改标签并冻结指数 `−3`；刻度保持不变。
- 用户保存的其他标签节点保持原样，软件不解析 `Ω / mΩ / μΩ`，也不根据文字猜倍率。

### 1.6 刻度必须有限、唯一且可解释

- locator 始终产生真实坐标值；倍率只改变显示文字，不能改变数据域、normalized fraction、tooltip、参考线或测量箭头数据值。
- 极端范围下 major/minor tick 数量有硬上限，不能依赖可能饱和的 `i64` 商值，也不能生成 `0`、`∞`、NaN 或重复 raw ticks。
- Auto formatter根据缩放后的 step 提高精度，直到实际显示的相邻标签唯一或达到 `f64` 有效数字极限；自动模式不得静默产生同名标签。
- 手动 Decimal 精度导致重复标签时，产生本地化 Publication Warning，但允许用户检查后导出。
- 单个刻度标签超过 24 个可见字符时产生非阻断 Publication Warning；手动指数严重错配、缩放后非有限则为验证错误。
- 隐藏轴标签但 tick labels 可见且有效指数非零时，产生“刻度已缩放但倍率说明被隐藏”的 Publication Warning；倍率随 axis label 一起隐藏，不另行漂浮显示。
- 对数轴跨越大量 decade 时也受 tick 数量上限约束；自动倍率不得破坏对数位置或标签唯一性。

## 2. 实施阶段

### Phase 0：失败用例与基线

- 固定换列旧范围、恒定小值、真实小刻度被清零、极值溢出、重复标签和倍率缺失。
- 保存普通数量级、默认图、单轴/双轴及旧项目的 golden/snapshot。
- 真实 `0V_40um_Hall.csv` 只用于本地复现，不复制或提交用户数据。

完成门：每个缺陷由独立测试指向具体数值层，尚未修改产品行为。

### Phase 1：P0 自动范围与稳定映射

- 原子修复逻辑系列重绑和自动轴刷新。
- 建立安全 span、padding、map/unmap、midpoint、log 展开和误差端点合同。
- 覆盖单轴、双轴、误差棒、参考线、删除重导和手动范围。

完成门：真实文件完整显示形状；有限输入不因中间运算溢出失真；失败事务保留旧状态。

### Phase 2：极端 locator 与 formatter

- 删除固定 clean-zero 和最低尺度容差。
- 重写有界 tick 生成、唯一自动精度及固定/Interval 验证。
- 接通重复标签和超长标签 Publication Warning。

完成门：极小、极大、跨零、全负、subnormal、大偏置小跨度和 log 范围均产生有限、有序、可区分的结果。

### Phase 3：倍率模型、迁移与标签组合

- 最终确认大值阈值。
- 增加四态倍率模型、显式 `{scale}` 节点和 schema v10 → 下一版迁移。
- 旧 Auto/Decimal/Scientific 的迁移语义、provenance、dirty 和幂等行为必须冻结并测试。
- 将倍率组合进唯一 AxisLabel glyph、bounds 和 hit target。
- 接通“刻度已缩放但倍率说明被隐藏”的 Publication Warning。

完成门：四条轴独立保存和恢复；`×10⁻³ Ω ↔ mΩ` 不改变刻度；预览和全部导出一致。

### Phase 4：紧凑交互、完整回归与安装

- 在现有轴标签窗口加入倍率设置，不增加嵌套窗口。
- 覆盖中间输入、一次 Undo/Redo、窗口置顶、普通/最大化/全屏。
- 运行全量测试、格式、严格 Clippy、release 构建和独立功能 QA。
- 全部门通过后才替换唯一 Spotlight 应用；GitHub 操作另行确认。

## 3. 必须覆盖的验收矩阵

- 换列：`index → Rxy_Ohm`、大列 → 小列、小列 → 大列；自动和手动范围分别验证。
- 线性范围：全零、单点、非零恒定、最小 subnormal、`MIN_POSITIVE`、跨零、全负、`±f64::MAX`、大偏置小跨度。
- 对数范围：恒定正值、最小 subnormal、`MIN_POSITIVE`、最大有限值、零/负值原子失败及大量 decade。
- 误差棒：普通 X/Y error、端点接近零、端点上溢/下溢、换列和删除。
- locator：Auto、Fixed、Interval、主副刻度及精确阈值边界。
- 倍率：AutoFactor、None、ManualFactor、ManualIncorporated；非法状态不可构造。
- 标签：无 `{scale}`、不同位置的 `{scale}`、指数零、数学上下标、旋转 Y 标签、`mΩ` 和自定义文字。
- 交互：hover、tooltip、reference line、measurement arrow、axis range/interval 编辑及 Undo/Redo。
- 生命周期：导入、手动录入、清除、重导、保存重开、v0–当前 schema 迁移和未来版本拒绝。
- 显示与输出：隐藏 label/tick labels、Publication Warning、tight crop、AxisLabel hit target、PNG/SVG/PDF glyph 与 bounds 一致。
- 普通回归：默认图、普通数量级、单轴、双 X、双 Y及已有项目外观无非预期变化。

## 4. 技术门

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release -p instplot-studio
git diff --check
```

定向测试必须确认实际执行数量不为零。任一行为门或技术门失败时，不替换 Spotlight、
不上传 GitHub，也不将本计划标记为完成。
