# InstPlot Studio Phase 4 文本与标签语义验收记录

日期：2026-09-25
结论：通过；可以进入 Phase 5。

## 1. 模块边界

- `label_input.rs` 已从二进制私有模块迁为库级公共 module，统一负责标签解析、语义节点、规范化格式、符号表、转义、字形验证和字体选择。
- 新增库级 `text_edit.rs`，负责不依赖 `egui` 的括号补全、全角括号规范化、转义与 Unicode 光标位置。
- 二进制 `text_input.rs` 只保留 `egui::TextEdit` 适配和控件级测试，不再实现文本语义。
- 尚无第二个真实 consumer，因此暂不额外创建 `instplot-text` workspace crate。

## 2. 规则与修正

- 新增 `INSTPLOT_STUDIO_LABEL_INPUT_RULES.md`，明确 `$`、变量/描述下标、单位、比较符号、星号转义、括号与粘贴规则。
- 修正规范化输出：变量下标保持在同一数学片段内，但 `≤/≥`、普通数字和单位不再被多余的 `$` 包围。
- `$v_{sk}$` 保持 `v/s/k` 全部为数学斜体；`$v$_{sk}` 与 `\mathrm{sk}` 保持描述正体。

## 3. 统一渲染契约

新增 `tests/text_pipeline_contract.rs`，证明同一 `LabelNode` 语义树同时驱动：

- 编辑器规范化回显；
- 字形覆盖验证；
- 正式布局与屏幕显示列表；
- PDF 导出。

符号选择器的每个条目、四种字体面、比较符号的独立数学字形、硬换行和现有项目标签继续由原有视觉/语义测试覆盖。

## 4. 阶段门槛证据

- `cargo fmt --all -- --check`：通过。
- `cargo test --locked --workspace`：226 个测试通过，0 个失败，1 个显式忽略的视觉检查测试。
- `cargo clippy --locked --all-targets -- -D warnings`：通过。
- 独立文本管线集成测试：2 项通过。
