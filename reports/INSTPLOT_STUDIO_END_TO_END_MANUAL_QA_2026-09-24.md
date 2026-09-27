# InstPlot Studio 独立端到端人工验收报告

执行日期：2026-09-24 夜—2026-09-25 凌晨（Asia/Shanghai）
执行对象：`$HOME/Applications/InstPlot Studio.app`
环境：macOS 27.0 (26A428)，屏幕 2560 × 1440；Studio v0.1.0，构建 `202609242333`
依据：`docs/INSTPLOT_STUDIO_END_TO_END_MANUAL_QA_PLAN.md` v1.0
方式：独立 agent 经真实 macOS UI 操作已安装应用；没有修改源码、用户原始文件或测试夹具。所有生成物均在 `/tmp/instplot-studio-e2e-qa/`。

## 总判定

**不通过。** 36 个计划用例：PASS 5、FAIL 5、BLOCKED 4、NOT RUN 22。最主要阻断是 **P0：真实 E-dsk CSV 保存为项目后无法重新打开**；单文件与四文件情形均可复现。`smoke.csv` 项目可重开，说明这不是对话框或所有项目文件普遍失效。自动验证六项均通过，但不能代替或推翻人工复现。

本次并未完成计划规定的完整 N/M/F 矩阵、20 次压力操作及所有符号/格式组合，故除已实际观察的范围外不写人工 PASS。此报告是发现并定位问题的中途验收，而非全部功能验收证明。

## 逐项结果

状态定义：PASS 仅表示该用例全部要求经 UI 看见；FAIL 为观察到要求不满足；BLOCKED 为操作设备/可用测试材料阻碍；NOT RUN 包含只完成部分步骤、无法对完整用例下结论。

