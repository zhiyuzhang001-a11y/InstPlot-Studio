# InstPlot Studio 字体与科学排版规范

**文件名：** `INSTPLOT_STUDIO_TYPOGRAPHY_SPEC.md`  
**规范版本：** V1.0  
**状态：** Accepted  
**适用范围：** InstPlot Studio V1 的科学绘图标签、坐标轴、刻度、图例、注释及面板标记  
**默认出版字体：** TeX Gyre Heros

## 1. 目的与规范用语

本文件定义 InstPlot Studio 中科学标签的语义排版规则，以及字体、Unicode、字符覆盖和 PDF 输出的最低验收条件。实现方案可以变化，但最终渲染结果必须符合本规范。

本文中的关键词含义如下：

- **必须**：V1 的强制要求；不满足即不得视为通过验收。
- **应该**：默认行为；只有明确、合理的兼容需求才可偏离。
- **可以**：可选能力，不属于 V1 的强制保证。

## 2. V1 范围

V1 面向 **scientific plot labels**，不是通用 LaTeX 或完整数学排版系统。

V1 优先支持：

```text
μ₀H_DL (mT)
J_e (A m⁻²)
K_eff (kJ m⁻³)
θ_SH (°)
ΔR/R (%)
ρ_xy (μΩ cm)
```

V1 不以以下能力为目标：

- 通用积分、矩阵、分式及多层嵌套根式；
- 任意深度的上下标嵌套；
- 完整 TeX/LaTeX 语法解析；
- 完整 Unicode 数学符号覆盖；
- 专业数学字距、伸缩定界符和数学断行。

## 3. 默认字体与真实字重

### 3.1 默认字体家族

InstPlot Studio V1 的默认出版字体必须为 **TeX Gyre Heros**。Latin、Greek、数字、单位及 V1 Scientific Symbol Core 应优先由同一字体家族提供，避免使用独立 Symbol 字体造成视觉不一致。

### 3.2 必需字体文件

V1 必须打包或可靠解析以下四个真实 face：

- TeX Gyre Heros Regular
- TeX Gyre Heros Italic
- TeX Gyre Heros Bold
- TeX Gyre Heros Bold Italic

不得通过倾斜 Regular 模拟 Italic，也不得通过描边、膨胀或其他方式模拟 Bold。缺少所需真实 face 时，渲染器必须给出明确错误或警告，不得静默伪造字形。

Arial、Helvetica 或自定义字体可以在未来作为高级选项提供，但不属于 V1 默认依赖，也不得改变本规范定义的语义样式规则。

## 4. 语义排版规则

### 4.1 普通文本

标题、描述、图例文字和普通注释默认使用 Regular、upright：

```text
Magnetic field
Temperature
Experiment
Theory
Sample A
```

### 4.2 物理量与数学变量

Latin 物理量或数学变量必须使用 Italic：

```text
H  M  J  T  R  K  D  t  x  y
```

同一个字母的样式由语义决定，而不是由字符本身决定。例如 `T` 作为温度变量时为 Italic，作为 tesla 单位时为 Regular。

示例 `Magnetic field H (mT)`：

```text
Magnetic field    Regular
H                 Italic
 (                Regular
mT                Regular
)                 Regular
```

### 4.3 Greek 变量

Greek 字符表示物理量或数学变量时必须使用 TeX Gyre Heros Italic；不得为了 Greek 切换到 Symbol 字体。

```text
θ  μ  ρ  σ  α  β  φ  ω  Δ
```

Greek 字符用于单位或普通文本语义时使用 Regular。例如 ohm 单位 `Ω` 必须 upright。

### 4.4 单位

单位、单位前缀和单位组合必须使用 Regular、upright，包括：

```text
mT  T  K  A  V  s  Hz  Ω  μm  nm
```

单位中的数字和指数也保持 upright，例如 `A m⁻²`、`kJ m⁻³`。

### 4.5 数字

所有数字默认使用 Regular、upright，包括普通值、负数、小数、下标数字和指数数字：

