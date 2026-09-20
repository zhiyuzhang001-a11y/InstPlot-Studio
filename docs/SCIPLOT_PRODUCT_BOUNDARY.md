# InstPlot Lite 与 SciPlot 产品边界及共享架构

> 文档性质：产品边界、能力复用与工程架构补充规范  
> 状态：Draft V1  
> 适用产品：InstPlot Lite、SciPlot  
> 相关规范：`PUBLICATION_PLOTTING_DESIGN_SPEC.md`、`scientific-publication-color-system-spec.md`

## 1. 文档目的

本文件补充现有两份出版绘图规范中尚未定义的内容：

1. InstPlot Lite 与 SciPlot 的产品职责；
2. Lite 已经具备、Studio 应直接复用的能力；
3. Studio 为出版绘图必须新增的能力；
4. 两个软件如何保持独立，同时避免复制代码和重复造轮子；
5. 从当前代码库演进到双产品架构的建议顺序。

本文不重新定义颜色、字体、线宽、物理尺寸等视觉规则。相关视觉规则以两份既有规范为准。

---

## 2. 核心产品决策

### 2.1 必须是两个独立软件

InstPlot Lite 与 SciPlot 必须是两个可以独立安装、独立启动、独立发布的软件，而不是同一个大型程序中的“简易模式”和“专业模式”。

原因如下：

- 两个软件服务于科研流程中的不同阶段；
- Lite 必须保持轻量、快速、低认知负担；
- Studio 必须允许更复杂的图形文档、出版排版和导出能力；
- Studio 的字体、PDF、SVG、色觉模拟等依赖不应增加 Lite 的安装体积和启动成本；
- Lite 用户不应被大量出版设置干扰；
- Studio 的发展不应破坏 Lite 已经稳定的数据检查和清理工作流。

### 2.2 独立产品不等于独立重写

两个软件可以拥有：

- 不同的应用入口；
- 不同的窗口、界面和交互流程；
- 不同的依赖集合；
- 不同的安装包、版本号和更新通道；
- 不同的功能范围。

但下列底层能力必须共享同一份实现：

- 数据文件解析；
- 数据集与数值列模型；
- 数据处理算法；
- 拟合算法与表达式求值；
- 原始数据与拟合结果之间的身份和关联；
- 通用数据导出；
- 可共享的错误类型与数据验证规则。

原则是：

> 两个产品、两个工作流、一套经过验证的数据核心。

### 2.3 Studio 也必须保持轻量

Studio 的出版能力不能以无约束扩张依赖和资源为代价。按 stripped release executable/app core 加 mandatory runtime assets 计算：

- architecture target：`≤10 MiB`；
- soft ceiling：`≤12 MiB`；
- `>15 MiB`：必须架构复审。

这是一项架构约束，不是发布末期再处理的优化任务。每个 GUI、字体、PDF、SVG、raster 和 math 依赖都必须记录体积增量及替代方案。

---

## 3. 两个产品在科研流程中的位置

### 3.1 InstPlot Lite

InstPlot Lite 负责实验数据到“可信可用数据”的阶段：

```text
仪器文件
  ↓
快速导入和查看
  ↓
选择 X/Y、发现异常、删除坏点
  ↓
常用处理和拟合
  ↓
导出清理后的数据或快速预览图
```

Lite 的核心评价标准是：

- 打开快；
- 操作简单；
- 数据不被静默改变；
- 清理和拟合结果可追溯；
- 不依赖 Python、Excel 或命令行；
- 可在实验现场或数据初检阶段立即使用。

Lite 不承担精确论文排版责任，也不需要成为通用图形设计软件。

### 3.2 SciPlot

SciPlot 负责“可信可用数据”到“可投稿图形”的阶段：

```text
来自 Lite 或其他来源的数据
  ↓
建立 Figure Document
  ↓
声明数据关系与科学角色
  ↓
自动生成出版级视觉编码
  ↓
精确排版和最终尺寸预览
  ↓
Publication Check
  ↓
PDF / SVG / PNG / TIFF
```

Studio 的核心评价标准是：

- 默认输出即具有出版质量；
- 图形语义明确、一致、可追溯；
- 屏幕预览与最终导出使用同一布局模型；
- 物理尺寸、字号、线宽和字体可靠；
- 矢量输出可编辑、可搜索、可复现；
- 用户主要描述科学含义，而不是手工设计每个元素。

---

## 4. InstPlot Lite 已有能力清单

以下能力已经存在于当前 Rust 版 Lite 中。Studio 不应另写一套近似实现。

