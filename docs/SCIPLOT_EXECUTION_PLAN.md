# InstPlot Studio 执行计划

> 文档性质：技术验证与正式开发执行计划
> 状态：Part A DONE；Part B ACTIVE（B0–B2、B3.1 DONE；B3.2 READY）
> 制定日期：2026-09-20
> 适用范围：InstPlot Studio 启动验证、共享核心抽取、V1 开发与发布
> 前置文档：`SCIPLOT_PRODUCT_BOUNDARY.md`、`SCIPLOT_TECHNICAL_RESEARCH.md`、`INSTPLOT_STUDIO_TYPOGRAPHY_SPEC.md`

## 1. 计划目标

本计划把 InstPlot Studio 从技术调查推进到可执行工程，同时保护已经可用的 InstPlot Lite。

计划分为两个连续部分：

1. **Part A：启动前技术验证**  
   通过独立原型消除物理布局、字体、PDF、raster 和跨平台一致性的关键风险；SVG 仅保留为可选研究。

2. **Part B：正式产品开发**  
   在技术路线通过验证后，抽取 Lite 共享核心并建立独立的 InstPlot Studio。

Part A 未通过最终启动门槛前，不开始完整 Studio UI，不大规模重构 Lite，也不承诺正式项目文件格式。

---

## 2. 执行原则

### 2.1 两个独立产品

- InstPlot Lite 与 InstPlot Studio 分别拥有独立 binary、安装包、版本号和发布节奏；
- Studio 的大型出版依赖不得增加 Lite 的体积和启动成本；
- 两者共享数据、处理、拟合和交换格式，不共享产品 UI。

### 2.2 先证据，后选型

- 不根据 crate 功能列表决定技术栈；
- 不根据单张截图判断输出质量；
- 所有关键选择必须由固定测试图、结构测试和跨平台结果支持；
- 每项关键选择形成 Architecture Decision Record，简称 ADR。

### 2.3 先垂直切片，后扩展功能

第一目标不是覆盖所有图形类型，而是让一张最小出版图完整通过：

```text
Figure Document
  → semantic resolution
  → physical layout
  → Display List
  → screen preview
  → PDF / PNG（可选 SVG）
  → structural and visual verification
```

### 2.4 Lite 始终保持可发布

- 共享核心抽取采用小步迁移；
- 每一步完成后 Lite 必须继续构建和通过测试；
- 不在同一个变更中同时重构 Lite、改变数据格式并开发 Studio 功能；
- 不复制 Lite 模块后分别维护。

### 2.5 许可证是技术门槛

- 生产依赖必须记录 SPDX license 和 transitive licenses；
- IronLAB 仅作 clean-room 架构参考，不直接依赖或复制代码；
- bundled font、palette data 和测试资源同样必须记录来源与授权。

### 2.6 轻量化是正式架构约束

Studio 的轻量化不能留到发布前再优化。Part A 开始时即采用统一测量口径：

- 测量对象：stripped release executable 加随应用强制分发的 runtime assets；
- 不计入：安装器封装、示例数据、可选语言包和用户项目；
- 单位：MiB，同时在面向用户材料中可换算为十进制 MB；
- 每次引入 GUI、字体、PDF、SVG、raster 或 shaping 依赖时记录增量。

体积预算：

```text
Architecture target    ≤ 10 MiB
Soft ceiling           ≤ 12 MiB
Architecture review    > 15 MiB
```

规则：

- `10–12 MiB` 必须说明超出 target 的原因及后续压缩空间；
- `12–15 MiB` 在进入生产依赖前必须给出明确减重方案；
- `>15 MiB` 触发架构复审，在复审完成前不得通过 Gate A；
- 不允许通过下载运行时组件、依赖系统外部命令或首次启动联网安装来伪造小体积。

---

## 3. 总体阶段

```text
Part A：启动前技术验证

A0  冻结 Lite 基线
 ↓
A1  固定测试图与验收合同
 ↓
A2  最小 Figure IR / Display List
 ↓
A3  文字与字体技术验证
 ↓
A4  PDF / Raster 后端验证（可选 SVG 实验）
 ↓
A5  Plotine 对照与复用决策
 ↓
A6  UI Shell 极小验证
 ↓
A7  单 axes 自动布局与出版图元
 ↓
A8  跨平台验证、回归系统与 ADR
 ↓
GATE A：是否允许正式启动 Studio

Part B：正式产品开发

B0  抽取 Lite 共享核心
 ↓
B1  建立独立 Studio 应用
 ↓
B2  正式 Figure Document 与项目格式
 ↓
B3  V1 出版绘图能力
 ↓
B4  Palette、语义系统与 Publication Check
 ↓
B5  Lite → Studio 交接
 ↓
B6  三平台打包、验收与预览发布
```

---

# Part A：启动前技术验证

## 当前检查点（2026-09-22，Gate A 关闭）

- **A0–A8：DONE。** 本机 21 项验证、字体/视觉矩阵、PDF Viewer、依赖审计和
  Windows 21 项自动化验证均通过；Windows 10 的四档 scaling、Unicode 输入、
  原生文件窗口和 Edge PDF 检查也通过。
- **延期但不冒充通过：** Linux 与物理 Retina/HiDPI 由产品负责人明确移到相应
  发布声明前；SVG editor compatibility 由 ADR-021 保留为非阻断研究。
- **已知迁移项：** A6 临时 shell 的纵轴占位文字不是出版级预览，必须在 B3
  接入 A3/A7 resolved text 后关闭。
