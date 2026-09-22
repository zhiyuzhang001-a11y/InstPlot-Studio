# InstPlot Studio 产品细节完善短计划

> 状态：ACTIVE（P0–P5 DONE；P6 next）
> 阶段代号：B5P（位于 B5 Studio 接收端完成之后、Lite 最终联调与 B6 发布准备之前）  
> 制定日期：2026-09-22  
> 上位计划：[`SCIPLOT_EXECUTION_PLAN.md`](SCIPLOT_EXECUTION_PLAN.md)  
> 范围：单 axes V1 的真实编辑体验、默认效果和可靠性，不包含安装包发布

## 1. 为什么增加 B5P

B0–B4 已建立共享核心、正式 Figure Document、单一 Display List、PDF/PNG 输出和
Publication Check；B5 的 Studio 交换包消费者也已经完成。但当前 Studio 仍是工程
骨架，而不是完整的日常绘图工作流：界面目前主要能导入、保存、导出、选择 series
条目并修改四个轴范围，Figure Document 中已经存在的许多能力还没有可用的编辑入口。

B5P 的目标不是增加宏大功能，而是把已有底层能力组织成连贯、可理解、可撤销、可保存
的产品体验。完成 B5P 后再补 Lite 侧启动集成，最后进入 B6 发布准备。

## 2. 当前基线与明确缺口

已经具备：

- TXT/CSV/DAT/TSV/XLS/XLSX 导入和 source/fit identity；
- versioned Figure Document、stable IDs、原子保存、备份恢复和 schema migration；
- linear/log axes、locator、formatter、line、scatter、error bar、reference line、
  annotation、legend 的正式布局与渲染；
- 同一 resolved Display List 驱动 Preview、PDF 和 PNG；
- 固定字体、科学标签语义、palette registry 和 Publication Check；
- Lite handoff 的 Studio 接收端。

仍缺少或不完整：

- 导入后选择 X/Y、建立/删除/重排 series 的完整流程；
- axis label、scale、locator、formatter、tick、grid、spine 的 UI；
- line、marker、error bar、reference、annotation 和 legend 的属性编辑；
- 画布命中选择、适合窗口、可靠缩放和选择反馈；
- dirty 状态、未保存保护、Undo/Redo 和统一编辑事务；
- 图尺寸、DPI、背景和导出选项的正式工作流；
- 错误提示的定位、恢复建议和可操作性；
- 真实科研数据驱动的端到端人工验收。

## 3. 执行原则

1. 编辑必须先进入 Figure Document，再由同一 resolve/layout 路径刷新预览和导出；
   UI 不得成为第二事实来源。
2. 每次用户操作是一个可验证的事务：候选文档校验成功才提交，失败时保留原状态。
3. 能进入项目文件的设置必须 round-trip；纯视图状态必须明确标识且不得污染出版几何。
4. 保留 stable IDs、source/fit identity、字体和 palette provenance；不得用显示名或文件名
   猜测关联。
5. 不为完善 UI 重写已经通过验证的字体、布局、Display List 或后端路线。
6. 每个小阶段先完成自动回归，再进行一次有目标的人工检查；长任务生成报告后再读取，
   不持续监控。

## 4. 固定的 V1 默认行为

- 单 axes；默认最终图尺寸以 85 mm × 65 mm 为基准，可在允许范围内编辑。
- 四条 spine 默认可见。
- 四条 spine 默认都绘制 major/minor tick mark，刻度方向朝内。
- tick mark 从 spine 起画，不在 tick mark 与 spine 之间引入空隙；tick label 与 spine 的
  默认间距为 4.0 pt。
- X/Y axis label 的实际墨迹外缘分别固定距画布底边/左边 6.0 pt，axis label 与最近的
  tick label 边界间距为 4.0 pt；文字增大时只向内调整 axes rectangle。
- 默认只在 bottom/left 显示 tick label，避免重复标签；top/right 标签以后可显式开启。
- 自动范围应留出稳定的小边距；用户手动范围不得被普通重绘静默覆盖。
- source 数据默认用 marker，fit/theory 默认用 line；同一 source/fit 关系优先共享颜色。
- 默认颜色来自已冻结的 publication palette；不能只靠颜色区分时使用 marker/dash 冗余。
- TeX Gyre Heros、语义变量斜体、描述性下标 upright、U+03BC 规则保持不变。
- 画布缩放和面板宽度属于 view state，不改变 PDF/PNG 的物理尺寸与布局。

