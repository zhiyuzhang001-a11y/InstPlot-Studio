# B5P P3 数据绑定与 series 管理验收

> 状态：DONE（自动验收完成；GUI 已完成真实启动，视觉复查并入 P8）  
> 日期：2026-09-22  
> 上位计划：`docs/INSTPLOT_STUDIO_PRODUCT_POLISH_SHORT_PLAN.md`

## 完成内容

- 数据面板显示 source/fit 类型、父 source 稳定 ID、总行数、有效行数、列数和列名。
- 普通文件导入后将数值数据嵌入 Figure Document，避免项目保存重开后依赖原始文件位置。
- 可选择 dataset、X/Y 列并创建 line、scatter 或 line + marker。
- series 支持复制、删除、显示/隐藏和顺序调整，全部进入统一 EditHistory。
- Inspector 支持重新选择 dataset 与 X/Y；error bar 额外支持 error column。
- source/fit 关系继续使用精确 ID；新建 fit series 默认与父 source 共用颜色。
- 删除 data source 前显示依赖数量；存在 fit/artist 依赖时必须显式确认级联删除。
- 项目 schema 从 1 升级到 2，新增 artist visibility；schema 0/1 均有确定性 migration。
- 打开旧项目、Undo/Redo 数据源删除、保存重开均会同步重建 UI session，不混合旧数据。

## 正确性约束

- 所有 P3 操作仍执行 clone → command → project validation → formal layout → commit。
- 缺失列、外部不可用数据、无效 error column 或 layout/log-domain 错误不会提交半有效状态。
- display name 只用于界面；绑定、fit parent、artist 和 legend 始终使用 stable ID。
- 删除 source 会确定性清理依赖 fit、绑定 artist、axes 顺序和 legend entries；取消不改变文档。
- visibility 只控制正式布局是否产生对象，保存、重开和导出保持一致。

## 验证结果

- Studio tests：新增 schema migration、普通导入内嵌、series 生命周期、级联删除、round-trip、
  visibility 和 Undo/Redo 覆盖。
- pinned Rust 1.98 Clippy `-D warnings`：通过。
- `scripts/validate_b5p_p0.py`：25/25 通过；脚本新增显式 binary build，避免误用旧可执行文件。
- GUI：当前构建完成真实启动并到达 first canvas；更完整的交互矩阵与视觉复查并入 P8。

## 后续边界

- axes、标签、locator/formatter、spine/tick/grid 和图尺寸编辑属于 P4。
- line/marker/error-bar 的颜色、宽度和形状等属性属于 P5。
