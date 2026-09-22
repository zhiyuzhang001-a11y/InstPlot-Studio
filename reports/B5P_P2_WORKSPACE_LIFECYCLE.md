# B5P P2 项目生命周期与工作区骨架验收

> 状态：DONE  
> 日期：2026-09-22  
> 上位计划：`docs/INSTPLOT_STUDIO_PRODUCT_POLISH_SHORT_PLAN.md`

## 完成内容

- 将顶部平铺按钮整理为正式的“文件 / 编辑 / 视图 / 导出”菜单。
- 建立集中式中英文 UI 文案层，中文为默认语言，可在“视图 → 界面语言”切换英文。
- UI 中文使用操作系统字体作为运行时回退；字体不打包进程序，也不进入出版图形或 PDF。
- 明确区分“导入数据”“打开项目”和“从 InstPlot Lite 打开”的生命周期语义。
- 标题栏和菜单栏显示当前项目名以及未保存标记。
- 新建、打开项目、打开 Lite 交换包和退出均接入未保存保护。
- 保存、另存为、备份恢复分别具有明确状态；备份恢复后不保留原保存路径，必须另存为。
- 打开项目时重建数据 session 并清除旧 series 选择：
  - embedded 数据精确恢复；
  - external 数据仅在 fingerprint 未变化时恢复；
  - 缺失或变化的数据不与上一个工作区混合，而是形成可恢复 warning。
- 成功状态在 8 秒后自动消失；warning/error 具有稳定 code，同类消息更新而不重复累积。

## 快捷键

- `Command/Ctrl+N`：新建项目
- `Command/Ctrl+O`：打开项目
- `Command/Ctrl+Shift+O`：导入数据
- `Command/Ctrl+S`：保存
- `Command/Ctrl+Shift+S`：另存为
- `Command/Ctrl+Z`：撤销
- `Command/Ctrl+Shift+Z` 或 `Ctrl+Y`：重做

`egui::Modifiers::COMMAND` 在 macOS 对应 Command，在 Windows 对应 Ctrl，因此不维护两套分叉逻辑。

## 验证结果

- `cargo test -p instplot-studio --all-targets`：通过。
- `cargo clippy --locked --all-targets -- -D warnings`：通过。
- `python3 scripts/validate_b5p_p0.py`：24/24 通过。
- Unicode 与空格路径：使用 `实验 数据/磁化 曲线.instplot` 完成真实保存与重开测试。
- macOS 本机人工检查：中文菜单、面板、标题、Figure Document 预览和状态区域均正常显示；
  系统字体回退生效，未出现方框字。

## 边界

- Publication Check 的规则消息和 Figure Document 内部 series 名称仍来自核心层英文诊断；
  它们不是散落在控件代码中的 UI 文案。后续如需翻译，应按稳定规则 ID 建立诊断翻译表，
  不能在 UI 中匹配英文句子。
- 数据列绑定和 series 管理由 P3 完成，不在 P2 扩展范围内。
