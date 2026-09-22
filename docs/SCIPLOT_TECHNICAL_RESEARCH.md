# InstPlot Studio 技术背景调查与验证路线

> 文档性质：正式开发前的技术调查、候选评估与原型验收规范  
> 状态：Research Draft V1  
> 调查日期：2026-09-20  
> 适用产品：InstPlot Studio
> 相关文档：`SCIPLOT_PRODUCT_BOUNDARY.md`、`PUBLICATION_PLOTTING_DESIGN_SPEC.md`、`scientific-publication-color-system-spec.md`

> 范围更新（2026-09-21）：ADR-021 已将 SVG 降为非阻断可选实验。本文中
> 对 SVG 的候选调查仍作为技术背景保留，但所有“必须”“通过条件”和启动门槛
> 均以 PDF/PNG 为准；SVG 不再是 Gate A 或 V1 发布条件。

## 1. 目的

InstPlot Studio 的产品目标和双产品边界已经基本明确，但正式开发前仍存在高风险技术问题：

- 屏幕预览与 PDF/PNG/TIFF 如何共享同一布局，以及可选 SVG 如何复用该结果；
- 如何以 mm 和 pt 而不是屏幕像素定义图形；
- 如何进行字体发现、fallback、shaping、数学排版和 PDF 字体嵌入；
- 如何设计可持久化、可验证的 Figure Document；
- 如何在 Rust 生态中复用成熟组件，而不是重新实现字体、PDF 或 SVG；
- 如何借鉴类似项目，同时避免不兼容许可证；
- 如何证明所选技术路线能够满足出版规范。

本文的目标不是罗列绘图库，而是形成可执行的技术判断：

1. 哪些架构已经被类似项目验证；
2. 哪些 Rust crate 值得进入原型；
3. 哪些能力必须由 InstPlot 自己掌握；
4. 哪些路线不适合作为 Studio 的核心；
5. 原型达到什么标准后才允许进入正式产品开发。

---

## 2. 调查结论摘要

### 2.1 推荐路线

Studio 应采用以下主干架构：

```text
共享数据核心
    ↓
Figure Document / Semantic IR
    ↓
Layout Compiler
    ↓
Backend-neutral Display List（统一使用 pt）
    ├── egui Preview Backend
    ├── PDF Backend
    ├── SVG Backend
    └── Raster Backend → PNG / TIFF
```

关键决策：

- `Figure Document` 是唯一持久化事实来源；
- `Display List` 是所有输出的唯一绘制事实来源；
- 预览和导出不得分别计算 tick、文字、legend 或 axes 边界；
- UI 不拥有出版布局；
- PDF、SVG、PNG/TIFF 是同一 Display List 的不同后端；
- 所有 figure-space 坐标和尺寸统一使用 point，`1 pt = 1/72 inch`；
- mm 只在用户输入、文档 metadata 和单位转换边界出现；
- 屏幕像素只在预览缩放和 raster 输出边界出现；
- release executable/app core 加 mandatory runtime assets 的架构目标为 `≤10 MiB`，`≤12 MiB` 为 soft ceiling，`>15 MiB` 必须架构复审；
- InstPlot Studio V1 bundle TeX Gyre Heros 四个真实 face；CJK 等非 V1 script 明确拒绝，不使用系统 fallback；
- Part A 即固定最小 color contract，并用 Publication Visual Benchmark Corpus 校准默认视觉系统；
- Gate A 前必须完成极小 UI shell spike，验证 HiDPI、scaling、file dialog、keyboard、启动、内存和体积。

### 2.2 初步组件建议

建议进入技术原型的组件：

- GUI：继续使用 `eframe/egui`；
- 几何与 path：`kurbo`；
- 普通文字布局：优先验证 `Parley + fontique + HarfRust + skrifa`；
- 基础数学排版：V1 只实现 InstPlot 自己的语义 Label AST；完整 LaTeX 或扩展 LaTeX-compatible syntax 仅作未来候选；
- PDF：优先验证 `Krilla`；
- SVG：由 InstPlot Display List 后端直接生成，`usvg` 用于解析验证；
- PNG/TIFF：优先验证 `resvg/tiny-skia` 或直接 Display List CPU raster backend；
- 色彩计算：`palette`；
- palette 数据：由 InstPlot 自己版本锁定并保存 provenance，不直接依赖某个 crate 的默认集合；
- 序列化：版本化 Rust model + Serde；项目容器格式在原型后单独决策。

### 2.3 不建议作为核心的路线

- 不以 `egui_plot` 作为 Studio 排版与导出核心；
- 不从 Lite 的屏幕截图生成出版文件；
- 不让 PDF 通过另一套独立布局代码重新画一次；
- 不把完整 Typst 编译器作为第一版图形布局核心；
- 不直接依赖 AGPL 的 IronLAB，除非未来明确决定把相关产品整体切换到 AGPL；
- 不直接把 Plotters 的像素式 chart API 当作 Figure Document；
- 不在第一版引入 GPU 专用场景作为唯一事实来源。

---

## 3. 最接近目标的 Rust 项目

## 3.1 IronLAB：最有价值的架构参考

IronLAB 是当前调查中与 InstPlot Studio 技术目标最接近的 Rust 项目。它已经实现或明确设计了：

- retained Figure IR；
- 固定物理尺寸 figure；
- stable node identity；
- 数值数组与图形节点分离；
- Figure IR 编译为 backend-neutral Display List；
- Display List 的坐标单位为 point；
- egui 交互式 viewer；
- 同一 Display List 生成预览和 PDF；
- HarfRust 文字 shaping；
- 数学排版；
- Krilla PDF；
- 字体嵌入和 subsetting；
- 可搜索、选择和复制的 PDF 文字；
- Protocol Buffers `.fig` 与 JSON 调试格式；
- validation report；
- source figure 与交互 view overlay 分离。