```text
0  1  10  300  −1.5  μ₀  10⁻³
```

### 4.6 描述性下标

表示名称、机制、缩写或说明的下标必须使用 Regular、upright，并缩小字号、下移基线：

```text
H_DL  K_eff  M_sat  θ_SH  R_AHE
```

其中 `DL`、`eff`、`sat`、`SH`、`AHE` 均为描述性下标；例如 `H_DL` 的 `H` 为 Italic，`DL` 为 Regular subscript。

### 4.7 变量下标

下标本身表示数学变量、坐标或分量时必须使用 Italic，并缩小字号、下移基线：

```text
H_x  H_y  M_z  J_c
```

解析器不得只按下标长度猜测语义；Label AST 必须明确区分 `DescriptiveSubscript` 与 `VariableSubscript`。自动解析无法可靠判断时，应采用显式语义输入或给出可编辑的默认结果。

### 4.8 上标

上标继承其内容的语义样式，并缩小字号、上移基线：

- 数值指数及其负号使用 Regular：`m⁻²`、`10⁻³`；
- 变量指数使用 Italic：`xⁿ` 中 `n` 为 Italic；
- 单位指数始终 upright。

V1 必须使用布局信息实现上下标，不得要求用另一字体伪造完整标签。可以接受 Unicode 上下标输入，但内部应规范化为相同的语义布局模型。

#### 4.8.1 上下标尺寸与基线

V1 冻结以下脚本尺寸规则：

```text
Subscript scale   = 0.72 × base font size
Superscript scale = 0.72 × base font size
```

上下标的 baseline shift 必须根据 TeX Gyre Heros 的实际字体度量计算。具体位移值由实现确定，但对于相同字体版本、字号和输入必须是确定且可复现的；屏幕预览与 PDF 必须使用同一算法和同一结果。若启用可选 SVG，也必须消费相同结果，不得由各输出后端自行选择“近似”偏移。

若实测表明 `0.72` 在目标字体版本中产生明显碰撞或可读性问题，调整该常量必须作为规范版本变更，并重新运行全部 typography fixtures；不得只修改单一后端。

### 4.9 Bold

Bold 只用于：

- 面板标记，例如 `(a)`；
- 用户明确要求的强调文字。

普通坐标轴标签、刻度和图例默认不得加粗。

### 4.10 Bold Italic

Bold Italic 只用于语义上需要粗斜体的数学变量，例如用户明确启用 bold-vector notation 时的向量变量。V1 必须使用真实 Bold Italic face；该记法的自动推断可以延后至 V1.1，但显式指定时不得使用伪粗体或伪斜体。

### 4.11 字符身份与语义歧义

**Semantic meaning always overrides character identity.** 同一个 Unicode 字符可以因语义不同而采用不同样式；字体样式不得仅由字符值决定。

| 字符 | 变量语义 | 单位或文本语义 |
|---|---|---|
| `T` | 温度等变量：Italic | tesla：Regular |
| `K` | 变量：Italic | kelvin：Regular |
| `A` | 变量：Italic | ampere：Regular |
| `V` | 变量：Italic | volt：Regular |
| `s` | 变量：Italic | second：Regular |
| `Ω` | Greek Omega 变量：Italic | ohm：Regular |
| `m` | 变量：Italic | metre 或 SI 前缀 milli：Regular |
| `H` | 磁场等变量：Italic | henry：Regular |

解析器不得仅根据单字符内容决定样式。GUI 和 Label AST 必须允许用户明确指定 `Variable`、`GreekVariable` 或 `Unit`；自动推断存在歧义时，结果必须可见且可编辑。

### 4.12 字体度量与布局

所有文字布局必须使用目标 face 和目标字号的真实字体度量，至少包括：

- ascender；
- descender；
- advance width；
- glyph bounds；
- baseline；
- kerning 或 shaping 后的 glyph positioning。