| ID | 状态 | 实际观察与缺口 |
|---|---|---|
| A01 | BLOCKED | 安装应用可启动，主窗仅一个，七条彩色示例曲线、中文菜单及底部产品名/版本/构建号可见。CUA 的系统 Spotlight 快捷键无响应，未亲眼验证结果列表与图标；`mdfind` 的唯一结果仅算自动补充。 |
| A02 | NOT RUN | N、M、F 均看见七曲线、四边向内刻度及边界副刻度；画布比例稳定。退出 F 后状态保持未完整验证。 |
| B01 | PASS | 首次导入真实 `E-dsk_1_5V.csv` 后示例消失，真实曲线、列标题、文件名图例及简洁左侧文件卡立刻出现。 |
| B02 | PASS | 继续导入其余三个 E-dsk CSV，四条异色曲线、四个独立卡片及同导入顺序的图例出现；文件标题截断而删除 × 保留。 |
| B03 | NOT RUN | 第一文件 Y 列由 `0.00083` 改 `0.00094` 再恢复，曲线及 Y 标签即时变化，其余三曲线未见串改；但未对第二文件及 X 列完成要求的组合。 |
| B04 | NOT RUN | 数字列名 CSV 显示可选数值标题；缺失值 CSV 出现三点而未整条消失；multi-source TXT 同时显示 Cooling/Warming 两条曲线。未完成断线、`lite-source-fit.txt` 及关系检查。 |
| B05 | BLOCKED | CSV、TXT 已人工成功导入；TSV、DAT、XLSX、XLS 无人工夹具，未以自动测试冒充通过。 |
| B06 | FAIL | 长文件名被截断且侧栏未无限变宽，但 F/default 左栏约 198 屏幕像素宽时 Y 列控件右侧被裁切；拖动侧栏边缘两次未改变宽度。悬停完整路径与 N/M 最窄/最宽未验证。见问题 3。 |
| B07 | NOT RUN | `smoke.csv` 文件删除确认、关联曲线与图例消失、Undo/Redo/Undo 恢复均成功；未测试“清空全部”及四文件中间删除。 |
| C01 | NOT RUN | X 最小值 0.9→1.2 后第一个点消失，自动范围恢复该点；Y min/max 和无效输入未覆盖。 |
| C02 | NOT RUN | 主刻度 Auto/固定间隔入口可见，四边 inward 刻度可见。CUA 在数值输入中发生输入法混入，无法可靠判定固定值；副刻度设置、格式、log10 未完成。 |
| C03 | NOT RUN | N/M 的 X label 独立小窗可见，F 为主窗内可见回退，点击主图不关闭；Y label、长文字和三状态完整编辑未测。N 的 X label 小窗约 470×422，底部大量空白，见问题 4。 |
| C04 | FAIL | ASCII `(` 自动生成 `()` 且光标居中；中文输入法直接输入 `《` 自动生成 `《》`，编辑器随即提示内置字体缺少 `《 (U+300A)`，删除左括号后又提示缺少 `》 (U+300B)`。这类 UI 自己支持配对却无法正常绘制的括号不满足“各种括号”目标。其余 Greek/数学/转义/保存 PDF 组合未逐项完成。见问题 2。 |
| C05 | BLOCKED | CUA 的一次滚动等于整页滚动，导致极大缩放，不等价普通鼠标滚轮一格或触控板手势；只确认缩放会响应，不能据此判定手感、锚点及三模式。 |
| D01 | NOT RUN | axes、X label、legend、annotation 选中时见蓝色反馈；未逐类核对所有曲线/marker/Y label 框贴合。 |
| D02 | NOT RUN | 未人工完成曲线角色、线型及 N/M/F 全组合。 |
| D03 | NOT RUN | `smoke.csv` marker 编辑器列出圆、方、上下三角、菱形、五角形、星形及实/空心预览；颜色为中文名称，不含 `object-`；红色和空心圆选择后立即刷新，保存重开仍保持。未逐一点击验证每种形状。 |
| D04 | NOT RUN | F 下图例/标注编辑器点击主图仍保留；N/M 四种编辑窗同时共存、移动和找回未完成。 |
| E01 | PASS | 双击图内图例进入编辑器，显示可见性、内/上/右位置、精确坐标、行列及数字顺序；该编辑器未见大块无用空白。 |
| E02 | NOT RUN | 控件可见，但未逐项改名、换序、隐藏及切换全部行列组合。 |
| E03 | NOT RUN | F 下四项图例可从图内拖到图右，再连续拖回图内；位置值更新，主图未缩小；未做 N/M、图上及快慢两种速度。 |
| E04 | NOT RUN | 四曲线右侧图例的 PDF 导出尺寸约 112.54×65 mm，85×65 mm 主图未缩小，图例旁白边紧凑；5/7 项、图上、远距离拖动未测。 |
| F01 | FAIL | F 下顶部“添加文字”会新建 `Text`，编辑窗和位置可用；默认位置与图内 legend 重叠，第一次拖拽命中并移动了 legend。先把 legend 拖出后才能正常拖动标注。N/M、长数学文字未完成。见问题 5。 |
| F02 | BLOCKED | CUA 在 egui TextEdit 中的 `super+a` 未选中文字，`paste` 超时；不足以判定产品清空即删除，未把自动化输入问题记作应用失败。 |
| G01 | PASS | 主图尺寸窗口初始 85×65 mm，无 89×65；把宽改为 90 后画布变化，恢复 85 成功。 |
| G02 | FAIL | N 全通过独立窗口只显示标题及“未发现出版规范问题”，约 480×510 窗口其余大块空白；F 嵌入回退可见，2/3 warning 可列出。M/error 未完成。见问题 4。 |
| G03 | NOT RUN | 四真实 CSV、右侧图例、标注经 UI 导出 PDF；文件一页、112.54×65 mm、曲线及文字完整、无选中框，嵌入 TeX Gyre Heros 字体。但导出内容未含 `≤/≥`，无法判定该关键字形一致性。 |
| G04 | NOT RUN | UI 分别导出白底、透明 PNG，二者都为 1329×768 RGBA；尚未逐像素确认透明度与白底确实不同。 |
| G05 | NOT RUN | 未制造明确 publication error 或测试导出阻断。 |
| H01 | FAIL | 真实 E-dsk 单文件及四文件项目均能保存，但重开报 `embedded data source … has inconsistent data`；详情见问题 1。对照 `smoke.csv` 无编辑及红色空心 marker 编辑后均能保存重开。 |
| H02 | NOT RUN | File→New 在脏项目时有保存/放弃/取消提示；取消保留当前图，放弃打开示例。Open/Exit 的两种选项未完整走完。 |
| H03 | PASS | UI 打开原夹具 `project-v0.instplot`，另存临时 `/tmp/instplot-studio-e2e-qa/migrated-v0.instplot`，经 UI 重开成功；原夹具未覆盖。 |
| H04 | NOT RUN | 文件删除 Undo/Redo 和标签编辑撤销仅作局部验证，未按要求串联六类操作。 |
| I | NOT RUN | N/M/F 都进入过；F 做过导入、图例跨区拖拽、标注拖拽、出版检查、导出，N/M 做过若干编辑。但计划 9×3 操作矩阵及小窗开启时 N→M→F→N 循环未完成。 |
| J01 | NOT RUN | 未达到 20 次选中/编辑、10 次真实手势缩放、5 次模式切换、3 次导入/删除循环门槛。 |
| J02 | NOT RUN | 曾正常退出并重启，没有当场崩溃或重复主窗；脏内容退出提示及多次稳定性尚未完整验证。 |

