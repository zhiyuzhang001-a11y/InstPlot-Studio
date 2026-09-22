# B5P P6 画布交互与选择反馈验收

> 状态：DONE（自动验收完成；GUI 手感与窄窗口复查并入 P8）  
> 日期：2026-09-22  
> 上位计划：`docs/INSTPLOT_STUDIO_PRODUCT_POLISH_SHORT_PLAN.md`

## 完成内容

- 画布点击直接调用正式 layout `HitMap::hit_test`，支持 axes、X/Y axis、series/data point、
  error bar、annotation 和 legend 的稳定 node identity。
- 画布选择与左侧对象树双向同步；选择绑定对象时继续同步数据集与 X/Y/error column。
- 选中对象使用单独的橙色 overlay bounds；overlay 在 resolved Display List 之后绘制，绝不
  进入 PDF、PNG、项目文件或 Publication Check。
- 画布工具栏提供适合窗口、100%、放大、缩小和明确的当前百分比。
- 鼠标/触控板滚轮以指针下的 document point 为缩放锚点；缩放限制为 10%–800%。
- 双向 ScrollArea 在小窗口或大倍率下提供滚动条；拖动画布可平移，滚轮保留给缩放。
- 缩放与滚动只保存在运行时 view state，不触发 EditHistory、dirty 或保存。

## 正确性约束

- screen → document 坐标只使用正式 figure origin 与 canvas zoom；高 DPI 仍由 preview adapter
  单独映射 framebuffer，不重复乘 pixels-per-point。
- overlay 使用 hit-map bounds 和 project ID 反向映射，不从显示名称猜测对象。
- 100% 表示 1 layout point 对 1 UI point；出版毫米尺寸和 export geometry 不随 view state 改变。

## 验证结果

- P6 测试从正式 legend hit item 的屏幕坐标反查同一稳定 project ID。
- 指针中心 zoom 的 scroll-offset 变换有独立单元测试。
- workspace tests、Rust 1.98.0 Clippy `-D warnings`、格式与 diff 检查通过。

## 后续边界

- finding 分组/跳转、导出设置摘要、DPI 与背景持久化属于 P7。
- macOS 真实点击手感、高 DPI 与窄窗口组合在 P8 集中人工验收；Windows 仍只要求最小必要
  人工检查，不启动持续监控。