- **Part B：ACTIVE。** B0 共享核心、B1 独立 Studio 应用与 B2 正式 Figure
  Document/项目格式已经关闭，下一步是 B3 single-axes 正式垂直切片。独立短计划见
  `docs/INSTPLOT_STUDIO_PART_B_SHORT_PLAN.md`。

## 4. A0：冻结 InstPlot Lite 基线

### 4.1 目标

在开始任何共享核心抽取前，建立可比较、可恢复的 Lite 行为基线。

### 4.2 工作项

#### A0.1 记录代码与工具链基线

- 记录 Rust toolchain；
- 记录直接依赖和 feature；
- 记录当前支持平台；
- 记录安装包和 release binary 大小；
- 按统一口径记录 executable + mandatory runtime assets；
- 记录空窗口内存基线；
- 记录测试数量和执行结果。

#### A0.2 建立 Lite 关键回归清单

至少覆盖：

- TXT/CSV/DAT/TSV 导入；
- XLSX/XLS 导入；
- 编码、分隔符和 header 识别；
- source/fit 关联；
- 单点和框选删除；
- undo/redo；
- processing；
- fitting；
- 多数据集导出；
- PNG 快速导出；
- 大曲线保峰降采样。

#### A0.3 保存代表性基线产物

- smoke 数据；
- source + fit round-trip 数据；
- 多 sheet workbook；
- Lite PNG；
- `--check` 输出；
- 三平台 CI 结果。

### 4.3 产物

- Lite baseline report；
- 可重复执行的基线命令；
- 必要的测试 fixture；
- 已知限制列表。

### 4.4 完成条件

- 当前 Lite tests 全部通过；
- 没有未解释的 warning；
- 关键格式至少各有一个固定 fixture；
- 后续共享核心抽取可以与本基线比较。

### 4.5 停止条件

如果 Lite 当前测试不稳定或数据格式行为不明确，先修复或记录，不进入 A1 之后的集成工作。

---

## 5. A1：固定测试图与验收合同

### 5.1 目标

建立所有候选技术共用的输入和输出合同，避免“每个库画一张不同的图”。

### 5.2 工作项

#### A1.1 固定数据集

测试数据应包括：

- 两组 experiment scatter；
- 一条 fit；
- 一条 theory；
- Y error bar；
- reference baseline；
- 正值、负值、零附近数据；
- 多数量级数据；
- 有意设置的局部密集区域；
- 足以测试 clipping 的边界点。

数据应小而可人工核对，不使用敏感实验数据。

#### A1.2 固定标签

至少包含：

```text
μ₀H_DL (mT)
Current density J_e (A m⁻²)
T ≤ 300 K
Experiment / Fit / Theory
```

另设 `温度 T (K)` 作为 unsupported-script 负向 fixture；它不得进入成功的
出版输出，只用于验证 V1 明确拒绝 CJK 且不调用系统 fallback。

#### A1.3 固定视觉参数

以下数值是 **validation reference values**，用于让候选技术接受完全相同的测试；它们不是在此阶段冻结的最终产品默认值。最终默认值仍需结合 Publication Visual Benchmark Corpus 和原型结果确认。

- figure：89 mm × 65 mm；
- axis label：9 pt；
- tick/legend/annotation：8 pt；
- spine：0.6 pt；
- primary line：1.0 pt；
- fit/theory：0.9 pt；
- error bar：0.7 pt；
- marker：4 pt；
- white background；
- top/right spine on；
- grid off；
- inward ticks。

#### A1.4 固定 Part A 颜色合同

颜色系统不能推迟到 Part B 才首次进入真实渲染。Part A 至少冻结以下带来源的测试 palette：

- Distinct：Paul Tol Bright；
- High Contrast：Paul Tol High Contrast；
- Diverging：从正式来源文件锁定的发散 palette；
- Neutral gray：固定颜色、来源和软件语义；
- Experiment/Fit：同对象共享颜色，以 marker/line style 区分；
- Theory/Reference：按既有规范应用 line style 和 neutral 规则。

每个测试 palette 保存：

- source；
- source version；
- provenance class；
- exact colors or source-file checksum；
- sampling method；
- expected grayscale order；
- expected role mapping。

Part A 的目标不是实现完整 Palette Registry UI，而是确保颜色、灰度、CVD、legend 和 semantic style 从第一张测试图开始共同验证。

#### A1.5 固定输出合同

必须输出：

- preview；
- PDF；
- 300 dpi PNG；
- 600 dpi PNG；
- 1200 dpi PNG。

SVG 按 ADR-021 为非阻断可选实验，不属于 A1 必须输出。

PDF 目标尺寸：

```text
252.28346 pt × 184.25197 pt
```

Raster 目标尺寸，采用 round-to-nearest：

```text
300 dpi  → 1051 × 768 px
600 dpi  → 2102 × 1535 px
1200 dpi → 4205 × 3071 px
```

#### A1.6 建立 Publication Visual Benchmark Corpus

建立 10–20 张高质量、以二维曲线、散点、误差棒和拟合为主的出版图参考集，覆盖 Nature、Science、APS/PRL、ACS/JACS、Advanced Materials/Wiley 等代表性来源。

Corpus 不用于 pixel-copy，也不把某一本期刊变成主题。对每张参考图记录可测量的视觉属性：

- figure/axes 比例；
- physical or published size；
- axis label、tick、legend 的相对字号；
- marker diameter / line width 比例；
- tick 密度；
- 四侧 margin；
- legend key length、padding 和间距；
- grid/spine 策略；
- 色彩亮度、饱和度和角色层级；
- experiment/fit/theory 编码；
- 数据密度和留白。