## 可复现问题

### 问题 1 — [P0] 真实 CSV 保存为项目后无法重开

- 用例：H01；窗口状态：F（四文件工作流）、N/F（单文件收窄对照）。
- 步骤：① File→New；② File→Import Data 选择 `~/Library/Mobile Documents/com~apple~CloudDocs/E-dsk_1_5V.csv`，确认曲线出现；③ File→Save 到 `/tmp/instplot-studio-e2e-qa/single-real.instplot`；④ File→Open Project 选择刚保存的文件。
- 预期：项目重开，数据、绑定及曲线与保存前一致。
- 实际：状态栏显示 `打开项目失败: primary project failed (invalid project: embedded data source instplot-… has inconsistent data); backup failed (No such file or directory (os error 2))`；原图仍停留，未重开。四个 E-dsk CSV 的 `/tmp/instplot-studio-e2e-qa/workflow.instplot` 也同样失败。单文件完全未编辑即失败，故不是图例或标注改动引起的。
- 复现率：2/2 个独立真实 E-dsk 项目；对照 `smoke.csv` 项目 2/2 可正常重开。
- 证据：两次打开失败的 CUA 界面截图和状态栏；临时项目 `single-real.instplot`、`workflow.instplot`；成功对照 `smoke-roundtrip.instplot`。附件均在上述临时目录，原始 CSV 未修改。

### 问题 2 — [P2] 自动闭合中文书名号后内置字体报缺字

- 用例：C04；窗口状态：N。
- 步骤：双击 X 轴标签→在文字框中使用中文输入法键入 `《`（直接按 `shift+comma`）→观察自动闭合与红色提示。
- 预期：既然编辑器自动生成 `《》`，应能显示或明确不支持此种配对；不应生成会导致出版缺字的文本。
- 实际：框内出现 `《》`，提示 `内置字体缺少符号 《 (U+300A)`；删除左括号后，对 `》 (U+300B)` 再报缺字。
- 复现率：1/1；其他中文括号和最终 PDF 未覆盖。
- 证据：CUA 的标签小窗截图与红色缺字提示。

### 问题 3 — [P2] 窄侧栏的 Y 列控件被裁切

- 用例：B06；窗口状态：F，默认侧栏宽度约 198 截图像素。
- 步骤：导入四 E-dsk CSV，或导入临时超长文件名 CSV；观察左侧文件卡片内 X/Y 并排控件右端。
- 预期：文件名截断后 X/Y 下拉和删除按钮完整可见、可操作。
- 实际：文件名和 × 正常，但 Y 列下拉右端超过左栏边界、文案被裁掉；暴露部分仍可点击。两次从侧栏边界拖动未见宽度改变，尚不能断定此为可伸缩功能故障。
- 复现率：在两个导入场景均观察到；N/M 极限宽度未覆盖。
- 证据：CUA 的四 CSV 左栏及长文件名左栏截图；临时长名文件。