不得通过固定字符宽度、字符数量、经验包围框或仅针对 Regular face 的常量估算文本尺寸。旋转 Y 轴标签、图例对齐、上下标、碰撞检测及导出边界必须使用 shaping 后的实际 advance 与 bounds。Italic、Bold 和 Bold Italic 必须分别测量，不得复用 Regular 的度量。

相同文本、字体文件、字号和布局参数在屏幕与 PDF 中必须产生一致的 advance、baseline 关系和可见边界；允许由抗锯齿导致像素级外观差异，但不允许结构性位移或换行差异。可选 SVG 不得重新 shaping 或布局。

## 5. 数量与单位格式

V1 默认并统一采用：

```text
Quantity (unit)
```

示例：

```text
Magnetic field (mT)
Temperature (K)
Current density (A m⁻²)
Resistance (Ω)
```

默认不采用 `H / mT` 或 `H [mT]`。括号及括号内单位均为 Regular、upright。

### 5.1 复合 SI 单位

复合 SI 单位默认使用**空格与幂**，不使用斜线形式：

```text
A m⁻²
m s⁻¹
J m⁻³
μΩ cm
```

不得作为默认规范化输出：

```text
A/m²
m/s
J/m³
```

单位因子之间必须使用 `UnitSeparator` 语义节点。其标准序列化字符为 `U+202F NARROW NO-BREAK SPACE`，布局宽度为 `0.2 em`，且不得在该位置换行。不得用普通可断行空格 U+0020 作为最终规范化输出。

数值与其单位之间也使用相同的不可断语义间距，例如 `1.2 mT`、`300 K`。单位名称内部不得插入间距，例如 `mT`、`kJ`、`μΩ`。

如果渲染后端以定位而非空格字形实现 `UnitSeparator`，PDF 的文本提取和复制结果仍必须在该位置恢复为 U+202F。屏幕与 PDF 必须使用同一宽度和不可断行规则；可选 SVG 若启用，也必须消费该布局结果。

V1 可以接受用户输入 `m/s`，但解析后必须规范化为 `m s⁻¹`；如果无法无歧义地解析，应保留输入并提示用户，而不是静默改写。

## 6. Unicode 语义规范

### 6.1 Micro sign 与 Greek mu

在现代 Unicode 输出中，micro 前缀和 Greek 变量均以 U+03BC 为规范字符，
但必须通过 Label AST 的语义节点区分其排版角色：

- 单位前缀 micro：`μ`，U+03BC GREEK SMALL LETTER MU，Regular；
- Greek 变量 mu：`μ`，U+03BC GREEK SMALL LETTER MU。

`µ`（U+00B5 MICRO SIGN）只作为 legacy/compatibility 输入接受。默认规范化
输出必须转换为 U+03BC；若项目显式启用“保留原始 code point”兼容选项，才可以
原样保留 U+00B5，并必须在字体诊断和 PDF 文本诊断中记录该决定。

示例：

```text
μm, μA, μΩ      # U+03BC，单位前缀，Regular
μ, μ₀, μH       # U+03BC，Greek 变量，Italic
```

复制、搜索和 PDF 文本提取必须保留规范化后的 U+03BC；启用 legacy 原样保留时，
必须保留输入的 U+00B5，不得由输出后端再次静默改写。

### 6.2 Minus sign

数值负号和数学减号必须使用 `−`（U+2212 MINUS SIGN），不得使用 `-`（U+002D HYPHEN-MINUS）代替。

```text
−1.0 V
−0.5
m⁻²
```

连字符、复合词及其他文本性 hyphen 仍可使用 U+002D 或适当的文本连字符。

### 6.3 Degree symbol

角度和温度必须使用 `°`（U+00B0 DEGREE SIGN），不得自行绘制圆圈，也不得使用字母 `o` 代替。

```text
θ (°)
Temperature (°C)
```

## 7. V1 Greek Core

默认字体的四个 face 必须覆盖以下 Greek Core，并能按语义使用 Regular 或 Italic：