来源要求：

- 优先使用出版社官方 figure guide、开放获取论文或明确允许研究引用的材料；
- 保存链接、期刊、年份、figure identifier 和访问日期；
- 未确认再分发权限时，不把原始出版图片提交进仓库；
- 仓库可以保存人工测量值、引用和不含受保护图像的派生统计。

Corpus 的作用是校准默认视觉系统是否达到出版审美基线，不能替代结构正确性测试。

#### A1.7 定义容差

明确：

- page-size tolerance；
- glyph-position tolerance；
- path-coordinate tolerance；
- raster perceptual-diff threshold；
- anti-aliasing 差异是否允许；
- 跨平台何种差异必须失败。

### 5.3 产物

- 固定 fixture 数据；
- fixture manifest；
- expected semantic description；
- expected physical metrics；
- versioned Part A palette fixture；
- Publication Visual Benchmark Corpus manifest；
- 验收脚本接口说明。

### 5.4 完成条件

- 不看渲染结果也能从 manifest 判断正确输出；
- 所有候选实现使用完全相同的输入；
- 所有物理值具有唯一换算和 rounding 规则。
- 固定测试图在正常颜色、灰度和至少一种 CVD 条件下具有预期参考输出；
- 默认比例可以与 benchmark corpus 的派生测量进行比较。

---

## 6. A2：最小 Figure IR 与 Display List

### 6.1 目标

验证与 UI、PDF 和具体绘图库无关的核心数据流。

### 6.2 原型位置

建议建立独立目录：

```text
prototypes/studio-render-spike/
```

原型暂不成为正式产品 crate，不修改 Lite 的应用入口。

### 6.3 最小 Figure IR

只实现：

- Figure；
- Axes；
- Axis；
- Line；
- Scatter；
- ErrorBar；
- ReferenceLine；
- Text；
- Legend；
- stable node IDs。

### 6.4 单位系统

建立明确边界：

- `Mm`：用户和文档物理尺寸；
- `Pt`：layout 和 Display List；
- `Px`：screen/raster backend；
- 不允许在核心布局中使用无含义的裸 `f32` 表示不同单位；
- 坐标原点和 Y 方向必须写入 ADR。

### 6.5 Display List

只实现：

- Path；
- Stroke；
- Fill；
- GlyphRun placeholder；
- Image；
- ClipPush/ClipPop；
- source node ID；
- ordered items。

### 6.6 结构化 snapshot

Display List 必须可以输出 deterministic debug representation，用于比较：

- figure size；
- axes rect；
- paths；
- strokes；
- clips；
- item order；
- node mapping。

### 6.7 ADR

- ADR-001：Figure、layout 和 raster 的单位系统；
- ADR-002：Display List 坐标系和图元合同；
- ADR-003：stable node identity。

### 6.8 完成条件

- 固定 Figure IR 可重复产生完全相同的 Display List snapshot；
- Display List 不依赖 egui、Krilla 或文件格式；
- 一个最小 path backend 可正确画出固定 axes 和曲线。

### 6.9 停止条件

如果必须在 backend 中重新计算 axes、tick 或文字位置，说明 Display List 边界错误，不进入 A3。

---

## 7. A3：文字、字体与数学排版验证

### 7.1 目标

选定唯一默认 shaping 路线，并证明它可以同时服务 preview 和 PDF；可选 SVG 若启用必须复用同一结果。

### 7.2 候选

- Candidate T1：Parley + fontique + HarfRust + skrifa；
- Candidate T2：cosmic-text；
- V1 math path：InstPlot semantic Label AST。

完整 LaTeX 或扩展 LaTeX-compatible math syntax 明确属于 V1 非目标。`latex-rust` 等方案只记录为未来候选，不进入 Gate A 的生产选型和依赖集合。

### 7.3 字体候选审计

每个字体集合记录：

- family and face names；
- Regular/Italic/Bold/Bold Italic；
- Latin、Greek 和 V1 Scientific Symbol Core coverage；
- OpenType tables；
- embedding/subsetting 权利；
- OFL 或其他许可证；
- 文件大小；
- checksum；
- PDF name；
- fallback 顺序。

### 7.4 必测场景

- plain Latin；
- italic variable；
- upright number and unit；
- Greek；
- descriptive subscript；
- mathematical subscript；
- superscript negative exponent；
- missing glyph；
- unsupported script；
- rotated Y-axis label；
- legend text；
- PDF text extraction。

### 7.5 比较指标

- shaped glyph ID 可获得；
- glyph positions 可传递给所有 backend；
- source Unicode 可保留；
- baseline 和 bounds 可测量；
- italic/upright range 正确；
- font selection deterministic；
- bundled TeX Gyre Heros 四个真实 face 跨平台一致；
- unsupported script 产生明确诊断且不触发系统字体；
- binary-size impact；
- build-time impact；
- API stability；
- maintenance and license。

### 7.6 Label AST 最小语法

```text
Text
Variable
Upright
GreekVariable
Number
DescriptiveSubscript
VariableSubscript
Superscript
Unit
UnitSeparator
Operator
Emphasis
BoldVariable
Group
```

AST 到 glyph runs 的转换必须独立于 backend。

### 7.7 ADR

- ADR-004：默认 text shaping engine；
- ADR-005：历史 Source Sans 3/CJK 决策，已由 ADR-020 取代；
- ADR-006：历史 fallback 决策，V1 部分已由 ADR-020 取代；
- ADR-007：V1 Label AST 与高级数学范围。
- ADR-020：InstPlot Studio V1 typography profile。

### 7.8 完成条件

