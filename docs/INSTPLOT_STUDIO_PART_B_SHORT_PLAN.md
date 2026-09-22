# InstPlot Studio Part B 短执行计划

> 状态：ACTIVE（B0 DONE；B1.1 DONE）
> 制定日期：2026-09-22
> 上位计划：[`SCIPLOT_EXECUTION_PLAN.md`](SCIPLOT_EXECUTION_PLAN.md)
> 适用范围：Gate A 关闭后的正式产品开发（B0–B6）

## 1. 目标

把 Part A 已验证的 Figure IR、文字、布局和输出路线迁入正式产品，同时保护
InstPlot Lite 的可发布状态。第一条正式产品垂直切片为：

```text
Lite/shared data
  → Figure Document
  → single axes layout
  → one Display List
  → Studio preview
  → PDF / PNG
  → structural and visual verification
```

本阶段不重新选择字体、shaper、PDF backend、GUI framework 或基础布局算法。

## 2. 必须遵守的顺序

```text
B0 共享核心
 ↓
B1 独立 Studio 应用
 ↓
B2 正式 Figure Document / 项目格式
 ↓
B3 V1 出版绘图能力（从 single axes 开始）
 ↓
B4 颜色、语义与 Publication Check
 ↓
B5 Lite → Studio 交接
 ↓
B6 打包、验收与预览发布
```

axes 是 B3 的第一条功能切片，但不能绕过 B0–B2。B3 图元与 B4 registry
只能在 B2 schema 稳定后有限并行。

## 3. B0：抽取共享核心

### 工作

1. 对 Lite 做只读依赖和模块清单，形成精确迁移表。**DONE**，见
   [`../reports/B0_SHARED_CORE_INVENTORY.md`](../reports/B0_SHARED_CORE_INVENTORY.md)。
2. 依次抽取 `instplot-core`（**DONE**，见
   [`../reports/B0_CORE_EXTRACTION.md`](../reports/B0_CORE_EXTRACTION.md)）、`instplot-io`
   （**DONE**，见 [`../reports/B0_IO_EXTRACTION.md`](../reports/B0_IO_EXTRACTION.md)）、`instplot-processing`
   （**DONE**，见
   [`../reports/B0_PROCESSING_EXTRACTION.md`](../reports/B0_PROCESSING_EXTRACTION.md)）、
   `instplot-fitting`（**DONE**）；每次只移动一个边界，不同时改行为。
3. 让 Lite 成为共享 crates 的消费者，再建立 workspace root。**DONE**。
4. 记录每步的测试、数据 round-trip、release size 和 memory 变化。**DONE**。

完整结项证据见
[`../reports/B0_SHARED_CORE_CLOSEOUT.md`](../reports/B0_SHARED_CORE_CLOSEOUT.md)。

### 完成条件

- Lite 全部既有测试继续通过并可独立发布；
- Studio 原型可消费共享 data model；
- Studio 专用字体、渲染和 GUI 依赖不进入 Lite binary；
- 不存在复制后分别维护的数据、处理或拟合实现。

## 4. B1：建立独立 Studio 应用

### 工作

- 创建独立 `instplot-studio` binary、产品身份和最小窗口；
- 读取共享核心支持的数据；
- 创建一个最小 Figure Document；
- 接入可替换的 Display List preview adapter；
- 提供 series tree、基础 inspector、warning panel 和固定图导出入口。

### 完成条件

- Lite 与 Studio 是独立 binary 和发布产物；
- 不安装 Studio 时 Lite 功能不受影响；
- UI 只编辑 Figure Document，不成为唯一数据源；
- headless export 不创建窗口或 GPU context。

## 5. B2：正式 Figure Document 与项目格式

### 工作

- 固定 version、stable IDs、data source、axes/artists、typography、palette、
  provenance、overrides 和 export preferences；
- 用 ADR 选择版本化容器；
- 实现 atomic save、backup/recovery、unknown-field policy 和 migrations；
- 建立 create/save/open、旧 fixture migration 与 source-change 测试。

### 完成条件

- 项目 round-trip 后生成相同 resolved figure；
- source/fit identity、字体与 palette provenance 保持；
- 缺失或变化的外部数据只产生明确提示，不静默替换；
- schema 稳定到足以支撑 B3/B4/B5。

## 6. B3：single axes 第一条正式垂直切片

### 实施顺序

1. 将 A7 的 scale、locator、formatter 和有限迭代布局迁入正式模块；
2. 固定 axes rectangle、spines、ticks、grid、labels 和 clipping；
3. 让 preview 消费 A3/A7 resolved glyph runs 或 outlines，修复 A6 临时
   shell 中纵轴标签未旋转和裁切的问题；
4. 先实现 linear axis、line、scatter、line + marker；
5. 再加入 log axis、error bar、fit/theory/reference/baseline、annotation、legend；
6. Preview、PDF、PNG 只消费同一个 layout result 和 Display List。

### 第一切片完成条件

- 单 axes 的 X/Y label、tick label 和纵轴旋转文字完整无裁切；
- 相同输入得到确定性的 axes rectangle、ticks 和 Display List snapshot；
- Preview、PDF、PNG 的几何、文字定位和裁切一致；
- Windows display scaling 只改变 framebuffer density，不改变导出；
- TeX Gyre Heros、U+03BC 和科学计数法继续满足 Part A 合同；
- 新增依赖通过 license、MSRV、RustSec、体积和原生链接检查。

## 7. B4–B6 收尾顺序

- **B4**：扩展 palette/semantic registry，加入 grayscale、CVD、字体、尺寸、
  裁切和 provenance 的 Publication Check；不重新定义 Part A 已冻结的基础语义。
- **B5**：先支持 Lite 文件导入，再实现有版本的临时交换包和“在 Studio 中打开”；
  Studio 样式不得回写 Lite 源文件。
- **B6**：生成独立安装包，完成 size/startup/memory、PDF/PNG/TIFF、项目迁移、
  失败恢复和平台验收；Retina 与 Linux HiDPI 按发布声明和设备条件补验。

## 8. 每个增量的验证要求

- format、test、Clippy `-D warnings`；
- deterministic semantic/layout snapshot；
- PDF structural/text extraction test；
- PNG visual regression；
- Lite compatibility 与数据 round-trip；
- dependency/license/MSRV/native-link audit；
- stripped release size 和 mandatory-assets 增量；
- 只保存可复现证据，不提交机器本地路径或生成缓存。

长时间构建和跨平台任务启动后自行运行，只在完成时读取结构化报告，不持续轮询。

## 9. 明确不进入当前短计划的范围

- complex multi-panel、3D、animation、dashboard；
- spreadsheet、完整数据清洗 UI、通用拟合平台；
- arbitrary vector editor、plugin system、cloud/multi-user；
- full LaTeX 或扩展 LaTeX-compatible math syntax；
- 以 SVG editor compatibility 作为 V1 阻断条件。

## 10. 当前可执行任务

**B0 与 B1.1 已完成。** 独立 Studio package/binary、产品身份、最小窗口和共享 core
边界的证据见
[`../reports/B1_APPLICATION_BOOTSTRAP.md`](../reports/B1_APPLICATION_BOOTSTRAP.md)。
当前任务是 **B1.2 共享数据导入接线**：接入 `instplot-io`，将一个 Lite 支持的数据文件
导入非 UI 的 `StudioSession`，再由应用壳提供原生打开入口。

本步不得提前实现 B2 的正式项目格式，也不得把 Studio 字体、渲染或 GUI 依赖带回
Lite workspace。