```text
α β γ δ ε ζ η θ ι κ λ μ ν ξ ο π ρ σ τ υ φ χ ψ ω
Γ Δ Θ Λ Ξ Π Σ Φ Ψ Ω
ϵ ϑ ϕ ϖ ς
```

其中包含以下常用变体：

- `ϵ` U+03F5 GREEK LUNATE EPSILON SYMBOL
- `ϑ` U+03D1 GREEK THETA SYMBOL
- `ϕ` U+03D5 GREEK PHI SYMBOL
- `ϖ` U+03D6 GREEK PI SYMBOL
- `ς` U+03C2 GREEK SMALL LETTER FINAL SIGMA

`ϱ`（U+03F1 GREEK RHO SYMBOL）不属于 V1 Greek Core。只有在目标 OTF 的 cmap 实测通过后，才可以声明支持；不得用 private-use glyph 冒充 U+03F1。

## 8. V1 Scientific Symbol Core

默认字体的四个 face 必须覆盖以下 Publication Scientific Core：

```text
+ − ± ∓ × · ÷ =
< > ≤ ≥ ≠ ≈
∞ ∂ √ ∑
° % Å
← → ↑ ↓
Ω μ
```

`Ω` 同时属于 Greek 字母集合，在单位语义中必须用 Regular。micro 前缀使用
U+03BC，但在 `Unit` 语义中必须用 Regular；这与同一字符作为 `GreekVariable`
时使用 Italic 不冲突。

V1 不需要独立 Symbol 字体。只有在未来扩展高级数学能力且主字体确实缺字时，才可以引入经过明确声明和验证的 Math fallback。

### 8.1 科学计数法

科学计数法属于 V1 标准能力，规范形式为：

```text
1.2 × 10⁻³
3 × 10⁸
```

其中数字为 Regular，`×` 使用 U+00D7 MULTIPLICATION SIGN，`10` 为基线文本，指数作为 `Superscript(Number)` 按 `0.72 × base font size` 排版。不得使用字母 `x`、星号 `*` 或整段 Unicode 上标字符串代替语义结构。

## 9. 高级数学符号：非 V1 保证范围

以下字符不属于 V1 默认保证范围：

```text
∇ ∝ ∫ ∬ ∭ ∮ ∏
⊥ ∥ ↔ ⇒ ⇔
∈ ∉ ⊂ ⊆ ∪ ∩
≲ ≳
```

实现可以显示主字体中实际存在的字符，但必须遵守以下规则：

- 字符缺失时给出可见 warning，不得显示空白、`.notdef` 方框后仍声称成功；
- 不得偷偷用语义不同但外观相似的字符替换，例如用 `‖`（U+2016）代替 `∥`（U+2225）；
- 不得把偶然存在的高级符号宣传为完整数学排版支持；
- 后续若增加 fallback，必须独立规定其字形覆盖、度量、嵌入和文本提取行为。

`≲`（U+2272）和 `≳`（U+2273）可在确认实际使用需求且四个 face 的 cmap 与渲染测试全部通过后升级到 Core；V1 Draft 暂不保证，以免在实测前扩大强制覆盖范围。

## 10. Label AST 到字体样式的映射

Label AST 必须保存语义，而不是仅保存最终字符串。最低映射如下：

| AST 节点 | 默认 face | 尺寸/基线 | 说明 |
|---|---|---|---|
| `Text` | Regular | 正常 | 普通文本 |
| `Variable` | Italic | 正常 | Latin 数学或物理变量 |
| `GreekVariable` | Italic | 正常 | Greek 数学或物理变量 |
| `Unit` | Regular | 正常 | 单位及前缀，upright |
| `UnitSeparator` | Regular | `0.2 em`、不可断行 | 序列化为 U+202F |
| `Number` | Regular | 正常 | 数字及数值负号 |
| `DescriptiveSubscript` | Regular | `0.72×`、下移 | 描述性名称或缩写 |
| `VariableSubscript` | Italic | `0.72×`、下移 | 数学变量或分量 |
| `Superscript` | 继承子节点语义 | `0.72×`、上移 | 数值 upright，变量 italic |
| `Operator` | Regular | 正常 | 运算符和关系符 |
| `Emphasis` | Bold | 正常 | 显式强调 |
| `BoldVariable` | Bold Italic | 正常 | 显式粗斜体变量 |