- 选定一个默认 shaper；
- preview/PDF 不使用不同 shaper；可选 SVG 不得另行 shaping；
- 固定标签可以产生 deterministic glyph runs；
- TeX Gyre Heros 四个真实 face 在三平台产生一致 metrics；
- Greek Core 和 Scientific Symbol Core 的每个 code point 均通过逐 face 检查；
- CJK 等非 V1 script 产生明确 unsupported-script error，且不调用系统 fallback；
- missing glyph 有明确 warning；
- PDF 中混合文字可搜索和复制。

### 7.9 No-go 条件

以下任一情况发生时不得用 workaround 掩盖后继续：

- PDF 只能把所有文字转 outline；
- preview 和 export 必须分别 shape；
- 字体授权不允许嵌入；
- 出现未声明或不可复现的字体 fallback；
- 数学布局无法返回稳定 bounds。

### 7.10 字体体积与 unsupported-script 策略

V1 固定字体范围：

```text
TeX Gyre Heros
  bundled and deterministic
  Regular / Italic / Bold / Bold Italic
  Latin / Greek / V1 Scientific Symbol Core

CJK and other unsupported scripts
  explicit unsupported-script diagnostic
  no system fallback
```

要求：

- 四个 bundled face 必须满足跨平台确定性和 `<10 MiB` 总体架构目标；
- 必须固定版本、文件 checksum、GUST Font License 和 PDF/PostScript name；
- U+03BC 是 micro 前缀与 Greek mu 的默认 Unicode 输出，语义节点决定 upright/italic；
- U+00B5 只作为 legacy 输入接受，默认规范化为 U+03BC；
- 非 V1 script 不得静默 fallback、转 outline 或转位图。

---

## 8. A4：PDF 与 Raster 后端验证（含可选 SVG 实验）

### 8.1 目标

证明同一 Display List 可以产生结构正确、物理尺寸一致的多种输出。

### 8.2 PDF Spike

首选候选：Krilla。

实现：

- page size；
- vector path；
- stroke/fill；
- dash/cap/join；
- clip；
- positioned glyph runs；
- font embedding/subsetting；
- metadata；
- raster image；
- transparent objects 的基本行为。

验证：

- MediaBox/CropBox；
- 字体对象；
- text extraction；
- page 是否被整体 rasterize；
- path 数量和 clip；
- 常见阅读器显示；
- Illustrator/Inkscape 等后续编辑兼容性抽样。

### 8.3 SVG Spike

本节是 ADR-021 下保留的非阻断研究，不属于 Gate A 或 V1 完成条件。

实现：

- physical width/height；
- pt-based viewBox；
- path；
- stroke/fill；
- dash/cap/join；
- clipPath；
- text/glyph positioning；
- font family/fallback；
- metadata。

验证：

- usvg 可解析；
- 无外部临时文件；
- physical size 正确；
- viewBox 与 Display List 一致；
- 默认保留真实 text；
- round-trip raster 无裁切。

### 8.4 Raster Spike

候选：

- Display List → tiny-skia；
- Display List → SVG → resvg；
- 两条路线进行结果和复杂度比较。

验证：

- 300/600/1200 dpi；
- white/transparent background；
- alpha；
- anti-aliasing；
- PNG metadata；
- 后续 TIFF 共用 RGBA buffer 的可行性。

### 8.5 ADR

- ADR-008：PDF backend；
- ADR-009：SVG backend；
- ADR-010：raster backend 和 DPI rounding；
- ADR-011：真实文字与 optional outline policy。

### 8.6 完成条件

- 必需后端只读取同一 Display List；
- PDF 保持 vector geometry；
- PDF 文字可搜索且字体嵌入；
- PNG 像素尺寸正确；
- 后端之间没有独立 tick、legend 或 layout 代码。

可选 SVG 原型若保留，仍应读取同一 Display List；其兼容性失败不阻断 Gate A。

---

## 9. A5：Plotine 对照与复用决策

### 9.1 目标

用数据决定是否复用 Plotine 或其低层 crates，避免两种错误：

- 忽略已有工作而从零重复实现；
- 因为库宣称支持 PDF 就过早锁定。

### 9.2 对照任务

使用 Plotine 生成 A1 的同一测试图，测量：

- 89 mm × 65 mm 精度；
- axis/tick 可控性；
- text shaping；
- italic/upright mixing；
- mathtext；
- PDF font embedding；
- text extraction；
- PDF/PNG parity；可选 SVG 仅作研究对照；
- marker/error bar；
- clipping；
- dependency tree；
- binary size；
- API customization；
- semantic-style integration difficulty。

### 9.3 可选结论

ADR 只能选择以下之一：

1. 直接依赖 Plotine 的特定低层 crate；
2. 通过 adapter 使用部分功能；
3. 仅借鉴算法和测试，不成为依赖；
4. 暂时放弃。

不得使用“以后再看”作为正式结论。

### 9.4 ADR

- ADR-012：Plotine 复用结论。

### 9.5 完成条件

- 对照数据、输出文件和依赖信息均保存；
- 结论说明收益、限制、退出成本和版本锁定策略；
- 不因 sunk cost 强行采用候选库。

---

## 10. A6：UI Shell 极小验证

### 10.1 目标

在 Gate A 前验证应用外壳不会破坏轻量化、跨平台、HiDPI 和交互目标。此阶段不建设正式 Studio UI，只制作可以替换或丢弃的极小 shell。

### 10.2 必须包含