间距取值依据：Matplotlib 当前默认 `xtick/ytick.major.pad = 3.5 pt`、
`axes.labelpad = 4.0 pt`；PGFPlots 的 `near ticklabel` 方案根据实际 tick-label 尺寸动态
放置 axis label；Nature 的官方要求强调轴线、刻度、单位、可读性和无重叠，并未规定
统一的固定间距。P4 的实际样张比较最终采用 6/4/4 pt 外向内约束，而不把
某一期刊不存在的间距要求写成硬性标准。参考：
[Matplotlib defaults](https://github.com/matplotlib/matplotlib/blob/main/lib/matplotlib/mpl-data/matplotlibrc)、
[PGFPlots axis descriptions](https://tikz.dev/pgfplots/reference-axisdescription)、
[Nature figure specifications](https://research-figure-guide.nature.com/figures/preparing-figures-our-specifications/)。

## 5. 分阶段实施

### P0：真实工作流基线与验收样例

工作：

- 固定至少三类真实样例：单 source、source + fit、多 source；至少一份含缺失/禁用行；
- 记录从打开数据到导出图形的当前点击路径、阻断点和默认效果；
- 为每类样例保存项目、PDF 和 PNG 基线，不把临时绝对路径提交到仓库；
- 建立按“阻断使用 / 明显不便 / 外观细节”划分的问题清单。

完成条件：

- 每个后续阶段都有同一组输入和可重复比较的输出；
- 当前缺口有代码位置、用户影响和验收方法，而不是只有截图描述。

### P1：编辑事务、dirty 状态与 schema 演进基础

工作：

- 为 Figure Document 增加类型安全的编辑命令，不允许 UI 直接随意改内部结构；
- 每个命令执行 clone/validate/commit，生成清晰的变更说明；
- 加入 Undo/Redo 历史、dirty 标记、保存点和新建/打开/退出前未保存保护；
- 把一次拖动或连续数值输入合并为一个合理的撤销步骤；
- 若 axes/style/visibility 等需要新字段，升级 schema 并提供确定性 migration；
- migration 后旧项目的 resolved figure 在未启用新选项时保持一致。

完成条件：

- 所有可见编辑都能撤销、重做、保存、重开；
- 无效输入不改变文档；保存后 dirty 清零；再编辑后重新变脏；
- schema 旧 fixture、损坏恢复和 unknown-field policy 回归继续通过。

### P2：项目生命周期与工作区骨架

工作：

- 整理 File/Edit/View/Export 的菜单结构和常用快捷键；
- 建立集中式 UI 文案层，明确中文/英文界面策略；不得继续在控件代码中散落硬编码文案。
- 区分 Open Data、Open Project 和从 Lite 打开的语义；
- 标题栏显示项目名与未保存状态；
- 打开项目时恢复文档数据并清理旧 session 选择，避免混合前一个项目的数据；
- 保存、另存为、恢复 backup、关闭/替换当前文档时提供明确结果；
- 状态信息分成短暂成功提示、可恢复 warning 和阻断 error，不无限累积重复文本。

完成条件：

- 新建/导入/编辑/保存/重开/另存为的状态机没有含糊分支；
- 取消文件对话框不改变状态；打开失败不破坏当前工作；
- macOS 与 Windows 的标准快捷键和 Unicode/空格路径可用。

### P3：数据、列绑定与 series 管理

工作：

- 数据面板显示 dataset kind、source/fit 关系、行列数、alive 数和列名；
- 导入普通数据后明确选择 X/Y，并创建 line、scatter 或 line + marker；
- 支持增加、复制、删除、显示/隐藏和重排 series；
- 允许重新绑定 X/Y 和 error column，同时验证长度、finite、alive 与 log-domain；
- fit/theory 保留精确 Parent-ID、Source-X/Y 和 equation/display equation；
- 给用户稳定的展示名称，但内部仍使用 stable ID；
- 删除 source 时对依赖的 fit/artists 给出明确选择，不产生悬空引用。

完成条件：

- 用户无需编辑 JSON 即可从普通两列数据得到正确曲线；
- 多数据集不会错绑，重名列和重名文件也不通过猜测解决；
- 删除、重排、隐藏和重绑均可撤销并能 round-trip。

### P4：axes、标签、刻度与图尺寸

工作：

- Inspector 可编辑 X/Y range、autoscale、linear/log scale；
- 可编辑 auto/fixed locator、目标刻度数和 decimal/scientific formatter precision；
- 实现 axis label 的语义编辑入口，覆盖 plain text、变量、Greek、上下标和单位；
- 将四边 spine、major/minor tick、tick direction、tick label side 和 grid 状态纳入文档；
- 落实“四条 spine 有刻度、刻度朝内”的默认值；
- 采用从画布外缘向内的约束：X/Y axis label 的实际墨迹外缘分别固定距画布底边/左边
  6.0 pt，再以 4.0 pt pad 放最近的 tick label，tick label 再以 4.0 pt pad 连接 spine；
  标签或符号变大时只向内调整 axes rectangle，一侧的长文字不得改变另一侧的语义间距；
- margin 必须由所有 decoration 的 union bounds 加外缘安全距离计算；空间不足时缩小
  axes rectangle 或给出明确 warning，不能覆盖、裁切或静默减小字体；
- 支持 figure width/height 与常用期刊尺寸预设，仍以毫米作为项目事实；
- range、log-domain 或标签解析错误就地显示，不提交半有效状态。

完成条件：

- 轴的所有 V1 常用设置无需改项目文件即可完成；
- 最终尺寸、preview、PDF 和 raster 的几何一致；
- 四边刻度、内向刻度和 bottom/left 标签默认值有 snapshot 与视觉回归。
- 85 mm 与 89 mm 宽度、普通/负数/科学计数 tick、长单位和上下标标签的组合矩阵均无
  overlap 或 clipping；改变 X decoration 不得无原因改变 Y 间距，反之亦然。

### P5：artist、颜色与 legend/annotation 编辑

工作：

- line：颜色、真实线宽、dash、角色；
- marker：形状、尺寸、颜色；
- error bar：误差列、cap width、stroke；
- reference/baseline：方向、数值、stroke；
- annotation：语义标签、锚点位置；
- legend：条目、标签、显示/隐藏、顺序和位置；
- source/fit 默认共享对象颜色，同时允许显式覆盖并记录 provenance/override；
- Inspector 根据当前对象类型只显示适用设置，不展示无效组合。

完成条件：

- 每种 B3 已支持 artist 都有最小但完整的编辑入口；
- 属性修改立即通过正式 layout 预览，保存重开和 PDF/PNG 保持一致；
- grayscale/CVD、线宽、透明度和重叠风险能回到具体对象。

### P6：画布交互与选择反馈

工作：

- 使用正式 hit map 支持点击 series、legend、annotation 和 axes，并与左侧树双向同步；
- 选中对象有不进入导出的 overlay，高亮不能修改 Display List；
- 提供 Fit to window、100%、放大/缩小和清晰的当前 zoom；
- preview 滚轮缩放以指针位置为中心；若加入平移，仅作为 view state；
- 处理小窗口、侧栏缩放、高 DPI 和画布溢出滚动；
- 不在 V1 引入任意矢量节点自由拖拽；可拖动对象只限计划明确支持的 annotation/legend。

完成条件：

- 用户可从图上找到并编辑对象，不必猜 stable node 名称；
- 任何 view 操作都不改变导出或 dirty 状态；
- selection overlay 不出现在 PDF/PNG。

### P7：Publication Check 与导出体验

工作：

- finding 按 error/warning/information 分组并关联可选择对象；
- 显示“为什么、影响什么、如何修复”，可以跳转到对应 Inspector 区域；
- 导出前显示图尺寸、格式、DPI、像素尺寸、背景和检查摘要；
- PDF 保持真实文字和嵌入字体；PNG 提供受控 DPI 选项；
- 导出错误不得留下看似成功的空文件，并提供目标路径和可执行的错误信息；
- TIFF 的编码实现和正式发布阻断仍归 B6，但本阶段要冻结与 PNG 共用的 raster 参数模型。

完成条件：

- 用户能从 finding 定位到对象并完成修复；
- 导出设置可保存、重开且不会与 Publication Check 使用不同参数；
- Preview/PDF/PNG 继续来自同一 resolved Display List。

### P8：稳定化、人工验收与阶段关闭

工作：

- 使用 P0 样例完成完整工作流：导入、绑定、编辑、撤销、保存、重开、检查和导出；
- 检查键盘导航、焦点、窄窗口、长标签、极值、空数据和错误输入；
- macOS 本机完成视觉与交互检查；Windows 使用结构化脚本后只做必要人工检查；
- 记录仍需延期的限制，不把未测平台或设备写成通过；
- 形成 B5P closeout 报告，再恢复 Lite producer/launcher 集成，之后进入 B6。

完成条件：

- P0 的每个样例都能由普通用户在 GUI 内完成，不需要手改 JSON 或命令行补步骤；
- 自动测试和人工检查均无阻断问题；
- 已知非阻断问题有明确范围、影响和后续归属。

## 6. 每个增量的验证合同

- `cargo fmt --all --check`；
- workspace tests；
- pinned Rust 1.98.0 Clippy `-D warnings`；
- Figure Document validation、schema migration 和 round-trip；
- deterministic semantic/layout/Display List snapshot；
- PDF 结构与文字提取；
- PNG 视觉回归和物理尺寸检查；
- 不改写输入数据的 hash 检查；
- release binary 与 mandatory assets 体积增量；
- 至少一个真实 GUI 操作路径的人工检查。

为 B5P 建立统一验证脚本，失败必须报告具体阶段、命令、日志路径和失败事实，不能仅用
少数文件存在性或单一 hash 判定通过。

## 7. 明确不进入 B5P 的范围

- 安装包签名、公证、DMG/MSI/DEB 和正式发布页面；
- Lite 仓库 producer/launcher 的最终修改；
- multi-panel、3D、animation、dashboard；
- 完整 spreadsheet/data-cleaning/fitting 平台；
- arbitrary vector editor、插件、云同步和多人协作；
- 完整 LaTeX、CJK 和 SVG editor compatibility。

这些排除项不妨碍完善已有 V1 单 axes 工作流；其中 Lite 最终集成和 TIFF/安装包验收
会在 B5P 关闭后恢复，并作为 B5/B6 的发布前必需项。

## 8. 推荐执行顺序

```text
P0 验收基线
 ↓
P1 编辑事务 / dirty / undo / schema
 ↓
P2 项目生命周期
 ↓
P3 数据绑定与 series 管理
 ↓
P4 axes / 标签 / 刻度 / 图尺寸
 ↓
P5 artists / legend / annotation
 ↓
P6 画布交互
 ↓
P7 Publication Check / 导出体验
 ↓
P8 稳定化与人工验收
 ↓
补完 Lite 集成 → B6 发布准备
```

P0 已完成，真实样例、结构化验证和分级问题清单见
[`../reports/B5P_P0_WORKFLOW_BASELINE.md`](../reports/B5P_P0_WORKFLOW_BASELINE.md)。P1 的编辑事务、
dirty 状态、Undo/Redo、保存点与未保存保护已完成，证据见
[`../reports/B5P_P1_EDIT_TRANSACTIONS.md`](../reports/B5P_P1_EDIT_TRANSACTIONS.md)。当前任务是
P3 的数据绑定与 series 管理证据见
[`../reports/B5P_P3_DATA_SERIES.md`](../reports/B5P_P3_DATA_SERIES.md)，P4 的 axes、语义标签、
刻度和尺寸证据见 [`../reports/B5P_P4_AXES_LABELS_SIZE.md`](../reports/B5P_P4_AXES_LABELS_SIZE.md)。
P5 的 artist、颜色、legend 和 annotation 编辑证据见
[`../reports/B5P_P5_ARTIST_EDITING.md`](../reports/B5P_P5_ARTIST_EDITING.md)。当前任务是
**P6：画布交互与选择反馈**。