### 4.1 数据导入

当前实现已支持：

- TXT、CSV、DAT、TSV；
- XLSX 与旧版 XLS；
- UTF-8、UTF-16、GBK 等常见编码；
- 分隔符、表头和数值列识别；
- 多工作表导入；
- 一个文本文件中的多个 InstPlot 数据区；
- source 与 fit 数据类型恢复；
- 原始数据与拟合结果的 Parent-ID 关联。

对应当前实现：`src/data.rs`。

Studio 应复用相同解析器和数据验证规则，确保同一个输入文件在两个软件中得到相同的数据集、列名、行数和数值。

### 4.2 数据模型与身份

Lite 已经具有：

- 连续 `f64` 数值列；
- `DataSet`、`NumericColumn`、`DataSetKind`；
- 稳定的 `plot_id`；
- source/fit 类型；
- 拟合结果到源数据集、X 列和 Y 列的明确链接；
- 使用 `alive` 位图保留删除状态而不复制整表。

Studio 应在共享数据模型上增加图形引用，不应把相同数据复制成另一套仅供绘图使用的表结构。

### 4.3 数据编辑与历史

Lite 已经具有：

- 单点删除；
- 框选删除；
- 撤销与重做；
- 保留视口范围；
- 受命令数量和内存上限约束的历史记录。

对应当前实现：`src/edit_history.rs` 以及 `src/app.rs` 中的数据交互。

Studio 第一版不必重新实现完整数据清洗界面。需要修改数据时，应优先引导用户回到 Lite，或只提供极少量非破坏性选择能力。

### 4.4 数据处理

Lite 已经具有：

- 中心化；
- 中心化后归一化；
- 多项式背景扣除；
- 局部展平；
- Savitzky–Golay 去噪；
- 按行公式计算；
- 单曲线和多曲线批处理；
- 覆盖源列或保留派生列；
- 原子化撤销和重做。

对应当前实现：`src/processing.rs`、`src/edit_history.rs`、`src/app.rs`。

Studio 不应复制这些算法。若 Studio 需要调用处理能力，应直接依赖共享处理 crate，并明确标记数据已经发生变换。

### 4.5 拟合

Lite 已经具有：

- 1–10 阶多项式拟合；
- 指数、对数和幂函数拟合；
- 自定义表达式拟合；
- 初始参数表达式；
- X/Y 范围过滤；
- 多曲线拟合；
- 拟合方程、系数和 R²；
- 拟合结果的导出、重新导入和自动叠加。

对应当前实现：`src/fitting.rs`、`src/data_export.rs`、`src/app.rs`。

Studio 的工作是把拟合结果正确表达为出版图形中的 `Fit` 角色，而不是重新开发拟合求解器。

### 4.6 数据导出

Lite 已经具有：

- CSV、XLSX、TSV、TXT、DAT；
- 单数据集与多数据集导出；
- 合并文件和独立文件；
- 导出列选择；
- source/fit 关系保持；
- 防止批量导出意外覆盖文件。

对应当前实现：`src/data_export.rs`。

Studio 可以复用这些能力导出数据，但出版图形导出必须由新的渲染系统负责。

### 4.7 大数据预览

Lite 已经具有基于当前可见范围的保峰降采样。命中检测、删除和数据导出仍使用完整数据。

对应当前实现：`DataSet::plot_points`。

Studio 可复用该算法进行交互预览，但最终导出必须具有明确策略：

- 默认使用完整数据；或
- 在数据量极大时使用可记录、可重复、对用户透明的导出降采样策略。

不得把屏幕预览的临时降采样结果静默当作最终出版数据。

---

## 5. Studio 必须新增的能力

### 5.1 Figure Document

Studio 首先需要一个与 UI 和渲染后端无关的图形文档模型。最低应包含：

```text
FigureDocument
├── document version
├── physical page size
├── data sources
├── axes
├── series
├── annotations
├── legend
├── semantic registry
├── palette metadata
├── typography settings
├── export settings
└── publication-check results
```

图形文档保存的是科学语义和排版意图，不是屏幕截图，也不是一串只能由某个 GUI 恢复的绘制命令。

### 5.2 科学角色与关系模型

Studio 必须显式支持：

- Experiment / Measurement；
- Fit；
- Theory / Simulation；
- Reference / Guide to the eye；
- Highlight；
- Background / Context。

还必须描述数据关系：

- unordered category；
- ordered / sequential；
- diverging around a meaningful center；
- shared object identity；
- paper-wide semantic identity。