- 独立 Studio prototype window；
- central canvas；
- left or right side inspector placeholder；
- menu/toolbar placeholder；
- native file dialog；
- keyboard shortcut 和文字输入；
- canvas zoom；
- HiDPI transform（Gate A 要求 1×/2× arithmetic 与 Windows 实机 scaling；
  Retina 实机无设备时延后到发布前）；
- Windows display scaling；
- light/dark OS 环境下的可读性，但出版画布保持白底；
- headless export 与 window renderer 解耦。

### 10.3 测量

- stripped release executable；
- mandatory runtime assets；
- cold startup；
- idle memory；
- first-canvas-render latency；
- HiDPI framebuffer size；
- Windows/macOS/Linux font and scaling behavior；
- 加入 PDF/text/raster 依赖前后的 size delta。

### 10.4 架构约束

- UI shell 只编辑 Figure Document 或 view overlay；
- canvas 只消费 Display List；
- file dialog 和 window state 不进入核心 crates；
- 不为 shell 引入 webview、JavaScript runtime 或首次启动下载；
- GUI framework 不能迫使 preview 与 headless export 共用不可分离的 GPU/window context。

### 10.5 ADR

- ADR-013：Studio UI shell 与 GUI framework；
- ADR-014：HiDPI、screen transform 和 window-independent export；
- ADR-015：binary-size、startup 和 idle-memory baseline。

### 10.6 完成条件

- 三平台可以启动并显示相同 Display List；
- 1×/2× transform 与 Windows scaling 下 figure physical preview scale 行为明确；
- native file dialog 和 keyboard input 工作；
- headless export 不要求创建可见窗口；
- executable + mandatory assets 满足 size budget，或触发规定的架构复审；
- 测量数据进入 Gate A evidence。

### 10.7 No-go 条件

- GUI shell 使产物超过 15 MiB 且没有可行减重路线；
- headless export 必须依赖可见 window；
- HiDPI 下 Display List 到 screen transform 无法稳定定义；
- framework 迫使出版 layout 使用 window pixels；
- idle memory 或 cold startup 相比 Lite 目标出现无法解释的数量级增长。

---

## 11. A7：单 Axes 布局与出版图元

### 11.1 目标

证明 V1 的核心出版布局能够确定性运行，并覆盖规范中的基础图元。

### 11.2 Scale、Locator、Formatter

实现并分离：

- LinearScale；
- Log10Scale；
- AutoLocator；
- MinorLocator；
- FixedLocator；
- ScalarFormatter；
- SharedExponentFormatter；
- negative-zero suppression；
- precision from step；
- collision detection。

### 11.3 有限迭代布局

实现：

1. initial axes proposal；
2. tick generation；
3. label shaping；
4. decoration measurement；
5. margin update；
6. axes update；
7. maximum 3–4 iterations；
8. stable or conservative fallback；
9. non-convergence warning。

### 11.4 图元

- line；
- scatter；
- line + marker；
- open/filled circle；
- square；
- triangle-up/down；
- diamond；
- plus/cross；
- X/Y error bar；
- reference line；
- annotation；
- legend key；
- solid/dashed/dotted/dash-dot。

### 11.5 Legend 自动放置

实现 deterministic candidate scoring：

- axes 内候选位置；
- coarse data occupancy；
- error bar/annotation overlap cost；
- extrema penalty；
- edge-distance penalty；
- threshold；
- 无安全位置时 axes 外放置。

### 11.6 Hit Map

Layout Compiler 同时产生：

- node ID；
- bounds/path proximity；
- z-order；
- selectable role；
- tooltip/data mapping。

Preview 使用 hit map，不重新解析几何。

### 11.7 ADR

- ADR-016：scale/locator/formatter contract；
- ADR-017：single-axes layout algorithm；
- ADR-018：legend auto-placement；
- ADR-019：preview overlay 和 document edit 边界。

### 11.8 完成条件

- 测试矩阵中无文字和图元裁切；
- tick label 不重叠；
- legend 不覆盖关键区域，或自动移到外部；
- open/filled marker 和 dash 在最终尺寸可辨识；
- layout deterministic；
- warning 可解释且可定位到 node。

---

## 12. A8：验证系统、跨平台与最终 ADR

### 12.1 目标

把原型从“本机看起来正确”提升为可持续验证的工程证据。

### 12.2 测试层级

#### Semantic tests

- role → style；
- object identity → shared color；
- override priority；
- palette provenance；
- Experiment/Fit/Theory mapping。

#### Layout snapshots

- axes rect；
- tick values；
- text bounds；
- glyph IDs/positions；
- legend rect；
- clip regions；
- path bounds。

#### Backend structural tests

- PDF page boxes、fonts、text、images；
- 可选 SVG 的 viewBox、paths、text、clipPath（非阻断）；
- PNG dimensions、alpha、DPI metadata。

#### Visual regression

- expected PNG；
- actual PNG；
- diff PNG；
- perceptual threshold；
- grayscale variant；
- CVD reference variant；
- multiple DPI。

### 12.3 三平台矩阵

Windows、macOS、Linux 各验证：

- build；
- deterministic TeX Gyre Heros four-face metrics；
- Greek Core、Scientific Symbol Core 和 unsupported-script diagnostics；
- preview；
- PDF；
- 可选 SVG（非阻断）；
- headless PNG；
- file opening；
- text extraction；
- output hashes or normalized structural snapshots。

### 12.4 依赖审计

- license；
- MSRV；
- transitive dependencies；
- native system libraries；
- bundled assets；
- advisories；
- release activity；
- version pinning；
- update policy。

### 12.5 最终技术报告

总结：

