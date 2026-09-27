# InstPlot Studio E2E QA 修复复测记录

日期：2026-09-25
对应报告：[`INSTPLOT_STUDIO_END_TO_END_MANUAL_QA_2026-09-24.md`](INSTPLOT_STUDIO_END_TO_END_MANUAL_QA_2026-09-24.md)
安装构建：`v0.1.0 · 202609250101`

## 结果

### P0：真实 CSV 项目保存后无法重开 — FIXED

根因不是数据长度或文件损坏，而是 `serde_json` 默认浮点解析没有保证所有高精度 `f64` 逐位
往返。项目保存时摘要基于导入后的原始二进制浮点值；重新读取 JSON 后，部分真实测量值产生极小
舍入变化，导致嵌入数据摘要被判不一致。

修复：为固定版本 `serde_json 1.0.145` 启用 `float_roundtrip`，并增加高精度嵌入数据保存重开
回归测试。

复测证据：

- 原先失败的 `/tmp/instplot-studio-e2e-qa/single-real.instplot` 已能通过正式 `--check-project`；
- 在安装应用中通过“文件 → 打开项目”成功打开该旧失败文件；
- 在构建 `202609250101` 中重新导入 `E-dsk_1_5V.csv`，经 UI 保存为
  `/tmp/instplot-studio-e2e-qa/single-real-fixed.instplot`，随后立即通过 UI 重开成功；
- 项目标题、曲线、轴标签、文件卡片及 X/Y 绑定均恢复。

### P2：左栏 Y 控件裁切 — FIXED

X/Y 控件改用更紧凑的 `X · 列名`、`Y · 列名`，并为组合框保留右侧箭头空间。安装应用中真实
CSV 文件卡片的 X/Y 文字和两个下拉箭头均完整可见。

### P2：新标注与图例重叠 — FIXED

新标注默认位置由图形上部改到左下区域，并保持后续标注错位排列。真实项目中新建 `Text` 位于
左下，图例位于左上，首次选择不会再命中图例。

### P2：独立窗口空白过大 — FIXED

- 无 error/warning 的 Publication Check 改为约 `440 × 150` 的紧凑窗口；
- Axis label 编辑窗口默认高度从通用高度缩小到 `310` 点；仍允许用户调整大小。

安装应用中两个窗口均已目检，内容完整且无原报告中的大片空白。

### 中文书名号自动闭合 — 按产品范围处理

Studio V1 不支持中文图形文字，因此不扩充 CJK 字体。Figure text 仅自动闭合出版字体覆盖的
ASCII `() [] {}`；输入《等 CJK 标点不再自动生成另一个缺字字符。

## 自动回归

- `cargo test -p instplot-studio --locked`：lib 88/88；main 65/65，另 1 个按需 PDF 测试忽略；
- `python3 scripts/audit_b5p.py`：19/19，通过；内部 P0 29/29；
- Clippy `-D warnings`、格式检查、导出结构、字体、体积、fixture 完整性全部通过；
- 应用替换在唯一 Spotlight 路径 `$HOME/Applications/InstPlot Studio.app`。

本记录只关闭原报告中已处理的 P0 和上述 P2；原报告中明确列出的 NOT RUN/BLOCKED 全矩阵仍不
自动升级为 PASS。