这些字段驱动颜色、marker、line style 和 legend，而不是仅作为备注保存。

### 5.3 出版样式引擎

Studio 必须把既有两份规范转化为可执行的 style token 和规则系统，包括：

- 89 mm × 65 mm 默认尺寸；
- pt、mm、inch 与像素转换；
- 全局 Text scale；
- axis、tick、curve、marker、error bar 的默认尺寸；
- open/filled marker；
- solid、dashed、dotted、dash-dot；
- Experiment/Fit/Theory 的默认视觉语义；
- 同一对象跨图颜色与 marker 一致；
- palette 来源和版本锁定；
- 禁止无提示使用 Rainbow/Jet。

样式系统必须是确定性的：相同数据、相同文档和相同版本应得到相同输出。

### 5.4 物理单位布局引擎

Studio 必须新增独立布局引擎，负责：

- 以物理单位创建画布；
- axis 区域、标签、tick、指数因子和 legend 的边界计算；
- 防止文字、marker、error bar 和 annotation 被裁切；
- 最终尺寸下的文字测量；
- legend 空白区域评估及 axes 外放置；
- 屏幕缩放与实际排版解耦。

布局结果必须由预览和所有导出后端共同使用。

### 5.5 独立渲染场景

Studio 需要一个 retained-mode 场景层，例如：

```text
Layout result
  ↓
Scene
├── paths
├── strokes
├── fills
├── marker geometry
├── clipped data regions
├── text runs
└── embedded raster images
```

同一个 Scene 应能够发送到：

- 屏幕预览后端；
- PDF 后端；
- SVG 后端；
- PNG/TIFF 光栅后端。

### 5.6 字体与数学排版

Studio 必须新增：

- 字体发现、回退与授权信息；
- deterministic Latin/Greek publication font，可根据体积与许可证决定是否 bundled；
- CJK system fallback，并记录实际字体、metrics 和可嵌入状态；
- PDF 字体嵌入或子集化；
- SVG 字体策略；
- 变量 italic、单位 upright、描述性下标 upright；
- Greek glyph；
- 真正的上下标布局；
- 数学文字与普通文字混排；
- 缺失 glyph 的可见警告和明确回退。

Lite 的界面字体加载代码可以提供字体资产和回退经验，但不能直接替代出版文字排版系统。

### 5.7 出版图元

Studio V1 至少新增：

- line；
- scatter；
- line + marker；
- X/Y error bar；
- fit/theory/reference line；
- baseline；
- direct label；
- legend key；
- annotation text。

所有 marker 应由矢量 geometry 生成，并校正视觉面积。不能依赖位图图标。

### 5.8 出版级坐标轴

Studio 必须新增或重写面向出版的坐标轴系统：

- linear 与 log scale；
- major/minor tick；
- `1, 2, 2.5, 5 × 10ⁿ` 等稳定步长；
- 公共科学计数因子；
- tick label 冲突检测；
- inward tick；
- 四边 spine；
- 明确的 axis range；
- quantity 与 unit 的语义化标签。

Lite 当前基于 `egui_plot` 的坐标轴格式化可作为交互经验和测试案例，但不是 Studio 的最终出版布局实现。

### 5.9 Palette 与可访问性系统

Studio 必须实现颜色规范中的：

- palette registry；
- provenance class；
- source、version、reference、checksum；
- qualitative、sequential、diverging 分类；
- 派生子集记录；
- paper-wide semantic registry；
- 灰度预览；
- 常见 CVD 模拟；
- 颜色之外的冗余编码检查。

Palette metadata 必须随项目保存，旧项目不能因软件升级而静默改变颜色。

### 5.10 出版导出

Studio 必须提供：

- PDF：默认首选，页面尺寸精确，矢量 geometry，真实文字，字体嵌入或明确回退；
- SVG：正确的 viewBox 和物理尺寸，可继续编辑；
- PNG：300、600、1200 dpi；
- TIFF：300、600、1200 dpi；
- 白色与显式透明背景；
- 导出前检查报告。

Lite 当前的 PNG 导出通过截取屏幕绘图区生成，适合快速分享，但不得作为 Studio 导出架构的基础。

### 5.11 Publication Check

Studio 必须能够检查并报告：

- 最终物理尺寸；
- 最小字号和线宽；
- 字体嵌入或回退状态；
- 越界和裁切；
- legend 遮挡；
- 颜色是否成为唯一编码；
- palette 类型是否与数据关系一致；
- 灰度和 CVD 条件下的区分风险；
- raster 像素尺寸和 DPI；
- 透明背景风险；
- palette 来源与版本是否完整。