- selected stack；
- rejected alternatives；
- benchmark and file sizes；
- cross-platform differences；
- known limitations；
- unresolved risks；
- productionization work。

### 12.6 完成条件

- 所有 spike 输出可由 CI 重建；
- 关键 ADR 已接受；
- 三平台没有未解释的结构差异；
- visual regression 对真实错误敏感；
- 许可证允许当前发行目标。

---

## 13. GATE A：正式启动 Studio 的 Go/No-Go 门槛

只有全部满足才能进入 Part B：

### Figure and layout

- 最小 Figure IR schema 已确定；
- point-based Display List contract 已确定；
- 89 mm × 65 mm layout 正确；
- single-axes layout 稳定；
- tick、label、legend 和 clipping 通过。

### Text

- 唯一默认 shaper 已选定；
- TeX Gyre Heros 四个真实 face 已固定并完成授权检查；
- Latin/Greek/Scientific Symbol Core 正确；
- CJK 等非 V1 script 被明确拒绝且不产生系统 fallback；
- italic/upright/subscript/superscript 正确；
- 实际 resolved font、版本和可嵌入状态可记录；
- missing glyph 或不可嵌入字体有明确 warning。

### Export

- PDF 页面尺寸正确；
- PDF path 为 vector；
- PDF 字体嵌入/subset；
- PDF 文字可搜索和复制；
- PNG 300/600/1200 dpi 正确；
- TIFF 不属于 Gate A 必需项，但属于 V1 release 必需项；
- 所有 backend 使用同一 layout result。

### Color and visual quality

- Part A 最小 palette contract 已固定：Tol Bright、Tol High Contrast、source-backed diverging、fixed-provenance neutral gray；
- palette 来源、版本、授权和派生规则已记录；
- 基础 fixture 通过 grayscale 和 CVD 检查；
- Publication Visual Benchmark Corpus 已建立，并用于校准比例、字体、线宽、marker、tick、margin、legend 和色彩默认值；
- benchmark 只作为视觉系统参照，不做受版权约束图形的 pixel-copy。

### Size and UI shell

- release executable/app core 加 mandatory runtime assets 的测量边界固定；
- 目标为 `≤10 MiB`，`≤12 MiB` 为 soft ceiling，`>15 MiB` 触发架构复审并阻止 Gate A；
- UI shell 已验证 window、canvas、side inspector、file dialog 和 keyboard input；
- Windows scaling 完成人工验证；macOS Retina 与 Linux HiDPI 在无设备或已明确
  延后的情况下不阻挡 Gate A，但发布声明相应平台支持前必须补验；
- export 与 window size、screen scale 和 UI 生命周期无关；
- cold startup、idle memory 和 release size 已记录为可复测 baseline。

### Engineering

- Plotine ADR 已完成；
- 依赖许可证审计通过；
- deterministic snapshots 已建立；
- visual regression 已建立；
- Windows/macOS/Linux 已验证；
- size、startup 和 idle-memory evidence 已归档；
- Lite 未被修改或回归。

### Go

所有必需项通过，已知限制不破坏 V1 目标，可以开始正式产品工程。

### Conditional Go

只允许对明确推迟到 V1 之后的能力做 conditional go，例如：

- multi-panel；
- GPU preview。

完整 LaTeX/扩展 LaTeX-compatible syntax 是 V1 明确非目标，不作为 conditional-go 项目。TIFF 可不进入 Gate A，但必须在 V1 release 前完成。核心文字、PDF、物理尺寸、共享布局和 size review 不得 conditional go。

### No-Go

出现以下任一项则暂停正式开发：

- preview/export 必须维护两套布局；
- PDF 无法保留真实文字；
- 字体无法合法嵌入；
- 输出尺寸依赖 window size；
- TeX Gyre Heros 四个 face 的 metrics 不稳定；
- Core 字符缺失或 unsupported script 被静默 fallback；
- UI shell 使产物超过 `15 MiB` 且没有已验证的减重路线；
- 候选依赖许可证与发行目标冲突；
- 原型无法通过结构化自动测试。

No-Go 不表示取消项目，而是返回对应 spike 更换技术路线。

---

# Part B：正式产品开发

## 14. B0：抽取 Lite 共享核心

### 14.1 目标

让 Lite 和 Studio 使用一套数据实现，同时保持两个独立软件。

### 14.2 建议抽取顺序

每一步独立完成和验证：

1. `instplot-core`：data model、identity、shared error；
2. `instplot-io`：text/XLS/XLSX import 和 data export；
3. `instplot-processing`；
4. `instplot-fitting`；
5. Lite application package；
6. workspace root。

不要一次性移动所有文件。

### 14.3 每步要求

- 先移动，不改变行为；
- 保留 public API 最小化；
- Lite tests 继续通过；
- 数据 round-trip 不变；
- release size 和 memory 无显著回退；
- Studio-specific dependencies 不进入 Lite binary。

### 14.4 完成条件

- Lite 成为共享 crates 的消费者；
- 原型也能消费共享 data model；
- 生产实现没有复制；
- Lite 可独立构建、测试和发布。

---

## 15. B1：建立独立 Studio 应用

### 15.1 目标

创建独立 binary 和最小应用壳，不复制 Lite app。

### 15.2 最小功能

- 独立窗口和产品身份；
- 打开共享核心支持的数据；
- 创建一个 Figure Document；
- 显示原型 preview backend；
- series tree；
- basic property inspector；
- export fixed test figure；
- error/warning panel。

### 15.3 禁止

- 复制 `src/app.rs` 后重命名；
- 把 Lite 所有处理窗口搬进 Studio；
- 在 UI widget 中存储唯一 figure state；
- 在这一阶段新增复杂 plot types。