### 问题 4 — [P2] 独立小窗仍有大块无效空白

- 用例：C03、G02；窗口状态：N/M 的 label 窗口，N 的出版检查窗口。
- 步骤：双击 X label；再打开 Publication Check 的无问题状态。
- 预期：窗口紧凑贴合内容，文字和按钮留有合理间距。
- 实际：label 小窗约 470×422，控制仅占上部约 280；全通过出版检查小窗约 480×510，除标题和一行“未发现出版规范问题”外几乎全空。图例编辑窗相对紧凑。
- 复现率：1/1 各窗口。
- 证据：CUA 小窗截图。

### 问题 5 — [P2] 新建标注默认与图例重叠，首次拖拽命中错误对象

- 用例：F01；窗口状态：F，四 E-dsk CSV、图例在图内。
- 步骤：顶部“添加文字”创建默认 `Text`；在新文字与图例交叠区域按住拖动。
- 预期：新对象可直接选中移动，不需先移动其他对象。
- 实际：第一次拖拽移动的是图例。将图例先拖到图右后，标注可正常拖动且位置实时变化。
- 复现率：1/1，密集示例/其他位置未覆盖。
- 证据：CUA 图内重叠及拖动后截图。

## 导出实物核对

- `/tmp/instplot-studio-e2e-qa/workflow.pdf`：271,685 bytes；`pdfinfo` 证实单页 319.025×184.252 pt（≈112.54×65 mm），可正常光栅化。`workflow-render.png` 目检四曲线、右侧图例、轴标签及标注完整，白边紧凑、无选中框。PDF 含嵌入的 TeX Gyre Heros 子集；此文件没有 `≤/≥`，不能用于判定该字形。
- `/tmp/instplot-studio-e2e-qa/workflow-white.png` 与 `workflow-transparent.png`：经 UI 生成，均 1329×768 RGBA；未进行 alpha 像素级断言，因此 G04 未给 PASS。
- 旧迁移项目 `/tmp/instplot-studio-e2e-qa/migrated-v0.instplot` 经 UI 另存及重开成功。

## 补充自动验证（人工操作后执行）

以下六项全部 exit 0，但仅作为补充，不能把相应人工 NOT RUN 改为 PASS：

1. `cargo fmt --all --check`：PASS。
2. `cargo test --workspace --locked --all-targets`：PASS；Studio 库 87 通过，UI/main 64 通过、1 个手动 PDF 测试 ignored，布局 19、刻度 7 等通过。
3. `cargo clippy --workspace --locked --all-targets --all-features -- -D warnings`：PASS。
4. `python3 scripts/audit_b5p.py`：PASS，19/19 检查，P0 摘要 29 项无失败；其使用的自动夹具未暴露问题 1 的真实 CSV 路径。
5. 已安装 app `codesign --verify --deep --strict --verbose=2`：PASS，valid on disk / satisfies Designated Requirement。
6. `mdfind 'kMDItemCFBundleIdentifier == "com.instplot.studio"'`：仅 `$HOME/Applications/InstPlot Studio.app` 一个结果；仍不能替代 Spotlight UI 图标目检。

## 未覆盖风险与下一步

优先修复问题 1，并添加以四个真实 E-dsk 数据等价结构为依据的“UI 导入→保存→重开”回归（应保护原始文件、不依赖 iCloud 路径）；修复后复跑 H01 和完整导出闭环。随后处理中文自动配对符号与内置字体不一致、窄侧栏控件裁切、弹窗空白、默认标注重叠。最后由可稳定输入键盘/触控板动作的人工执行剩余 N/M/F 矩阵、复杂符号 PDF 对照、全部格式、清空与跨功能撤销及 J01 压力测试。当前不得宣称 Studio 端到端闭环通过。