检查结果应分为 error、warning 和 information，不应把所有建议都变成阻止导出的错误。

### 5.12 Studio 项目文件

Studio 需要版本化项目格式，用于保存：

- Figure Document；
- 数据源引用或嵌入数据；
- 数据集 identity；
- series 到源列的映射；
- palette 和字体 metadata；
- paper-wide semantic registry；
- 导出设置；
- 创建软件版本和格式版本。

项目文件必须具备迁移机制。未知字段应尽可能安全保留，不能因为打开旧项目就丢失信息。

---

## 6. 明确禁止重复实现的内容

Studio 开发中应设置以下工程约束：

1. 不复制 `data.rs` 后改名维护；
2. 不为 Studio 再写一套 CSV/XLSX 解析器；
3. 不重新实现已有处理算法；
4. 不重新实现已有拟合算法；
5. 不另造 source/fit 关联规则；
6. 不让两个软件分别定义不兼容的 dataset ID；
7. 不让两个软件分别维护数据导出格式；
8. 不把 Lite 的屏幕截图导出扩展成伪矢量出版系统；
9. 不把 Studio 的大型出版依赖链接进 Lite；
10. 不通过复制整个 Lite 应用再逐渐修改的方式启动 Studio。

如果某项能力需要同时服务两个产品，应先抽取共享模块，再由两个应用调用。

---

## 7. 推荐的 Rust Workspace 结构

建议从当前单 package 逐步演进为：

```text
InstPlot/
├── Cargo.toml                    # workspace
├── crates/
│   ├── instplot-core/            # 数据模型、身份与通用错误
│   ├── instplot-io/              # 数据导入和数据导出
│   ├── instplot-processing/      # 数据处理
│   ├── instplot-fitting/         # 拟合和表达式
│   ├── instplot-figure/          # Studio Figure Document 与语义规则
│   ├── instplot-layout/          # 物理单位布局和文字测量
│   └── instplot-render/          # 场景及导出后端
└── apps/
    ├── instplot-lite/            # 独立轻量应用
    └── instplot-studio/          # 独立出版绘图应用
```

这只是目标结构，不要求一次性完成大规模移动。应通过小步抽取保持 Lite 始终可构建、可测试、可发布。

### 7.1 依赖方向

推荐依赖方向：

```text
instplot-core
   ↑
instplot-io / instplot-processing / instplot-fitting
   ↑
instplot-figure
   ↑
instplot-layout
   ↑
instplot-render

instplot-lite   → 只选择 Lite 所需模块
instplot-studio → 选择共享模块 + Studio 专属模块
```

底层 crate 不得依赖任何具体应用 UI。共享核心中不应出现 Lite 或 Studio 的窗口状态。

### 7.2 功能开关不是产品边界

Cargo feature 可以用于控制可选后端或平台依赖，但不能把两个产品实现成同一个二进制文件通过 feature 切换界面。两个应用必须具有独立 binary target 和独立发布产物。

---

## 8. Lite 到 Studio 的数据交接

### 8.1 第一阶段

第一阶段直接复用 Lite 已有的分区式文本和 XLSX 格式：

- Lite 导出 source 与 fit；
- Studio 导入相同文件；
- Parent-ID、Source-X、Source-Y 和方程信息保持不变。

这能在不设计新协议的情况下快速建立可靠闭环。

### 8.2 第二阶段

增加“在 SciPlot 中打开”：

1. Lite 将选择的数据集写入临时交换包；
2. 包中包含完整数据、alive 状态、列名、dataset ID、fit links 和必要 metadata；
3. Lite 启动 Studio 并传递交换包路径；
4. Studio 导入后建立新的 Figure Document；
5. 用户保存时写入正式 Studio 项目文件，而不是依赖临时文件。

交换包必须是有版本、可测试的数据协议，不能依赖进程内内存结构或 GUI 状态。

### 8.3 数据所有权

默认规则：

- Lite 负责数据清理和数值变换；
- Studio 默认只引用或嵌入已确定的数据；
- Studio 的样式修改不得回写 Lite 源文件；
- Studio 若允许数值变换，必须记录变换配方和来源；
- 外部源文件变化时，Studio 应提示重新加载，不得静默替换已经用于出版的结果。

---

## 9. UI 与渲染边界

### 9.1 可继续使用 egui 的部分

Studio 可以继续使用 Rust 和 egui 构建：