### 15.4 完成条件

- Studio 独立 binary；
- 不安装 Studio 仍可完整使用 Lite；
- Studio 能读取 Lite 数据；
- UI 只是 Figure Document 的编辑器。

---

## 16. B2：正式 Figure Document 与项目格式

### 16.1 正式 schema

包含：

- schema version；
- producer version；
- data source records；
- embedded/reference policy；
- axes/artists；
- stable IDs；
- semantic registry；
- palette registry；
- typography；
- overrides；
- export preferences；
- provenance；
- migrations。

### 16.2 项目格式候选

通过 ADR 选择：

- ZIP container + JSON manifest + binary data；
- 单 JSON；
- 其他版本化容器。

必须支持：

- atomic save；
- unknown-field strategy；
- backup/recovery；
- migration tests；
- external source missing；
- source changed detection；
- no silent data replacement。

### 16.3 完成条件

- create/save/open round-trip；
- old fixture migration；
- source/fit identity 保持；
- palette/font provenance 保持；
- 相同项目生成相同 resolved figure。

---

## 17. B3：V1 出版绘图能力

### 17.1 图形范围

- single axes；
- line；
- scatter；
- line + marker；
- X/Y error bar；
- fit；
- theory；
- reference；
- baseline；
- annotation；
- legend；
- linear/log axis。

### 17.2 编辑范围

- data source/columns；
- scientific role；
- semantic identity；
- axis range；
- linear/log；
- quantity/unit label；
- figure size；
- text scale；
- palette semantics；
- explicit style override；
- legend label/order/location policy。

### 17.3 不做

- spreadsheet；
- full data-cleaning UI；
- general fitting platform；
- 3D；
- animation；
- dashboard；
- arbitrary vector editor；
- multi-user/cloud；
- plugin system；
- complex multi-panel；
- full LaTeX 或扩展 LaTeX-compatible math syntax。

---

## 18. B4：颜色、语义与 Publication Check

### 18.1 Palette Registry

- version-pinned source data；
- checksum；
- provenance class；
- type；
- source/reference；
- recommended/not recommended；
- sampling；
- derived subset；
- CVD/print/monochrome 独立状态。

### 18.2 Semantic Registry

- object color；
- object marker；
- ordered variable palette；
- diverging center/direction；
- Experiment/Fit/Theory roles；
- neutral gray；
- paper-wide consistency。

### 18.3 Publication Check

分级：

- Error；
- Warning；
- Information。

V1 检查：

- physical size；
- font size；
- stroke width；
- font embedding；
- clipping；
- legend overlap；
- color-only encoding；
- palette/data relationship；
- grayscale distinguishability；
- CVD risk；
- raster DPI/pixels；
- transparency；
- provenance completeness。

### 18.4 完成条件

- 每条检查规则有 fixture；
- warning 指向具体 node；
- override 可记录理由；
- 检查结果随文档和导出参数变化而更新；
- 规则版本写入报告。

---

## 19. B5：Lite → Studio 交接

### 19.1 第一阶段

继续使用现有分区文本/XLSX：

- source；
- fit；
- Parent-ID；
- Source-X；
- Source-Y；
- equation；
- display equation。

### 19.2 第二阶段

实现“在 InstPlot Studio 中打开”：

- Lite 生成版本化临时交换包；
- 包含完整数据、alive state、column identity、fit links；
- 启动 Studio；
- Studio 创建新的 Figure Document；
- 正式保存后不再依赖临时文件。

### 19.3 完成条件

- 数据无损；
- source/fit 自动关联；
- 不通过文件名猜测关联；
- Studio style 不回写 Lite 源数据；
- 临时文件有安全清理策略；
- 交换格式有兼容测试。

---

## 20. B6：发布准备

### 20.1 发布物

- Windows installer；
- macOS Apple Silicon DMG；
- Linux DEB；
- Linux portable archive；
- third-party notices；
- font/palette licenses；
- checksums；
- release notes。

### 20.2 发布验收

- clean-machine install；
- offline launch；
- sample project；
- Lite export → Studio open；
- PDF/PNG/TIFF export；
- publication check；
- crash recovery；
- project migration；
- uninstall；
- no Python/Excel/runtime requirement。

### 20.3 V1 Definition of Done

用户可以：

1. 从 Lite 或普通数据文件导入；
2. 指定 X/Y、科学角色和数据关系；
3. 不调整几十项样式即可得到清晰图形；
4. 在 89 mm × 65 mm 最终尺寸预览；
5. 查看灰度/CVD 风险；
6. 通过 Publication Check；
7. 导出具有真实文字和嵌入字体的矢量 PDF；
8. 导出正确 DPI 的 PNG 和 TIFF；TIFF 是 V1 release 必需项，不是 Gate A 必需项；
9. 保存并重新打开项目而不发生样式和颜色漂移。

---

## 21. 任务依赖与可并行性

### 必须串行

- A1 → A2：没有固定合同不能评估 IR；
- A2 → A3/A4：没有 Display List 不能判断后端一致性；
- A3 → A4：PDF 真实文字依赖 glyph contract；
- A2 → A6：UI shell 必须承载真实 Display List，而不是另建绘图模型；
- A3/A4/A5 → A7：布局需要已确定的文字测量、后端能力和复用边界；
- A6/A7 → A8：最终验证必须覆盖 UI shell 与 layout；
- A8 → GATE A；
- GATE A → Part B；
- B0 → B1：正式 Studio 应使用共享核心；
- B2 → B4/B5：检查和交接依赖稳定 schema。

