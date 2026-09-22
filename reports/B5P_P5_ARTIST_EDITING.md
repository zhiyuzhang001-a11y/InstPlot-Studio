# B5P P5 artist、颜色与 legend/annotation 编辑验收

> 状态：DONE（自动验收完成；GUI 视觉复查并入 P8）  
> 日期：2026-09-22  
> 上位计划：`docs/INSTPLOT_STUDIO_PRODUCT_POLISH_SHORT_PLAN.md`

## 完成内容

- line 可编辑语义角色、项目 palette 颜色、真实 pt 线宽与 solid/dashed/dotted/dash-dot。
- marker 可编辑语义角色、palette 颜色、circle/square/triangle/diamond 和 pt 尺寸。
- error bar 可编辑 error column（P3）、cap width、stroke color/width/dash。
- reference/baseline 可编辑水平/垂直方向、数值和完整 stroke。
- annotation 可编辑 figure-point 锚点和由科学语义片段组成的标签。
- legend 可编辑位置、条目显示/隐藏、顺序及每个条目的语义标签；artist 本身的显示状态
  继续由对象树控制。
- Inspector 按实际 artist variant 只显示适用控件，不制造无效属性组合。
- 颜色从 Figure Document 的冻结 palette registry 选择；显式 artist 修改以
  `artist_style` override 写入项目，默认 source/fit 共享颜色关系仍保持。

## 正确性约束

- artist ID 和 kind 不可被属性编辑替换；候选记录必须通过完整 Project validation 和
  formal layout 后才由 EditHistory 提交。
- 新增的 legend-entry visibility 使用向后兼容默认值；旧项目缺少该字段时按可见处理。
- semantic label 仍保存为 `LabelNode`，不是把屏幕字符串当作第二事实来源。
- grayscale/CVD、线宽、透明度和重叠风险仍由现有 Publication Check 以稳定 node ID
  指向具体对象。

## 验证结果

- P5 round-trip 测试一次覆盖 line、scatter、error bar、reference、annotation、legend 六类
  artist，并比较保存重开后的 Project Document 与 resolved Display List。
- 显式修改的六类对象均产生稳定 override 记录；legend 条目隐藏和重排通过正式 layout。
- workspace tests、Rust 1.98.0 Clippy `-D warnings`、格式与 diff 检查通过。

## 后续边界

- 点击画布选择、selection overlay、fit/100%/缩放等纯 view-state 交互属于 P6。
- Publication finding 跳转、导出前摘要和 DPI/background 工作流属于 P7。
