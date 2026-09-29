# InstPlot Studio 空副轴、轴显示与导出裁边实施计划

> 配套设计：`INSTPLOT_STUDIO_EMPTY_AXIS_PLACEHOLDER_AND_VISIBILITY_SHORT_PLAN.md`
> 状态：`COMPLETED`
> 原则：预览与导出分流；只有导出使用 tight bounds。

## 1. 不变量

实施过程中以下内容不得改变：

- `FigureRecord.width_mm/height_mm` 继续表示现有总画布尺寸。
- 普通 `resolve_document()` 继续产生原尺寸预览。
- layout、hit map、拖拽、双击编辑和工具草稿继续使用原 scene coordinates。
- 图例、figure-point 标注、轴标签 gap 和数据坐标不迁移、不重基。
- 旧项目不增加兼容模式或转换流程。

## 2. 模块职责

### 2.1 Layout

- 负责原有画布和所有对象的 scene-space 几何。
- 轴 visibility 决定对应 display item 和 hit item 是否生成。
- 不计算 tight export page，不读取导出 translation。

### 2.2 Export resolver

- 普通 `resolve()`：保留 display list 原宽高，translation 为零，用于预览和常规派生状态。
- `resolve_tight(display_list, axes_bounds, 3 pt)`：仅供文件导出。
- 输出 `ink_bounds`、`export_bounds` 和 `export_translation`，但不改写各个 item 的 scene coordinates。
- clip-aware 扫描可见 ink；文本使用真实 glyph outline，路径包含曲线极值和实际 stroke geometry，图像按可见 alpha/resource 处理。

### 2.3 PDF / SVG / PNG

- 三个后端在最外层应用一次相同 translation。
- clip 与内容接受完全相同的 translation。
- SVG/PDF 页面尺寸取 tight width/height；PNG 按 DPI 换算并 `ceil`。
- 不在后端重复计算 bounds。

### 2.4 Studio

- `resolve_document()`：预览专用，调用普通 `resolve()`。
- `resolve_document_for_export()`：导出专用，调用 `resolve_tight()`。
- `AppAction::ExportFigure` 必须重新取得 export resolution，不能直接复用屏幕预览的 resolved display。
- Publication Check 若检查最终栅格尺寸，应显式使用 export resolution；其他检查继续使用普通 layout/preview。

## 3. 执行阶段

### Stage A：收缩此前过度设计

- [x] 恢复 `FigureRecord.width_mm/height_mm`。
- [x] 删除 PlotSize/LegacyCanvas 模型与 layout 分支。
- [x] 删除画布转换 UI、编辑命令和坐标迁移。
- [x] 恢复图例、标注拖拽的既有坐标语义。
- [x] 取消预览对 export translation 的依赖。
- [x] 保留 axis visibility、空副轴占位及 UI-only gutter。

### Stage B：隔离导出路径

- [x] 普通 preview 使用 `resolve()`。
- [x] 新增 `resolve_document_for_export()`。
- [x] PDF、SVG、PNG 文件导出统一走 tight resolution。
- [x] 检查所有间接导出入口，确认没有继续复用 preview resolution。
- [x] 导出事务中的 Publication Check 使用 export resolution；普通编辑检查继续使用 preview resolution。

### Stage C：边界正确性

- [x] axes rectangle 始终作为最小导出边界。
- [x] 隐藏轴装饰不贡献 ink。
- [x] 图外 legend、annotation、connector、measurement arrow、reference line、image 完整计入。
- [x] 嵌套 clip、clip 边缘粗线、负坐标对象、旋转文字和透明内容行为正确。
- [x] 统一 miter limit、cap/join 与三个后端一致。
- [x] 3 pt safety padding 在四边一致且不会累积。

### Stage D：回归测试

- [x] 项目 v0–v10 迁移和保存重开不改变既有画布宽高语义。
- [x] 预览尺寸在隐藏标签前后只按原 layout 规则变化，不受 tight export translation 影响。
- [x] 数据坐标、hit testing、图例/标注/箭头/参考线拖拽保持原行为。
- [x] 单轴、双 X、双 Y及空副轴占位正常。
- [x] PDF/SVG/PNG 对相同文档得到一致物理边界。
- [x] 普通、最大化、全屏 GUI 操作通过。

## 4. 自动测试矩阵

- Export resolver：路径、虚线、miter、marker、误差棒、文字、旋转文字、图像、透明度、嵌套 clip。
- Axis visibility：分别隐藏 spine、ticks、tick labels、label；验证显示、命中和导出 bounds。
- Outside content：上方/右侧图例、四方向标注、双向箭头和参考线。
- Backend parity：PDF MediaBox、SVG viewBox、PNG 像素宽高对应同一 pt bounds。
- Isolation：同一文档的 preview resolved width/height 在导出前后不变，所有项目数据逐字不变。
- Lifecycle：Undo/Redo、保存重开、清除数据、重新导入、单轴/双轴切换。

## 5. 质量门

依次执行：

1. `cargo fmt --all -- --check`
2. `cargo check --locked --workspace --all-targets --all-features`
3. `cargo test --locked --workspace --all-targets --all-features`
4. `cargo clippy --locked --all-targets -- -D warnings`
5. `git diff --check`
6. Release 构建
7. 独立 agent 按本计划逐项验收
8. 本地 Spotlight 应用替换与普通/最大化/全屏实测

任一阶段失败只修复与本计划直接相关的问题；不得再次引入画布语义迁移。

## 6. 完成记录（2026-09-28）

- 完整 workspace 测试、严格 Clippy、格式与差异检查、release 构建全部通过。
- 独立 agent 先发现 tight export 未携带 raster resources；补充 resource-aware tight resolver、可见 alpha 判断和回归测试后复验为 PASS。
- 唯一 Spotlight 应用已更新为 build `202609281528`，签名、arm64、bundle ID 与唯一索引通过；普通、最大化、全屏界面均完成实机检查。
- 本轮未提交或推送 GitHub。