示例 `H_DL (mT)` 的期望 semantic runs：

```text
H       Variable                → Italic
DL      DescriptiveSubscript    → Regular, smaller, baseline down
space   Text                    → Regular
(       Text                    → Regular
mT      Unit                    → Regular
)       Text                    → Regular
```

示例 `J_e (A m⁻²)`：

```text
J       Variable                → Italic
e       VariableSubscript       → Italic, smaller, baseline down
A       Unit                    → Regular
space   UnitSeparator           → 0.2 em, no break, U+202F
m       Unit                    → Regular
−2      Superscript(Number)     → Regular, 0.72×, baseline up
```

示例 `1.2 × 10⁻³`：

```text
1.2     Number                   → Regular
space   Text                     → Regular
×       Operator                 → Regular, U+00D7
space   Text                     → Regular
10      Number                   → Regular
−3      Superscript(Number)      → Regular, 0.72×, baseline up
```

AST 到屏幕和 PDF 的样式映射必须来自同一规则源，避免不同输出后端产生语义或视觉差异。可选 SVG 不得建立独立规则源。

## 11. V1 Coverage 与验证规则

### 11.1 冻结字体前的必需检查

正式冻结 TeX Gyre Heros 的具体发布版本前，必须直接读取实际随软件交付的四个 OTF 文件的 cmap，而不能只依赖网页、旧版技术文档、字体名称或系统预览。

每个 face 均必须分别检查：

1. Greek Core 中每个 Unicode code point；
2. Scientific Symbol Core 中每个 Unicode code point；
3. U+03BC 是否存在；若实现支持 legacy 原样保留模式，还必须检查 U+00B5；
4. U+2212 是否存在，不得仅有 U+002D；
5. 四个 face 是否为真实文件和真实样式，而非合成结果。
6. `UnitSeparator` 在三个输出后端中是否均为 `0.2 em`、不可断行，并在文本提取时恢复为 U+202F。

只有四个 face 对两个 Core 均为 100% PASS，才可将该字体版本标记为 V1 默认字体。任一必需字符缺失都必须阻止发布或触发明确的字体替换决策；不得用未声明的系统字体静默补字。

### 11.2 必需测试层级

V1 至少必须包含：

- **静态覆盖测试**：逐 face 检查 cmap；
- **AST 单元测试**：检查语义节点及对应 face、字号和 baseline shift；
- **字体度量测试**：检查各 face 的 ascender、descender、advance、bounds、baseline 与 shaping positioning；
- **渲染 fixture**：覆盖 Latin、Greek、单位、上下标、粗体和核心符号；
- **跨后端一致性测试**：屏幕预览和 PDF 使用相同 semantic runs；可选 SVG 若启用则复用它们；
- **PDF 文本测试**：复制或提取文本后 Unicode code point 与输入语义一致；
- **缺字测试**：非 Core 缺字必须产生明确 warning；
- **回归测试**：升级字体文件或 shaping/rendering 组件时重新运行全部检查。

建议至少固定以下 fixture：

```text
Magnetic field H (mT)
μ₀H_DL (mT)
J_e (A m⁻²)
K_eff (kJ m⁻³)
θ_SH (°)
ΔR/R (%)
ρ_xy (μΩ cm)
T ≤ 300 K
H ≈ 1.2 mT
x ± σ
1.2 × 10⁻³
1.2 mT
m s⁻¹
μΩ cm
−1.0 V
(a)
```

### 11.3 版本与可重复性