### 可以有限并行

- A3 字体许可证调查与 shaper coding；
- A4 PDF 与可选 SVG backend；
- A5 Plotine 对照与 A4 后端实现；
- A6 UI shell 可在 A2 完成后与 A3–A5 有限并行，但 export independence 验收要等 A4；
- A8 测试工具可与 A7 layout 有限并行；
- B3 图元与 B4 palette registry；
- B6 packaging definitions 与后期功能稳定化。

### 禁止提前开始

- GATE A 前不做完整 Studio property editor；
- GATE A 前不迁移整个 Lite workspace；
- text engine 未选定前不冻结项目格式；
- Display List 未稳定前不做多 panel；
- PDF 真实文字未通过前不宣传 publication-ready；
- V1 未稳定前不做插件、云端协作或 3D。

---

## 22. 变更与评审策略

每个变更应尽量满足：

- 一个明确目标；
- 一个可验证输出；
- 不混合机械移动与行为修改；
- 不修改无关代码；
- 有测试或 ADR；
- 能独立回退；
- 对 Lite 影响明确。

推荐按以下粒度组织：

- fixture change；
- IR contract；
- one backend；
- one text candidate；
- one ADR；
- one shared crate extraction；
- one Studio feature vertical slice。

不推荐：

- “创建 Studio 基础架构”式大改；
- 同时更换 GUI、数据模型、渲染器和项目格式；
- 无测试的大规模文件移动；
- 依赖升级与功能实现混在一起。

---

## 23. 风险登记

### R1：字体跨平台漂移

- 影响：布局和导出不一致；
- 控制：固定 TeX Gyre Heros 四个 face、checksum、single shaper、layout snapshot；非 V1 script 明确拒绝；
- gate：A3。

### R2：PDF 看起来正确但不可编辑

- 影响：违反出版规范；
- 控制：structural PDF test、text extraction、font inspection；
- gate：A4。

### R3：预览和导出分叉

- 影响：用户无法信任预览；
- 控制：single Display List、backend 无 layout logic；
- gate：A2/A4。

### R4：过早依赖不成熟 crate

- 影响：被 API 和维护状态锁定；
- 控制：adapter、version pin、comparison spike、exit plan；
- gate：A5/A8。

### R5：共享核心抽取破坏 Lite

- 影响：现有产品退化；
- 控制：baseline、小步移动、行为不变、每步测试；
- gate：B0。

### R6：项目格式过早冻结

- 影响：长期兼容负担；
- 控制：原型格式不承诺、GATE A 后设计正式 schema；
- gate：B2。

### R7：功能范围膨胀

- 影响：核心出版链路延迟；
- 控制：V1 non-goals、禁止提前开始列表；
- gate：所有阶段评审。

### R8：许可证冲突

- 影响：无法按当前方式发行；
- 控制：dependency/asset audit、AGPL clean-room 边界；
- gate：A8。

### R9：依赖和资源使产品失去轻量化

- 影响：启动、分发和产品定位偏离目标；
- 控制：统一 size boundary、每次引入生产依赖都记录增量、feature pruning、可选资源与核心分离；
- gate：A0/A3/A4/A6/A8。

### R10：结构正确但默认视觉质量不足

- 影响：技术验收通过，用户仍需大量手调才能达到出版质量；
- 控制：Publication Visual Benchmark Corpus、参数化视觉测量、专家评审、非 pixel-copy 的默认系统校准；
- gate：A1/A7/A8。

---

## 24. 执行状态记录方式

每个阶段维护：

```text
Status: NOT_STARTED | ACTIVE | BLOCKED | DONE
Inputs:
Outputs:
Acceptance:
Evidence:
ADRs:
Known limitations:
Next dependency:
```

完成阶段时只报告可验证事实，不以“代码已经很多”或“界面看起来不错”代替验收。

---

## 25. 当前第一个可执行任务

Part A 与 Gate A 已按当前范围完成。下一项工作是：

> **B3.2 — 固定正式 axes rectangle、spines、ticks、grid、labels 与 clipping。**

具体范围和验收条件见
[`INSTPLOT_STUDIO_PART_B_SHORT_PLAN.md`](INSTPLOT_STUDIO_PART_B_SHORT_PLAN.md)。
B0 已完成，完整证据见
[`../reports/B0_SHARED_CORE_CLOSEOUT.md`](../reports/B0_SHARED_CORE_CLOSEOUT.md)；早期盘点和
分步证据仍保留在各 B0 报告中。B1 完整证据见
[`../reports/B1_STUDIO_APPLICATION_CLOSEOUT.md`](../reports/B1_STUDIO_APPLICATION_CLOSEOUT.md)，
B2 完整证据见
[`../reports/B2_PROJECT_FORMAT_CLOSEOUT.md`](../reports/B2_PROJECT_FORMAT_CLOSEOUT.md)。
B3.1 正式布局模块提升证据见
[`../reports/B3_1_LAYOUT_PROMOTION.md`](../reports/B3_1_LAYOUT_PROMOTION.md)。
后续按 B3 → B4 → B5 → B6 顺序执行。

---

## 26. 最终原则

Studio 的成功标准不是尽快出现一个新窗口，而是建立一条可信的出版链路：

```text
科学语义
  → 可复现文档
  → 确定性布局
  → 单一 Display List
  → 多后端一致输出
  → 自动验证
```

只有这条链路通过验证，完整产品开发才开始。这样既能避免重复造轮子，也能避免把未经验证的第三方组件或临时截图方案固化为长期架构。
