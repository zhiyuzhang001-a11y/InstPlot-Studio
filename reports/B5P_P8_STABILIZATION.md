# B5P P8 稳定化与阶段关闭记录

> 状态：DONE（自动审计与 macOS 人工视觉交互均通过）
> 更新：2026-09-23
> 上位计划：`docs/INSTPLOT_STUDIO_PRODUCT_POLISH_SHORT_PLAN.md`

## 已完成的稳定化

- 新增 `scripts/audit_b5p.py`，可无人值守运行；每个命令保存独立日志，失败时打印具体检查、
  退出码与日志路径，不以少量 hash 或文件存在性代替完整验证。
- P0 四类真实样例均重新完成 handoff、Figure Document、PDF、PNG、项目校验和
  Publication Check；缺失值负向样例继续明确拒绝。
- 检查 schema 3 的 axis appearance、export preferences 和 legend-entry visibility；四份 PDF
  均含 TeX Gyre Heros Regular/Italic 嵌入，四份 300 dpi PNG 均为 1004×768 px。
- 输入 fixture 前后 hash 不变；tracked files 不含本机用户绝对路径。
- release binary 为 6,571,408 bytes，低于既定 12 MiB 上限。
- release GUI 本机真实启动并到达 first canvas，耗时 162 ms；检查后立即结束进程，没有持续监控。
- 完成逐项收尾审计：P4–P7 的可见文档修改统一进入 EditHistory，并以一条自动测试证明
  Undo/Redo、保存重开和 resolved Display List 一致；连续拖动与文本输入按一次用户操作合并。
- Publication Check 的透明 palette 风险现在指向第一个实际受影响的 artist，可由 finding
  跳转到对象与 Inspector；P4 自动组合矩阵覆盖 85/89 mm、两类 formatter、负数/大数 tick
  与长 Greek/上下标/单位标签。
- P8 边界矩阵新增直接测试：无 dataset、零行或无 alive row 会给出具体错误；NaN/Inf、空白及
  非数字 fixed tick 输入不会进入文档；X=±1e12、Y=±1e-9 的有限科学计数范围可完成正式
  layout，且所选 precision 不被重写。
- 在系统保护画面下通过目标窗口捕获完成 1192×864 静态视觉检查；发现“预览帧缓冲区”误用
  monospace family 后显示为缺字方框，已改为带系统 CJK fallback 的 proportional UI 字体并
  重新捕获确认文字完整。应用在取得证据后立即退出，没有留下长时间运行进程。
- 按该真实截图完成一轮静态界面收敛：轴标签的“应用标签”只在编辑区展开时显示；无运行时
  警告/错误时不再保留空白底栏；Inspector 默认宽度由 260 调整为 300 logical points；缩放
  控件按“− / 百分比 / +”排列。修改后的 release 已真实到达 first canvas；受保护桌面仍不能
  代替最终交互验收。
- 左侧系列/数据区增加独立纵向滚动，避免窄窗口或多数据集时底部操作不可达；出版图形对象树
  首次打开默认展开，使 axes 与现有 artists 无需额外发现步骤即可选择。中心缩放工具栏与
  图尺寸输入在窄空间自动换行；运行时消息面板在多条警告/错误时可独立滚动。
- 最终人工检查发现实际大小按钮与当前缩放读数同时显示“100%”，含义容易混淆；按钮已改为
  “实际大小 / Actual size”，百分比只用于当前缩放读数。

## 自动审计结果

最终 `target/b5p-audit/summary.json` 为 12/12 PASS：

1. format；
2. workspace tests（all targets）；
3. pinned Rust 1.98.0 Clippy `-D warnings`；
4. release build；
5. P0 完整工作流；
6. P0 25/25 结构化汇总；
7. Figure Document 合同；
8. PDF/PNG 结构；
9. release size；
10. P0–P7 阶段证据；
11. fixture 不变性；
12. 无本机路径泄漏。

## macOS 人工视觉与交互结果

2026-09-23 在 release 构建上完成最终人工检查，结果如下：

1. 正常窗口与窄窗口均保留对象树、画布、Inspector 和问题面板；控件可换行、侧栏可滚动，
   没有不可达控件或面板互相覆盖；
2. Tab/Shift-Tab 可在轴范围字段前进和返回；键盘把 X 最小值从 -3 改为 -4 后 dirty 状态出现，
   Undo 清除修改，Redo 恢复修改；保存为 `p8-manual.instplot` 后标题清除星号，重开仍为 -4；
3. 从对象树选择 line、error bar、scatter、annotation 和 legend 时，Inspector 与橙色画布反馈一致；
   直接点击 annotation 也会反向同步对象树与 Inspector；
4. 100%、放大、Fit 和滚动画布均按纯 view state 工作，不使项目变脏；
5. `legend_overlap · node-15` finding 可跳到 legend，左侧对象树、Inspector 和画布高亮同步；
6. 导出摘要正确显示格式、85×65 mm、300 dpi、1004×768 px 和 Publication Check 计数；
   实际导出 PDF、白底 PNG 与透明 PNG 成功。两份 PNG 均为 1004×768；白底 alpha 全为 255，
   透明版 alpha 范围为 0–255。PDF 为单页 85×65 mm（240.945×184.252 pt）。

## 延期但不冒充已测的范围

- Windows 本阶段没有重跑新的 B5P 人工界面矩阵，因此不把 B5P 的 Windows 视觉结果写成通过；
  跨平台结构和输出仍由统一审计覆盖，后续安装包/发布前在真实 Windows 环境做最小人工复核。
- 按用户当前范围，Lite producer/launcher 集成与正式发布继续暂停；它们不是本轮 Studio 界面
  完整化的阻断项。

B5P 的 Studio 单 axes V1 范围至此关闭；后续仍先完善产品细节，不自动启动 Lite 集成或发布。