软件必须记录并固定实际使用的 TeX Gyre Heros 版本及文件校验信息。字体版本更新属于可见输出变更，必须重新执行 coverage、fixture 和 PDF 验证，不能只凭 family name 认为结果等价。

## 12. PDF 与文本保真

### 12.1 真实文字

PDF 中的标签必须尽可能保持为真实文字对象，不得默认把整段文字轮廓化或栅格化。输出必须满足：

- 文本可选择；
- 文本可搜索；
- 文本可复制；
- Unicode 语义可正确提取；
- 放大后保持矢量清晰度。

如果某个受限导出路径必须转为轮廓，必须由用户明确选择，并提示将失去搜索、复制和辅助技术可读性；该模式不得作为默认出版 PDF。

### 12.2 字体嵌入

PDF 导出必须按字体许可嵌入或 subset 嵌入实际使用的真实 face。导出后必须检查：

- Regular、Italic、Bold、Bold Italic 的实际使用 face 均已正确嵌入或 subset；
- 没有未声明的字体替换；
- 没有 synthetic bold 或 synthetic italic；
- ToUnicode 映射可恢复原始字符；
- U+03BC、U+2212、U+00B0 等关键字符可正确复制与搜索；启用 legacy 原样保留模式时还必须验证 U+00B5。

### 12.3 上下标与结构

上下标应由相同文本 run 的字号和基线位置表达，且仍保持可提取文本。视觉布局不得通过把整段标签转成图片实现。

### 12.4 可选 SVG

SVG 不属于 V1 强制输出，也不参与 Gate A 是否通过的判定。保留的 SVG
实验必须明确标注其目标查看器、字体策略和已知编辑器限制；不得把浏览器或
resvg 中可显示等同于 Illustrator 等编辑器中的可编辑文字兼容性。重新将 SVG
列为必需输出时，必须通过新的 ADR 和独立验收合同。

## 13. 错误处理与 fallback

- V1 Core 字符缺失是发布阻断错误，不是普通 warning。
- 高级数学字符缺失时必须给出明确 warning，并指出缺失字符及 Unicode code point。
- 不得在没有记录的情况下按字符切换到系统 fallback。
- 未来新增 fallback 时，应优先保持同一标签内 Latin、Greek 与运算符的视觉协调，并单独验证 PDF 嵌入和 Unicode 提取。
- 不得用 private-use glyph、相似字符或路径图形掩盖必需字符缺失。

## 14. 明确排除项

中文及其他 CJK 文本不属于 InstPlot Studio V1 的产品范围。V1 不要求 CJK
字体、fallback、换行、字体度量或出版输出保证；遇到 CJK 字符必须给出明确的
unsupported-script 诊断，不得静默调用系统字体、替换字符、轮廓化或栅格化后声称
成功。未来是否增加 CJK 支持必须通过新的产品范围和字体规范决策，不作为本规范的
待完成验收项。

## 15. V1 验收结论模板

字体版本冻结时应生成并保存类似以下记录：

```text
Font family: TeX Gyre Heros
Font version: <exact version>
Font file hashes: <hashes>

Greek Core
Regular       PASS / FAIL [missing code points]
Italic        PASS / FAIL [missing code points]
Bold          PASS / FAIL [missing code points]
Bold Italic   PASS / FAIL [missing code points]

Scientific Symbol Core
Regular       PASS / FAIL [missing code points]
Italic        PASS / FAIL [missing code points]
Bold          PASS / FAIL [missing code points]
Bold Italic   PASS / FAIL [missing code points]

Semantic fixtures             PASS / FAIL
Font metrics/layout           PASS / FAIL
UnitSeparator/PDF U+202F      PASS / FAIL
Screen/PDF consistency        PASS / FAIL
PDF fonts embedded            PASS / FAIL
PDF searchable/copyable       PASS / FAIL
Unicode extraction            PASS / FAIL
Missing-glyph warnings        PASS / FAIL

V1 default font freeze        APPROVED / REJECTED
```

只有所有 V1 强制项均通过，才能报告字体与科学排版系统 ready。
