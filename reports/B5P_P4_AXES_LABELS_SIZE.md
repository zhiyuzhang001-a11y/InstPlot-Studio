# B5P P4 axes、标签、刻度与图尺寸验收

> 状态：DONE（自动验收完成；完整 GUI 组合矩阵并入 P8）  
> 日期：2026-09-22  
> 上位计划：`docs/INSTPLOT_STUDIO_PRODUCT_POLISH_SHORT_PLAN.md`

## 完成内容

- Inspector 可编辑 X/Y autoscale、linear/log scale、auto/fixed locator、目标刻度数以及
  decimal/scientific formatter precision；固定刻度必须有限、严格递增且位于轴范围内。
- 手动修改轴范围会显式关闭 autoscale，普通重绘不会再覆盖手动范围；失败的 log autoscale
  在候选文档上回滚，不会留下半有效状态。
- axis label 使用语义片段编辑器，覆盖 plain/upright/variable/Greek、描述性或变量下标、
  上标、单位、分隔符、运算符、强调和粗体变量；Greek 输入验证为单个 Greek Unicode 字符。
- 四边 spine、major/minor tick、tick direction、tick-label side、major/minor grid 全部成为
  Figure Document 设置；默认四边 spine/tick、内向刻度、bottom/left tick label。
- tick 与 grid 的开关保持独立：关闭 minor tick mark 不删除 minor locator 结果，因此仍可单独
  显示 minor grid。
- 图尺寸以毫米保存，支持任意 20–500 mm 范围及 85×65、89×65 mm 快捷预设。
- 布局从画布外缘向内使用 6/4/4 pt 约束；长标签向内压缩 axes rectangle；可用绘图区不足
  时产生显式 `InsufficientPlotArea` warning，不静默裁切或缩小字体。
- schema 从 2 升至 3，旧 schema 0/1/2 确定性迁移到新的 axis appearance/autoscale 默认值。

## 正确性约束

- 所有轴、标签和尺寸操作仍经 EditCommand 的 clone → validate → formal layout → commit；
  UI draft 不是出版事实来源。
- axis identity 与 semantic label identity 不能被设置操作替换。
- preview、PDF 和 PNG 继续使用同一 resolved Display List；view state 不进入尺寸或布局事实。
- tick label 与 spine、axis label 与画布外缘分别由独立设置控制，X/Y decoration 不共用
  可变状态。

## 验证结果

- layout snapshot 覆盖四边 spine、内/外/双向 tick、label side 和 decoration bounds。
- Studio tests 覆盖 autoscale 原子失败、89×65 mm、fixed/scientific tick、语义 Greek/下标/单位、
  schema 2 migration、保存重开和 resolved display list 一致性。
- `scripts/validate_b5p_p0.py`：25/25 通过，所有 P0 数据与输入 hash 保持不变。
- workspace tests、Rust 1.98.0 Clippy `-D warnings` 和 `cargo fmt --all --check`：通过。

## 后续边界

- line、marker、error bar、reference、annotation 和 legend 的属性编辑属于 P5。
- 真实 GUI 的 85/89 mm、长标签、科学计数和窗口尺寸组合检查集中在 P8，避免每个阶段重复
  启动长时间人工流程。
