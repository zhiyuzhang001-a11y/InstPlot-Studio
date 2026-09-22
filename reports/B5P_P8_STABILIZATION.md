# B5P P8 稳定化与阶段关闭记录

> 状态：AUTOMATED PASS；macOS 最终视觉交互确认待完成  
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
- release binary 为 6,571,392 bytes，低于既定 12 MiB 上限。
- release GUI 本机真实启动并到达 first canvas，耗时 162 ms；检查后立即结束进程，没有持续监控。
- 完成逐项收尾审计：P4–P7 的可见文档修改统一进入 EditHistory，并以一条自动测试证明
  Undo/Redo、保存重开和 resolved Display List 一致；连续拖动与文本输入按一次用户操作合并。
- Publication Check 的透明 palette 风险现在指向第一个实际受影响的 artist，可由 finding
  跳转到对象与 Inspector；P4 自动组合矩阵覆盖 85/89 mm、两类 formatter、负数/大数 tick
  与长 Greek/上下标/单位标签。
- P8 边界矩阵新增直接测试：无 dataset、零行或无 alive row 会给出具体错误；NaN/Inf、空白及
  非数字 fixed tick 输入不会进入文档；X=±1e12、Y=±1e-9 的有限科学计数范围可完成正式
  layout，且所选 precision 不被重写。

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

## 尚未冒充通过的项目

- 当前 macOS 在自动视觉检查时进入系统保护画面“ChatGPT is Using Your Mac”，同时拒绝
  AppleScript 辅助访问；截图只能得到保护层，不能据此判断 Studio 视觉效果。
- 因而 P6/P7 新增的画布选择/缩放与导出弹窗仍需一次解锁后的人工点击确认。此前 P2 已完成
  中文界面本机视觉检查，P3 及本次 release 已完成真实首画布启动，但这不能替代新增交互的
  最终视觉验收。
- Windows 本阶段未重新测试；按既定范围只在最终必要时做最小人工确认。

## 解锁后的最小人工清单

1. 在正常窗口和窄窗口各确认一次对象树、画布、Inspector 与问题面板均可到达且不互相遮挡；
2. 用 Tab/Shift-Tab 检查主要控件焦点，以键盘编辑一个标签，再撤销、重做并保存重开；
3. 点击 line、marker、error bar、annotation、legend，确认画布选中反馈与 Inspector 对象一致；
4. 执行滚轮缩放、拖动画布、Fit 与 100%，确认这些 view 操作不制造 dirty 状态或改变导出；
5. 从一个 finding 跳到对应对象，打开导出摘要，分别导出 PDF、白底 PNG 和透明 PNG。

在上述 macOS 人工确认完成前，B5P 不标记为完全关闭，也不启动 Lite 集成或发布工作。