关键参考资料：

- [IronLAB 总体 API](https://docs.rs/ironlab/latest/ironlab/)
- [Retained Figure IR](https://docs.rs/ironlab-ir/latest/ironlab_ir/)
- [Scene compiler](https://docs.rs/ironlab-scene/latest/ironlab_scene/)
- [Backend-neutral Display List](https://docs.rs/ironlab-scene/latest/ironlab_scene/display/)
- [egui viewer](https://docs.rs/ironlab-viewer/latest/ironlab_viewer/)
- [PDF backend](https://docs.rs/ironlab-pdf/latest/ironlab_pdf/)
- [Text and math engine](https://docs.rs/ironlab-text/latest/ironlab_text/)

### 3.1.1 最值得借鉴的设计

#### 单一 Figure IR

Figure IR 是所有图形状态的唯一事实来源，而不是把状态散落在 widget、renderer 和导出窗口中。

#### 编译阶段

Figure IR 不直接交给每个后端解释，而是先编译成已经完成布局、文字定位和裁切的 Display List。后端只负责忠实绘制。

#### point 坐标

Display List 全部使用 point，物理尺寸从进入布局层开始就稳定。PDF 页面的 MediaBox 和 CropBox 与 figure 尺寸一致。

#### 预览与 PDF 共用图元

交互 viewer 和 PDF exporter 消费同一个 Display List，避免出现两套 tick、legend 或字体测量逻辑。

#### View Overlay

pan、zoom、隐藏曲线等临时交互状态保存在 overlay 中，不直接破坏源 Figure。这非常适合 Studio 的“编辑状态”和“文档状态”分离。

### 3.1.2 许可证边界

IronLAB 0.5.5 的 crates 使用 `AGPL-3.0-or-later`。当前 InstPlot Lite 使用 MIT License。

因此，在保持 InstPlot 产品为 MIT 的前提下：

- 可以研究公开文档、行为和通用架构思想；
- 不应复制其实现代码；
- 不应直接把 IronLAB crate 链接进发行版；
- 不应以修改后 vendoring 的方式绕过许可证；
- 如未来考虑直接依赖，必须先做独立许可证决策，并接受 AGPL 对整个组合发行方式的影响。

本文默认采用 clean-room 原则：学习架构，不复制实现。

## 3.2 Plotine：最值得做依赖验证的 MIT 候选

Plotine 是一个 MIT 许可的 Rust 原生科学绘图库，已经提供：

- core geometry、scale 和 tick；
- backend-neutral rendering traits；
- SVG、PDF、PNG 和 PGF 后端；
- mathtext；
- GUI 功能；
- `kurbo` 几何；
- `cosmic-text` 文字 shaping；
- SVG → PDF 路径。

资料：

- [Plotine 项目](https://github.com/AIGO3fz/plotine)
- [Plotine mathtext](https://docs.rs/plotine/latest/plotine/mathtext/)
- [Plotine PDF backend](https://docs.rs/crate/plotine-backend-pdf/latest)

### 3.2.1 优点

- MIT 与当前项目兼容；
- 已经拆分 core、render、text、backend；
- 覆盖的基础图形和导出格式较多；
- 可以帮助验证哪些算法值得复用；
- 可能显著降低 tick、scale、path 和 mathtext 的初期工作量。

### 3.2.2 风险

- 项目仍处于较早版本，API 和架构稳定性需要验证；
- 产品目标较广，与 InstPlot 的“语义驱动、出版检查”并不相同；
- PDF 当前走 SVG 转换链路，必须验证真实文字、字体嵌入、subsetting 和跨平台稳定性；
- 当前 SVG→PDF 依赖链需要核对长期维护状态；
- GUI、动画、3D 等能力不应被无意引入 Studio 核心；
- 不能因为它“能导出 PDF”就假设满足我们的出版规范。

### 3.2.3 建议

不立即把 Studio 建立在 Plotine 之上。先用相同验收样例比较：

1. 直接使用 Plotine 输出；
2. InstPlot 最小 Display List + Krilla/resvg 输出。

如果 Plotine 能通过字体、物理尺寸、可搜索文字、SVG/PDF 一致性和自定义语义扩展测试，则优先复用其低层 crate；否则只借鉴算法或将其作为对照实现。

## 3.3 Plotters：成熟的静态绘图参考，不是 Studio 文档模型

Plotters 是 MIT 许可的 Rust 绘图库，提供 bitmap、SVG 和多种 drawing backend，并具有 DrawingArea、ChartContext、series 和 element 模型。

资料：[Plotters 官方仓库](https://github.com/plotters-rs/plotters)

适合借鉴或复用的部分：

- scale 和 coordinate mapping；
- tick/mesh 生成思路；
- series 与 element 抽象；
- backend trait 的边界；
- 基础 error bar、line、point 等图元。

不适合作为 Studio 核心的原因：

- 它主要是程序化绘图库，不是可编辑 Figure Document；
- 默认 API 强烈围绕输出尺寸和 drawing area；
- 不提供我们所需的 paper-wide semantic registry；
- 不解决项目持久化、科学角色、palette provenance 和 Publication Check；
- PDF、字体和复杂出版排版仍需要另行解决。

## 3.4 rsplot：交互和大数据预览参考

rsplot 是 MIT/Apache-2.0 的 egui + wgpu 科学绘图组件，强调交互、ROI、GPU 数据层和多种图片导出。

资料：[rsplot 官方仓库](https://github.com/physwkim/rsplot)

值得研究：

- egui 与 wgpu 数据层结合；
- 曲线更新和 picking；
- 大数据交互；
- 后端 item registry；
- PNG/TIFF/SVG 输出入口。

不建议作为 Studio 的出版事实来源：

- 主要目标是高性能交互；
- GPU preview 与出版 vector scene 是不同问题；
- 物理尺寸、真实文字和 PDF 字体嵌入仍需独立验证。

Studio V1 的二维曲线规模可先沿用 Lite 的保峰降采样思路，不应为了潜在的大数据需求过早引入复杂 GPU renderer。

## 3.5 egui_plot：保留在 Lite，Studio 只作过渡参考

`egui_plot` 官方定位是“Simple plotting library for egui”，当前 Lite 已成功使用它完成数据检查、zoom、pan、picking 和快速 PNG。

资料：[egui_plot 文档](https://docs.rs/egui_plot/latest/egui_plot/)

它适合 Lite，但不满足 Studio 核心要求：

- 布局以 UI point 和 widget 生命周期为中心；
- 不是独立 Figure Document；
- 不是出版矢量后端；
- 无法自然保证 PDF/SVG 与屏幕布局相同；
- 字体与数学排版由 GUI 环境主导。

## 3.6 Vello：未来预览后端候选，不是第一阶段出口

Vello 是 MIT/Apache-2.0 的高性能 Rust 2D renderer。当前项目同时提供较成熟的 CPU renderer、GPU renderer 和实验性 compute renderer，并有 retained Scene 与 glyph-run 支持。

资料：[Vello 官方仓库](https://github.com/linebender/vello)

适合未来考虑：

- 大量矢量路径的高性能屏幕预览；
- CPU fallback；
- 复杂 clipping 与 compositing；
- 与 Linebender 文字和几何生态结合。

不应成为第一阶段关键依赖：

- Vello 主要负责 raster rendering，不直接解决 PDF/SVG；
- Studio V1 的瓶颈是正确布局和文字，不是 GPU 吞吐量；
- 若先以 Vello Scene 为唯一 IR，会让导出后端被迫反向适配 GPU renderer 的模型。

正确关系应是：InstPlot Display List 在未来可以增加 Vello preview backend，而不是让 Figure Document 直接依赖 Vello。

---

## 4. 成熟生态提供的架构经验

## 4.1 Matplotlib：Figure → Axes → Artist 与 backend 分离

Matplotlib 的经验仍然重要：Axes 包含 Axis artists、data artists、labels、legends，并通过 backend 输出。其 constrained layout 会测量 tick labels、axis labels、title 和 legend，再调整 margins。

资料：

- [Matplotlib Axes architecture](https://matplotlib.org/stable/users/explain/axes/index.html)
- [Constrained layout](https://matplotlib.org/stable/users/explain/axes/constrainedlayout_guide.html)
- [Mathtext](https://matplotlib.org/stable/users/explain/text/mathtext.html)

可吸收的经验：

- Figure、Axes、Axis、Artist 必须分层；
- scale、locator、formatter 应是独立对象；
- layout 必须基于实际文字测量；
- decoration 需要参与 margin 计算；
- 复杂 constraint solver 并非总能得到符合用户意图的结果；
- 不同 backend 的字体处理会造成细微差异，因此必须尽量统一 shaping 和 glyph positioning。

Studio V1 只有单 axes，不需要立即复制完整 constrained-layout solver。应先实现确定性的单 axes 两阶段布局：

```text
初始 axes proposal
  ↓
生成 tick + shape text + 测量 decoration
  ↓
求出四侧 minimum margins
  ↓
重新计算 axes
  ↓
最多进行有限次数稳定迭代
```

## 4.2 Typst 与 Krilla：排版和 PDF 的职责分离

Typst 使用 Rust 实现排版，并将 PDF 输出建立在 Krilla 之上。Typst 目前可以输出 PDF、SVG 和 PNG；Krilla 专门接收已经 layout 好的内容并生成 PDF。

资料：

- [Typst 官方仓库](https://github.com/typst/typst)
- [Typst PDF export](https://github.com/typst/typst/blob/main/docs/content/reference/export/pdf.typ)
- [Krilla 官方仓库](https://github.com/LaurenzV/krilla)

这验证了一个适合 InstPlot 的边界：

> Studio 自己负责 Figure 和文字布局，Krilla 负责把已经定位的路径、glyph、图片和 metadata 写成 PDF。

不建议让 Studio 生成 Typst 源代码再调用完整编译器，因为：

- Figure Document 与 Typst source 会形成两套事实来源；
- 交互式局部更新更加困难；
- 自定义 plot picking 和 editor overlay 不自然；
- 编译器和标准库会增加依赖体积与维护面；
- Studio 并不需要一般文档分页能力。

Typst 更适合作为质量参照，Krilla 更适合作为直接候选依赖。

---

## 5. 推荐的 Studio 核心分层

## 5.1 Layer 1：共享数据核心

直接来自 Lite 抽取：

- `DataSet`；
- `NumericColumn`；
- dataset identity；
- source/fit link；
- parsers；
- processing；
- fitting；
- data export。

本层不知道 Figure、Axes、PDF 或 UI。

## 5.2 Layer 2：Figure Document / Semantic IR

这是 Studio 自己必须掌握的产品核心。建议最低模型：

```text
FigureDocument
├── schema_version
├── producer_version
├── figure_size_mm
├── data_sources
├── axes[]
├── artists[]
├── semantic_registry
├── palette_registry
├── typography
├── publication_profile
└── export_preferences
```

每个节点必须具有稳定 ID。Series 不保存重复数据，而是引用：

```text
dataset_id + x_column_id + y_column_id
```

Series 同时保存：

- scientific role；
- semantic object identity；
- data relationship；
- explicit overrides；
- legend label；
- visibility；
- draw order。

自动样式必须由 semantic state 推导，不能在第一次创建时永久展开为一堆失去来源的裸颜色和线宽。

## 5.3 Layer 3：Resolved Figure

Figure Document 中可能存在自动值，例如：

- palette = Auto；
- marker = Auto；
- axis limits = Auto；
- legend location = Auto；
- tick count = Auto。

布局前先进行 resolve：

```text
Figure Document + Style Rules + Data Summary
    ↓
Resolved Figure
```

Resolved Figure 包含已经确定但尚未定位的样式。这样 Publication Check 可以区分：

- 用户明确指定的值；
- 软件自动决定的值；
- 来自全局语义 registry 的值；
- 来自默认规范的值。

## 5.4 Layer 4：Layout Compiler

输入：Resolved Figure、数据摘要、字体资源。  
输出：Display List、hit map、layout warnings。

职责：

- axis scale 和 transform；
- locator 和 formatter；
- text shaping 和 measurement；
- margins；
- axes rectangle；
- tick geometry；
- series geometry；
- marker geometry；
- error bars；
- legend placement；
- clipping；
- draw order；
- source node → display item mapping。

Layout Compiler 必须是纯逻辑，不能依赖 egui window、GPU device 或文件保存对话框。

## 5.5 Layer 5：Display List

建议的最小图元：

```text
DisplayList
├── Path
│   ├── geometry
│   ├── fill
│   ├── stroke
│   └── source_node_id
├── GlyphRun
│   ├── font resource
│   ├── source text
│   ├── glyph ids
│   ├── positioned glyphs
│   └── source_node_id
├── Image
├── ClipPush
├── ClipPop
└── Metadata
```

Display List 的不变量：

- 所有坐标和宽度有限；
- 所有坐标均为 figure-space pt；
- 每个 path 明确 fill rule；
- stroke 明确 cap、join、dash；
- clip 成对且作用域明确；
- glyph run 同时保留 source Unicode text 和 positioned glyphs；
- 所有可交互项保留 source node ID；
- 后端遇到非法 item 时报告并跳过，不 panic。

## 5.6 Layer 6：Backends

后端只做格式映射，不重新进行科学或布局决策。

### Preview backend

- 把 path tessellate 为 egui mesh；
- 使用同一 glyph positioning；
- 将 figure pt 通过一个 screen transform 映射到逻辑像素；
- picking 使用 Layout Compiler 产生的 hit map；
- pan/zoom 形成 view overlay，不直接改变文档，除非用户提交编辑。

### PDF backend

- 使用 Krilla；
- page MediaBox/CropBox 等于 figure size；
- path 保持矢量；
- glyph run 写为真实文字；
- 字体嵌入和 subset；
- 保留 source text，确保搜索与复制；
- 大量密集图元允许按明确策略局部 rasterize，不能整页静默 rasterize。

### SVG backend

- `width`、`height` 使用物理单位；
- `viewBox` 使用相同 pt 坐标；
- path 和 marker 保持矢量；
- text 默认保留为 text；
- 可提供“portable outlines”高级选项，但不得替代默认真实文字；
- font family 和 fallback 必须明确。

### Raster backend

- 从 Display List 或其 SVG 表达进行 CPU rasterization；
- DPI 只改变像素密度，不改变排版；
- mm → px 使用统一 rounding policy；
- PNG/TIFF 共享同一 RGBA buffer；
- 输出前执行透明背景和色彩空间检查。

---

## 6. Rust 组件详细评估

## 6.1 Geometry：kurbo

`kurbo` 提供 path、affine transform、rect、stroke 等 2D geometry 类型，并已被 Vello、IronLAB、Plotine 等项目采用。

结论：适合用作 Display List 和 marker geometry 的基础。版本必须在 workspace 中统一，避免不同依赖携带不兼容的 `kurbo` 类型。

## 6.2 Plain text：Parley + fontique + HarfRust + skrifa

Parley 提供 styled ranges、font selection、shaping、line layout 和测量；fontique 负责系统字体枚举和 fallback；HarfRust 负责 shaping；skrifa 负责 OpenType font metadata 和 glyph outlines。

资料：

- [Parley concepts](https://github.com/linebender/parley/blob/main/doc/concept.md)
- [HarfRust](https://github.com/harfbuzz/harfrust)
- [Vello](https://github.com/linebender/vello)

优点：

- Rust 原生；
- MIT/Apache 或 MIT；
- rich text range；
- 系统字体和 fallback；
- 可获得 glyph ID、advance、baseline 和 outlines；
- 与 kurbo/peniko/Vello 生态接近。

需要验证：

- Latin、Greek 与 V1 Scientific Symbol Core 的统一字体覆盖；
- italic variable 与 upright unit 的 range shaping；
- 同一 shaped run 写入 Krilla 后的 glyph positioning；
- macOS/Windows/Linux 使用 bundled TeX Gyre Heros 时是否完全一致；
- unsupported script 是否在 shaping 前产生稳定、明确的错误；
- 是否存在未声明的系统字体 fallback。

推荐策略：

- TeX Gyre Heros 四个 face 使用版本锁定的 bundled 资源；
- 项目保存 font family、font source、version/checksum 和 coverage 结果；
- CJK 等非 V1 script 不启用系统 fallback；
- Core 缺字阻止导出，非 Core 缺字产生 warning，不静默替换后继续导出。

## 6.3 另一文字候选：cosmic-text

`cosmic-text` 是成熟的纯 Rust 多行文字 shaping/layout/rendering 库，支持 bidi、font fallback，并被多个 GUI 项目使用。

资料：[cosmic-text 官方仓库](https://github.com/pop-os/cosmic-text)

它值得作为 Parley 的对照候选，特别是 Plotine 已经使用它。但 Studio 更关注：

- 精确 positioned glyph export；
- 同一 glyph run 跨 preview/PDF/SVG；
- font provenance；
- styled scientific label ranges。

因此不能只比较“屏幕上能不能显示”，而要比较能否稳定导出真实文字。

## 6.4 数学排版：语义 Label AST 优先

Studio V1 不应以“支持完整 LaTeX”为起点。产品规范真正需要的是：

- variable italic；
- number and unit upright；
- descriptive subscript upright；
- mathematical subscript italic；
- superscript；
- Greek glyph；
- common operators and functions。

建议建立：

```text
Label
├── Text
├── Variable
├── Upright
├── Greek
├── Subscript
├── Superscript
├── Unit
├── Operator
└── Group
```

UI 可以把简化输入语法解析成 AST，也可以通过 symbol/semantic editor 生成 AST。AST 再转换为 styled glyph runs。

完整 LaTeX 与扩展 LaTeX-compatible syntax 明确不进入 V1，也不进入 Gate A 的生产依赖集合。未来若实际用例证明 Label AST 不足，可把 `latex-rust` 作为独立、可选的扩展候选。它是 MIT/Apache-2.0 的纯 Rust LaTeX math renderer，可输出 SVG、PNG 或 egui shapes。

采用前必须验证：

- font 是否可由 Studio 控制；
- glyph 和 font bytes 是否能进入 Krilla；
- math baseline 是否能与普通文字对齐；
- SVG/PDF/preview 是否使用同一定位；
- crate 的 TeX 子集是否覆盖目标用例；
- 是否会引入第二套独立文字测量逻辑。

## 6.5 PDF：Krilla

Krilla 是 MIT/Apache-2.0 的高层 PDF 生成库，面向已经具有 layouted content IR 的项目；Typst 的 PDF backend 也使用它。

资料：[Krilla 官方仓库](https://github.com/LaurenzV/krilla)

与 Studio 的匹配点：

- page size；
- vector paths；
- clipping；
- glyph-based text；
- font embedding/subsetting；
- raster images；
- PDF standards 和 validation 支持；
- 不要求使用完整文档排版框架。

结论：Krilla 是当前 PDF 原型的第一候选。

## 6.6 SVG 与 raster：usvg/resvg/tiny-skia

resvg/usvg/tiny-skia 是 Rust 中成熟的 SVG 解析、规范化和 CPU rasterization 生态。

资料：[resvg 官方仓库](https://github.com/linebender/resvg)

推荐用途：

- 校验 Studio 生成的 SVG 是否可解析；
- 将 SVG 或 Display List rasterize 为 PNG；
- 生成 deterministic visual regression image；
- 作为 CPU、headless、CI 友好的参考 rasterizer。

注意：如果 PNG 仅通过重新解析 SVG 产生，应验证 text/font resolution 不会与 PDF backend 分叉。更稳妥的长期方案是让 SVG、PDF 和 CPU raster 都直接消费 positioned glyph Display List。

## 6.7 颜色：palette + 自有 registry

`palette` 是 MIT/Apache-2.0 的颜色转换和计算库，支持 Lab、Oklab、色差等。

资料：[palette 官方仓库](https://github.com/Ogeon/palette)

建议用它完成：

- sRGB 与 linear RGB 转换；
- 灰度和 luminance 相关计算；
- Lab/Oklab 色差；
- palette subset 的可区分度辅助检查。

内置 palette 本身不得直接由通用 crate 的常量决定。InstPlot 必须保存：

- 官方来源数据；
- source version；
- checksum；
- provenance class；
- sampling method；
- CVD/print/monochrome 独立状态。

颜色系统不能等到正式 UI 阶段才决定。Part A 至少固定：

- Distinct：Paul Tol Bright；
- High Contrast：Paul Tol High Contrast；
- Diverging：有明确原始来源与版本的 palette；
- Neutral gray：固定值并记录 provenance。

这组最小合同直接进入 Experiment/Fit/Theory、positive/negative/zero、legend、grayscale 和 CVD fixture。B4 再扩展为完整 registry、采样与 publication check，不重新定义基础语义。

CVD 模拟算法和参数也必须版本锁定，并用公开参考样例验证。

---

## 7. 最困难问题的具体实现策略

## 7.1 Preview 与 export 一致性

错误路线：

```text
egui_plot 负责预览
PDF library 重新计算布局
SVG library 再重新计算一次
```

正确路线：

```text
Figure Document
    ↓ resolve + layout + shape
Display List
    ├── Preview
    ├── PDF
    ├── SVG
    └── Raster
```

一致性并不意味着像素逐点相同，而是必须共享：

- 相同 axes rectangle；
- 相同 tick value；
- 相同 tick label source；
- 相同 shaped glyph IDs 和 positions；
- 相同 paths；
- 相同 line widths 和 dash arrays；
- 相同 clipping；
- 相同 legend bounds。

## 7.2 字体与项目可复现性

如果默认依赖用户系统中的 Helvetica 或 Arial，同一个项目在三台电脑上可能得到不同字宽，从而改变 tick、legend 和 margins。

建议：

- 默认使用 bundled TeX Gyre Heros 四个真实 face；
- 保存 PostScript name、版本、checksum、metrics、coverage 和 embedding state；
- 找不到完全相同的 bundled font bytes 时阻止确定性出版导出；
- CJK 等非 V1 script 明确报错，不能静默 fallback、outline 或栅格化；
- 不在没有提示的情况下重新 layout 后覆盖旧项目结果。

现有 Lite 内置字体可用于 UI 和过渡测试，但 Studio 需要重新确认：

- italic face；
- Greek；
- mathematical symbols；
- embedding 权利；
- subsetting；
- PDF/PostScript naming；
- SVG fallback。

## 7.3 单 axes 自动布局

Studio V1 不需要通用 constraint solver。建议有限迭代：

1. 根据 figure size 和默认 padding 生成 axes proposal；
2. 根据 proposal 生成 locator 输出；
3. formatter 生成 label AST；
4. shape/measure 所有 tick 和 axis labels；
5. 计算四边所需 margin；
6. 更新 axes rectangle；
7. 若 tick 数或文字范围发生变化，再迭代；
8. 达到稳定或最大 3–4 次后停止；
9. 未收敛时选择保守 margins 并产生 warning。

必须用长负数、科学计数、Greek 和语义上下标测试；另用中文标签作为
unsupported-script 负向 fixture，验证它在 shaping/export 前被明确拒绝。

## 7.4 Tick locator 与 formatter

分开设计：

```text
Scale → Locator → Tick values
Tick values + Range context → Formatter → Label AST
```

V1 至少需要：

- linear major locator；
- linear minor locator；
- log10 major/minor locator；
- fixed locator；
- scalar formatter；
- shared scientific exponent；
- precision derived from step；
- negative-zero suppression；
- collision detection and label reduction。

不要让 formatter 直接返回只能由 UI 字体绘制的字符串；它应返回可进入统一 text engine 的 Label AST。

## 7.5 Legend 自动放置

“寻找真正空白区域”不能只比较 legend 与 axes 边界。建议：

1. 先生成若干候选位置；
2. 将 visible series 映射到 coarse occupancy grid；
3. 计算候选 legend rect 与数据、error bar、annotation、极值邻域的重叠代价；
4. 加入边缘距离和阅读顺序代价；
5. 选择最低代价位置；
6. 若所有位置超过阈值，放到 axes 外；
7. 在 Publication Check 中记录自动选择原因。

V1 可先使用确定性 coarse-grid 算法，不需要机器学习。

## 7.6 大数据预览与出版输出

区分三个层次：

- source data：完整数据；
- preview representation：viewport-aware 保峰降采样；
- export representation：完整数据或显式记录的 export simplification。

建议策略：

- line/scatter 默认完整矢量导出到安全上限；
- 超过阈值时提示文件大小和 viewer 性能风险；
- 用户可选择局部 rasterization 或可记录的 simplification；
- rasterization 只作用于密集 artist，axes、labels、legend 和普通曲线仍为矢量；
- 项目保存策略、阈值、DPI 和算法版本。

---

## 8. 候选路线比较

| 路线 | 复用程度 | 出版控制 | 许可证 | 主要风险 | 结论 |
|---|---:|---:|---|---|---|
| 直接扩展 egui_plot | 高 | 低 | 兼容 | 无统一矢量布局 | 不采用 |
| 直接使用 Plotters | 中 | 中低 | MIT | 缺少 Figure Document、字体/PDF/语义 | 仅借鉴或局部复用 |
| 直接依赖 IronLAB | 很高 | 高 | AGPL | 与当前 MIT 发行冲突 | 只作 clean-room 参考 |
| 直接建立在 Plotine 上 | 高 | 待验证 | MIT | 新项目、PDF/字体链路需验证 | 进入对照原型 |
| 自有 IR/Display List + 成熟底层 crates | 中 | 最高 | 可保持 MIT | 初期工程量较大 | 当前推荐主路线 |
| 完整嵌入 Typst | 中 | 高 | Apache-2.0 | 体积、集成复杂度、双事实来源 | 不作为 V1 主路线 |
| Vello 作为核心 IR | 中 | 中 | MIT/Apache | PDF/SVG 仍需另一层 | 未来仅作 preview backend |

---

## 9. 必须完成的技术原型

正式开发 Studio UI 前，必须先完成一个独立、无复杂编辑器的 vertical slice。

## 9.1 原型输入

固定测试数据：

- 一组 Experiment scatter；
- 一条同对象 Fit curve；
- 一条 Theory dashed curve；
- 一组 Y error bars；
- 一条 reference baseline；
- 至少两个 series；
- 包含负值、接近零值和科学计数量级；
- 包含 Latin、Greek、上下标和单位，并另设 CJK unsupported-script 负向 fixture。

推荐标签：

```text
μ₀H_DL (mT)
Current density J_e (A m⁻²)
T ≤ 300 K
```

## 9.2 原型输出

同一 Figure Document 必须生成：

- egui 屏幕预览；
- PDF；
- 300 dpi PNG；
- 600 dpi PNG；
- 1200 dpi PNG；
- TIFF 不要求在 Gate A 前完成，但必须在 V1 release 前完成，并与 PNG 共享 raster buffer。

SVG 是 ADR-021 下的可选实验输出，不属于本节的必需集合。

## 9.3 物理尺寸验收

当前产品默认 figure：85 mm × 65 mm。Part A 的 89 mm × 65 mm 输出继续作为历史兼容性
reference fixture，不再代表 Studio 默认值。

PDF page 目标值：

```text
width  = 85 / 25.4 × 72 = 240.94488 pt
height = 65 / 25.4 × 72 = 184.25197 pt
```

采用 round-to-nearest 像素策略时：

```text
300 dpi  → 1004 × 768 px
600 dpi  → 2008 × 1535 px
1200 dpi → 4016 × 3071 px
```

允许文件格式 metadata 表示造成的极小浮点误差，但不能出现由 UI window 尺寸决定的页面变化。

## 9.4 字体验收

- PDF 中普通文字可搜索、选择和复制；
- PDF 嵌入或 subset 所需字体；
- 不把整页文字转为位图；
- preview/PDF 的 glyph advances 来自同一次 shaping；可选 SVG 不得重新 shaping；
- italic variable、upright unit、upright descriptive subscript 正确；
- Greek 使用真实 Unicode glyph；
- 缺失 glyph 产生可见 warning；
- 三个平台使用同一组 TeX Gyre Heros font bytes 时布局一致；
- CJK 等非 V1 script 产生明确诊断且不调用系统字体。

## 9.5 图元验收

- open/filled circle、square、triangle、diamond；
- marker 视觉面积校正；
- solid、dashed、dotted、dash-dot；
- line cap/join 一致；
- error bar cap size 为物理尺寸；
- clip 后曲线不越出 axes；
- reference line 不覆盖重要数据；
- legend key 忠实反映 marker、fill、line style。

## 9.6 一致性验收

Layout Compiler 输出结构化 snapshot：

- axes rect；
- tick values；
- tick label bounds；
- axis label bounds；
- legend rect；
- path bounds；
- glyph IDs 和 positions；
- clipping regions。

所有 backend 测试只允许格式映射差异，不允许重新生成这些值。

## 9.7 文件结构验收

PDF：

- 页面物理尺寸正确；
- 路径为 vector；
- 字体存在；
- 文字可搜索；
- 没有整页 raster image；
- metadata 记录 producer/version。

可选 SVG（非阻断研究）：

- 可被 usvg 解析；
- `width`/`height` 与 viewBox 一致；
- path、clipPath、text 结构有效；
- 无外部临时文件引用。

Raster：

- 像素尺寸正确；
- 白底与透明背景行为明确；
- 300/600/1200 dpi 只改变像素密度；
- 不改变 axes、text 和 legend 的相对布局。

---

## 10. 原型实施顺序

### Spike A：Display List 和单位系统

实现：

- `Mm`、`Pt`、`Px` 明确类型或严格转换边界；
- `FigureSize`；
- Path、Stroke、Fill、Clip；
- marker geometry；
- 一个固定 axes 和固定 tick 的 figure；
- deterministic Display List snapshot。

通过条件：PDF 和 PNG 显示相同 geometry，物理尺寸计算正确。

### Spike B：文字路线比较

分别验证：

- Parley/fontique/HarfRust；
- cosmic-text；
- V1 Label AST。

记录：

- shaping correctness；
- glyph access；
- measurement；
- fallback；
- italic/upright ranges；
- PDF embedding compatibility；
- binary size；
- build time；
- three-platform behavior。

通过条件：选出唯一默认文字路线，不允许 preview 和 PDF 使用不同 shaper。`latex-rust` 等完整 LaTeX 路线不属于本 spike 或 Gate A。

### Spike C：PDF backend

使用 Krilla 将相同 Display List 写入单页 PDF。

通过条件：

- MediaBox/CropBox 正确；
- path vector；
- glyph run 为真实文字；
- font subset；
- text extraction 正确；
- clipping 和 dash 正确。

### Spike D：Plotine 对照

用 Plotine 生成相同测试图，比较：

- API 可控性；
- physical size；
- text shaping；
- PDF text；
- PDF/PNG parity；可选 SVG 只作研究对照；
- dependency size；
- 是否可以只依赖低层 crate；
- 是否容易注入 InstPlot semantic style。

通过条件不是“Plotine 必须胜出”，而是获得足够证据决定复用、适配或放弃。

### Spike E：Layout

实现单 axes 两阶段/有限迭代布局：

- linear/log tick；
- shared exponent；
- margins；
- rotated Y label；
- legend candidate scoring；
- clipping。

通过条件：测试矩阵中无裁切、无 label overlap，算法 deterministic。

### Spike F：Visual regression

建立：

- Display List JSON snapshot；
- 可选 SVG structural test（非阻断）；
- PDF structural test；
- resvg/Poppler raster golden；
- perceptual diff threshold；
- Windows/macOS/Linux CI artifact。

通过条件：依赖升级造成布局或输出变化时 CI 能明确发现。

### Spike G：UI shell 与轻量化

只实现 window、真实 Display List canvas、side inspector、file dialog 和 keyboard input，不实现正式 property editor。验证：

- 1×/2× transform 与 Windows scaling；macOS Retina、Linux HiDPI 在无设备时
  可由产品负责人明确延后到发布门槛；
- headless export 不依赖 window、screen scale 或 UI 生命周期；
- stripped release executable/app core 加 mandatory runtime assets 的体积；
- cold startup 和 idle memory；
- GUI framework feature pruning 后的最小依赖集合。

通过条件：测量边界可复现，架构目标为 `≤10 MiB`，`≤12 MiB` 为 soft ceiling；`>15 MiB` 且无可行减重路线时不得通过 Gate A。

---

## 11. 测试策略

测试分四层，不能只依赖截图：

### 11.1 Semantic tests

验证 Figure Document resolve：

- Experiment 与 Fit 共享颜色；
- role 决定 marker/line style；
- sequential/diverging 规则；
- palette provenance；
- explicit override 优先级。

### 11.2 Layout tests

对 Display List 和 layout result 做结构化 snapshot：

- ticks；
- bounds；
- glyph positions；
- clip；
- legend placement。

### 11.3 Backend structural tests

- PDF objects、fonts、page box、images；
- SVG nodes、attributes、viewBox、font family；
- raster dimensions、alpha、DPI metadata。

### 11.4 Visual regression

- reference PNG；
- normal-color、grayscale、CVD variants；
- 多 DPI；
- Latin/Greek/Core 使用固定 TeX Gyre Heros font bytes；unsupported script fixture 验证不会调用系统 fallback；
- diff 失败时保留 expected、actual、diff artifacts。

视觉回归只能发现外观变化，不能替代真实文字、字体嵌入和物理尺寸结构测试。

### 11.5 Publication Visual Benchmark Corpus

建立 10–20 个有来源记录的高质量单图曲线参照，覆盖 Nature、PRL、Advanced Materials、Science、JACS 等出版风格。对每个参照只记录可量化的视觉关系：

- axes/figure 比例；
- 字体层级；
- marker/line 比例；
- tick 密度；
- margin；
- legend padding；
- 色彩饱和度与灰度表现。

Corpus 用于校准产品默认值和专家视觉评审，不复制论文图的数据、标识或像素。来源、访问日期、允许保存的 metadata 与版权边界必须随 manifest 记录。

---

## 12. 许可证与供应链要求

每个候选 crate 在进入生产依赖前记录：

- crate name/version；
- source repository；
- SPDX license；
- transitive license；
- bundled assets license；
- MSRV；
- maintenance activity；
- security/advisory status；
- whether native system libraries are required；
- whether network/runtime executables are required。

当前明确结论：

- IronLAB：AGPL，默认只作架构参考；
- Plotine：MIT，可进入原型；
- Plotters：MIT，可借鉴或局部复用；
- rsplot：MIT/Apache-2.0，可作交互参考；
- Vello/Parley/fontique：MIT/Apache-2.0；
- HarfRust：MIT；
- Krilla：MIT/Apache-2.0；
- resvg/usvg：MIT/Apache-2.0；
- tiny-skia：BSD-3-Clause；
- palette：MIT/Apache-2.0；
- latex-rust：MIT/Apache-2.0。

正式采用前仍须通过自动化 license audit 和人工复核，不能仅依赖本文记录。

---

## 13. 尚未决定的问题

以下问题必须通过 spike 后决定：

1. Parley 还是 cosmic-text 作为默认 plain-text engine；
2. V1 后的 extended math 是否需要可选 `latex-rust` adapter；V1 固定只提供 Label AST；
3. SVG backend 直接手写 serializer，还是建立在现有 SVG crate 上；
4. raster backend 直接消费 Display List，还是先经过 SVG；
5. 是否复用 Plotine 的 scale/tick/mathtext/backend；
6. TeX Gyre Heros 的具体版本、许可证、四个 face checksum 和 Core coverage；
7. project container 使用 ZIP+JSON、单 JSON 还是其他格式；
8. dense artist 的 vector/raster 阈值；
9. V1 TIFF encoder、compression 和 metadata 方案；Gate A 不要求 TIFF；
10. unsupported-script 诊断和旧项目兼容策略；
11. GUI framework 在体积、启动、内存与三平台行为上的最终选择。

这些问题不应通过偏好决定，必须由原型数据决定。

---

## 14. 开发启动门槛

满足以下条件后，才开始正式 Studio 编辑器和完整 UI：

1. Figure Document 最小 schema 已冻结；
2. point-based Display List 已实现；
3. 默认文字 shaping 路线已选定；
4. 89 mm × 65 mm 的 PDF page 精确输出；
5. PDF 字体嵌入和 text extraction 通过；
6. preview、PDF、PNG 使用同一 layout result；可选 SVG 若启用也只能消费该结果；
7. open marker、dash、error bar、clip 通过；
8. 300/600/1200 dpi 像素尺寸通过；
9. Display List snapshot 和 visual regression 已建立；
10. Plotine 复用决策已有书面 ADR；
11. 所有生产候选依赖已完成许可证检查；
12. 原型在 Windows、macOS、Linux 至少各验证一次；
13. TeX Gyre Heros 四个 face 路线通过，Core coverage 完整且 unsupported script 不 fallback；
14. Part A color contract 已固定，并通过基础 grayscale/CVD fixture；
15. Publication Visual Benchmark Corpus 已建立并完成第一轮默认参数校准；
16. UI shell 的 HiDPI、file dialog、keyboard 和 headless export independence 已通过；
17. release size、cold startup 和 idle memory baseline 已归档；
18. app core 目标为 `≤10 MiB`，`≤12 MiB` 为 soft ceiling，`>15 MiB` 已触发且通过架构复审。

如果这些条件未满足，继续扩展 UI 只会把尚未解决的渲染问题埋入更多界面代码。

---

## 15. 当前建议的最终技术方向

在完成 spike 前，采用以下工作假设：

```text
Application shell        eframe / egui
Shared data              extracted from InstPlot Lite
Figure model             InstPlot-owned versioned Rust IR
Semantic resolution      InstPlot-owned rules engine
Geometry                 kurbo
Text shaping             Parley + fontique + HarfRust + skrifa
Basic scientific labels  InstPlot Label AST
Advanced math            future optional extension, outside V1
Layout                   InstPlot-owned deterministic compiler
Display list             InstPlot-owned, coordinates in pt
Preview                   egui mesh backend; Vello optional later
PDF                       Krilla
SVG                       InstPlot display-list backend
Raster                    resvg/tiny-skia or direct CPU backend
Color calculations       palette
Palette data             version-pinned InstPlot registry
```

这条路线不是“所有东西自己写”。InstPlot 只掌握与产品价值直接相关的部分：

- Figure Document；
- 科学语义；
- style resolution；
- publication layout；
- Display List contract；
- Publication Check。

字体 shaping、OpenType 解析、PDF 写入、SVG 解析、CPU rasterization、颜色空间转换应尽量使用成熟 Rust 组件。

最终原则：

> 借鉴 IronLAB 已验证的 IR → Display List → 多后端架构，保持 clean-room；用 Plotine 做可复用性对照；优先采用 Krilla 和 Linebender/Harfbuzz 生态解决底层问题；在最小出版图原型通过前，不进入完整 Studio UI 开发。