- 主窗口；
- 数据与 series 面板；
- 属性编辑器；
- palette 选择；
- Publication Check 面板；
- 导出对话框；
- 预览画布容器。

这样可以复用当前跨平台发布经验，并保持整个产品家族的交互一致性。

### 9.2 不应由 egui_plot 决定的部分

下列内容必须由 Studio 自己的 Figure/Layout/Scene 系统决定：

- 最终 axes 几何；
- tick 的位置和文本；
- 物理线宽；
- marker geometry；
- legend 布局；
- 字体与数学排版；
- clipping；
- PDF/SVG/PNG/TIFF 输出。

egui 或其他 UI 工具只能显示 Scene 的预览，不应成为出版文件的事实来源。

---

## 10. 分阶段实施建议

### Phase 0：保护现有 Lite

- 为当前 Lite 建立稳定基线；
- 保持现有测试全部通过；
- 冻结分区式数据格式的兼容行为；
- 不在这一阶段改变 Lite 用户界面。

### Phase 1：抽取共享核心

- 抽取 data model；
- 抽取 data import/export；
- 抽取 processing；
- 抽取 fitting；
- Lite 改为依赖共享 crate；
- 验证行为、安装包和性能没有明显回退。

完成标准：Lite 的用户体验不变，但共享核心已经可被第二个 binary 使用。

### Phase 2：Studio 技术骨架

- 创建独立 `instplot-studio` binary；
- 打开 Lite 支持的数据文件；
- 建立最小 Figure Document；
- 实现单 axes、line 和 scatter；
- 建立物理单位布局；
- 使用同一 Scene 完成屏幕与一个矢量格式输出。

完成标准：同一文档可稳定生成预览和尺寸正确的矢量文件。

### Phase 3：出版语义

- Experiment/Fit/Theory/Reference；
- palette registry 与 provenance；
- marker 与 line-style 规则；
- error bar；
- legend 自动布局；
- 数学文字；
- paper-wide semantic registry。

### Phase 4：出版检查与完整导出

- PDF、SVG、PNG、TIFF；
- 字体嵌入；
- 灰度与 CVD 预览；
- Publication Check；
- 300/600/1200 dpi；
- 裁切、尺寸和一致性回归测试。

### Phase 5：产品级联动

- “在 SciPlot 中打开”；
- Studio 项目文件；
- 独立安装包和更新通道；
- 跨版本项目迁移；
- Windows、macOS、Linux 发布验证。

---

## 11. Studio V1 明确不做

为防止 Studio 变成另一个庞大的通用分析平台，V1 明确不做：

- 重复 Lite 的完整数据清理界面；
- 电子表格编辑器；
- 新的通用拟合平台；
- 3D 图；
- 动画；
- dashboard；
- 任意矢量设计工具；
- 大量装饰效果；
- 按期刊复制整套视觉主题；
- 插件系统；
- 云端协作；
- 多人实时编辑；
- full LaTeX 或扩展 LaTeX-compatible math syntax。

这些能力只有在核心出版工作流稳定后才能重新评估。

---

## 12. 跨产品验收标准

双产品架构至少应通过以下验收：

1. Lite 与 Studio 为两个独立可执行文件和独立安装包；
2. 不安装 Studio 也能完整使用 Lite；
3. Studio 的出版依赖不会增加 Lite 的发布体积；
4. 同一数据文件在两个软件中解析结果一致；
5. Lite 导出的 source/fit 关系可被 Studio 完整恢复；
6. 处理和拟合算法只有一份生产实现；
7. Studio 样式修改不会改变 Lite 的源数据；
8. Studio 预览与 PDF/SVG 使用同一布局结果；
9. 屏幕降采样不会静默改变最终导出数据；
10. 两个产品可以采用不同发布节奏而不破坏共享格式兼容性；
11. 共享核心升级后，Lite 原有数据导入、处理、拟合和导出测试继续通过；
12. Studio 在默认设置下满足两份出版规范的最低验收要求。

---

## 13. 最终原则

InstPlot 产品家族不应通过堆叠功能形成一个越来越大的单体应用。

InstPlot Lite 的价值是：

> 快速、可靠地把实验文件变成可信数据。

SciPlot 的价值是：

> 把可信数据变成语义正确、视觉一致、可验证、可投稿的科学图形。

两个软件的界面、依赖和发布保持独立；数据模型、处理、拟合和交换格式保持共享。任何新功能在实现前都应先回答：它属于数据准备，还是属于出版表达；它是否已经在另一个产品或共享核心中存在。
