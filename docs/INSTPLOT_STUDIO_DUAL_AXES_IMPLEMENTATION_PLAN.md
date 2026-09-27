# InstPlot Studio 双轴实施计划

> 状态：`IN PROGRESS`（Phase 0–10 已完成；Phase 11 等待用户确认安装）
> 建立日期：2026-09-27  
> 设计基准：[`INSTPLOT_STUDIO_DUAL_AXES_DESIGN.md`](INSTPLOT_STUDIO_DUAL_AXES_DESIGN.md)  
> 实施范围：单轴、双 Y、双 X 三种互斥模式；最多同时显示三条独立坐标轴。  
> 交付策略：本地分阶段完成并验收，在用户确认前不推送 GitHub、不替换 Spotlight 应用。

## 文档配套规则

本计划不能脱离设计方案单独执行。每个阶段必须使用以下闭环：

1. **设计输入**：先阅读设计方案中与本阶段相关的产品规则和边界；
2. **阶段实现**：按本计划限定的模块责任和顺序修改；
3. **技术检查**：运行本阶段命令，保证格式、编译、测试和静态检查通过；
4. **行为验收**：回到设计方案，逐条核对实际行为，而不只判断测试是否为绿色；
5. **证据记录**：在本计划记录完成项、命令结果和真实操作结果；
6. **进入下一阶段**：只有技术门和设计验收同时通过才允许推进。

两份文档发生差异时，以已确认的设计方案为产品事实来源；实施计划必须同步调整。若实现中发现设计不可行或存在新边界，暂停编码，先补充设计决策，再更新实施步骤和测试，不允许只在代码中形成隐含规则。

阶段与设计章节的主要对应关系：

- Phase 0：设计全文和第 11 节验收场景，建立单轴基线；
- Phase 1：设计第 2、3、9 节，落实模式、绑定、休眠和兼容性；
- Phase 2：设计第 3、7 节，落实模式切换与独立自动范围；
- Phase 3：设计第 3.4、7、8 节，落实对称刻度、布局和内容边界；
- Phase 4–5：设计第 2、3、6、8 节，落实预览、窗口、曲线和轴设置；
- Phase 6：设计第 4–6 节，落实文件导入、录入数据和标签建议；
- Phase 7：设计第 8 节，落实图例、标注和外围画布；
- Phase 8：设计第 8.5 节，落实各导出后端；
- Phase 9–11：设计第 11、13 节，执行完整验收并确认所有既定决策完成。

## 1. 完成目标

在不破坏当前单轴行为、项目兼容性和既有模块边界的前提下，完成：

- `X1 + Y1` 单轴模式；
- `X1 + Y1 + Y2` 双 Y 模式；
- `X1 + X2 + Y1` 双 X 模式；
- 曲线级数据列选择与 X/Y 轴绑定；
- 副轴关闭后的休眠、隐藏及重新开启恢复；
- 对称刻度与独立副轴之间的正确切换；
- 导入文件、录入数据和混合数据的统一行为；
- 独立自动范围、误差棒、图例、标注、保存和导出；
- 窗口化、最大化、全屏和本机 Spotlight 应用的真实操作验收。

## 2. 非目标

本轮不实现：

- X2 与 Y2 同时启用的四轴模式；
- 多子图及每个子图的双轴；
- inset axes；
- 三条或更多同方向 Y 轴；
- 主、副轴之间的自动单位换算或数学联动；
- 对任意数据文件进行不可见、不可修改的完全自动语义推断。

## 3. 长任务操作规范

### 3.1 代码与版本控制

- 在本地 `codex/dual-axes` 分支实施；若分支已存在，先核对其基线。
- 开始前记录 `git status --short`，保留所有不属于本任务的用户修改。
- 禁止 `git reset --hard`、大范围 checkout 覆盖以及删除未知文件。
- 每个阶段可以建立本地检查点提交，提交内容只能属于该阶段。
- 用户验收前不推送、不创建 PR、不合并、不创建 tag 或 release。
- 开发阶段不替换 `$HOME/Applications/InstPlot Studio.app`。
- 不修改导入的源文件；项目保存不能意外覆盖外部数据。

### 3.2 修改纪律

- 每阶段只修改其负责的层次，不顺手重构无关功能。
- 先补测试或迁移夹具，再改变公开的数据结构和行为。
- 单轴模式是兼容基线，不能作为完成双轴功能后的次要路径。
- 不允许通过复制一套导入、录入、自动范围或导出代码来实现副轴。
- 不允许通过叠加第二个完整 `AxesRecord` 模拟双轴，以免阻塞未来多子图。
- 不允许仅凭相同 `DataBinding` 猜测线、点、误差棒属于同一曲线；必须建立稳定逻辑曲线身份。
- 不允许通过改写持久 `visible` 实现副轴休眠；全链路使用统一有效可见性。
- 不把 UI 状态写入布局或渲染层；不让渲染层重新推断数据绑定。
- 不为了通过测试而删除测试、降低断言、放宽项目校验或隐藏错误。
- 新增公共结构、枚举或迁移必须有文档说明和 round-trip 测试。

### 3.3 工作过程沟通

每个阶段结束时记录：

1. 修改的文件和职责；
2. 新增或修改的测试；
3. 已运行的命令及结果；
4. 是否发现设计冲突或技术债；
5. 是否满足进入下一阶段的门槛。

长时间构建或测试期间应持续提供简短进度，不让任务在无说明状态下运行。

## 4. 模块责任边界

### 4.1 项目与持久化

主要文件：

- `apps/instplot-studio/src/project.rs`
- `apps/instplot-studio/src/project/migration.rs`
- `apps/instplot-studio/src/project/validation.rs`
- `apps/instplot-studio/src/project/storage.rs`

职责：轴模式、`AxisSlot`/轴身份、逻辑曲线身份、曲线与数据对象的轴绑定、持久可见性、schema、迁移、验证与保存恢复。

禁止：窗口行为、像素布局、文件解析及实际绘制。

### 4.2 文档与业务规则

主要文件：

- `apps/instplot-studio/src/document/axes.rs`
- `apps/instplot-studio/src/document/series.rs`
- `apps/instplot-studio/src/document/autoscale.rs`
- `apps/instplot-studio/src/document/datasets.rs`
- `apps/instplot-studio/src/document/objects.rs`

职责：模式切换、轴绑定、有效可见性、曲线显隐、范围计算、误差棒、数据对象及事务前置验证。

禁止：直接绘制、直接创建窗口或改写导入文件。

### 4.3 布局

主要位置：

- `crates/instplot-layout`

职责：四侧边缘唯一所有权、主/副轴几何、按轴组合映射、刻度标签占位、命中区域、外围画布、图例位置和 resolved 内容边界。

禁止：读取数据文件、修改项目或重新决定曲线绑定。

### 4.4 渲染与导出

主要位置：

- `crates/instplot-export`
- `apps/instplot-studio/src/render.rs`
- `apps/instplot-studio/src/export.rs`
- `apps/instplot-studio/src/preview.rs`
- `apps/instplot-studio/src/publication.rs`

职责：沿正式 `document → instplot-layout → DisplayList → instplot-export` 路径消费统一布局结果，保证预览、PDF、SVG、光栅输出和出版尺寸检查一致。

禁止：单独维护另一套范围、刻度或图例布局算法。

`crates/instplot-render` 属于较旧路径；除非代码追踪证明正式双轴流程必须修改，否则本任务不为双轴重构该 crate。

### 4.5 数据导入与录入

主要文件：

- `apps/instplot-studio/src/data.rs`
- `apps/instplot-studio/src/import_flow.rs`
- `apps/instplot-studio/src/document/datasets.rs`

职责：产生统一数据源、列信息和曲线创建输入；支持同文件多曲线、多文件及录入数据。

禁止：把文件直接绑定到轴；双轴绑定属于曲线属性。

### 4.6 UI 与交互

主要文件：

- `apps/instplot-studio/src/app_state.rs`
- `apps/instplot-studio/src/app_controller.rs`
- `apps/instplot-studio/src/app_ui.rs`
- `apps/instplot-studio/src/editor_support.rs`
- `apps/instplot-studio/src/canvas_support.rs`

职责：模式入口、曲线轴选择、轴编辑窗口、休眠提示、选择和拖拽反馈。

禁止：绕过 document/transaction 层直接篡改项目内部结构。

## 5. 阶段门与执行清单

### Phase 0：基线与隔离

状态：`DONE`（证据：[`reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE0_BASELINE.md`](../reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE0_BASELINE.md)）

- [x] 确认工作区和当前分支状态；
- [x] 建立或切换到本地 `codex/dual-axes` 分支；
- [x] 记录当前 schema、版本、测试数量和 release 构建状态；
- [x] 保存代表性的单轴项目、预览和导出基线；
- [x] 记录可比较的单轴布局快照、导出物理尺寸和像素尺寸；
- [x] 确认当前工作区没有被意外修改；
- [x] 运行完整基线门禁。

基线命令：

```bash
git status --short
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
cargo test --locked --workspace --doc
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo build --locked --release --package instplot-studio
git diff --check
```

通过条件：所有既有门禁通过，或已经把与本任务无关的既有失败单独记录。

停止条件：基线存在未解释的失败、工作区包含可能被覆盖的用户修改，或无法复现当前单轴行为。

### Phase 1：项目模型、schema 与迁移

状态：`DONE`（证据：[`reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE1_MODEL.md`](../reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE1_MODEL.md)）

- [x] 定义单轴、双 Y、双 X 三种互斥模式；
- [x] 定义稳定的 X1、X2、Y1、Y2身份；
- [x] 建立稳定逻辑曲线身份，将线、点、误差棒和图例归入同组；
- [x] 为逻辑曲线增加 X/Y 轴绑定，旧值默认 X1/Y1；
- [x] 校验单曲线只允许 X1/Y1、X1/Y2、X2/Y1，拒绝 X2/Y2；
- [x] 保存 X2、Y2各自设置和休眠曲线绑定；
- [x] 校验禁止 X2/Y2 同时启用；
- [x] 为参考线和数据坐标箭头连接点增加轴组合，普通画布文字保持无轴绑定；
- [x] 定义四侧 `AxisSlot` 或等价的唯一边缘所有权；
- [x] 升级项目 schema；
- [x] 完成旧 schema 0–7 到新 schema 的迁移；
- [x] 迁移先按图例代表 artist 建立锚点组，再把只能唯一匹配一个相同绑定锚点组的互补 artist 加入该组；
- [x] 对没有图例锚点的剩余 line/scatter/error-bar，仅在互补候选唯一且无歧义时归组；
- [x] 重复绑定、多锚点或多候选保持独立逻辑组并产生非破坏性警告，不删除、合并或重排原 artist/图例；
- [x] 逻辑组 ID 基于锚点 artist ID 或稳定文档顺序生成，相同旧项目的迁移结果确定且可重复；
- [x] 覆盖 JSON round-trip、旧项目迁移和无效组合拒绝测试。

阶段命令：

```bash
cargo test --locked -p instplot-studio project
cargo check --locked --workspace --all-targets
cargo fmt --all -- --check
git diff --check
```

通过条件：旧项目外观所需信息不丢失；保存重开后模式、轴设置和绑定完全一致。

停止条件：需要删除旧字段才能迁移、旧项目无法无损打开，或模型开始依赖 UI 类型。

### Phase 2：文档操作与独立自动范围

状态：`DONE`（证据：[`reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE2_DOCUMENT.md`](../reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE2_DOCUMENT.md)）

- [x] 模式切换使用 document/transaction API；
- [x] 曲线轴绑定修改可撤销、重做；
- [x] 定义统一 `effective_visible = artist.visible && (unbound || bound_axes_enabled)`；
- [x] 关闭副轴时休眠并隐藏绑定曲线，但不改写持久 `visible`；
- [x] 重开副轴时恢复曲线、误差棒及对象；
- [x] X1、X2、Y1、Y2分别计算可见数据范围；
- [x] X/Y 误差棒完整参与相应轴范围；
- [x] 数据参考线保持现有行为并参与所属轴自动范围；
- [x] 箭头连接目标和普通数据标注默认不扩展范围，画布文字永不参与数据范围；
- [x] 空列、单点、NaN、无穷值具有稳定行为；
- [x] 删除最后一条副轴曲线后进入明确无数据状态；
- [x] 清除全部数据及重新导入后不保留陈旧范围。
- [x] 自动范围、图例、出版规范检查、命中测试和导出调用同一有效可见性规则。

阶段测试重点：

- 单轴回归；
- 双 Y 范围；
- 双 X 范围；
- 误差棒；
- 模式切换；
- 删除、清除、撤销、重做。

通过条件：业务行为不依赖 UI；所有自动范围均由统一实现产生。

### Phase 3：布局、刻度与内容边界

状态：`DONE`（证据：[`reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE3_LAYOUT.md`](../reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE3_LAYOUT.md)）

- [x] 单轴顶部镜像 X1、右侧镜像 Y1；
- [x] 双 Y 右侧显示独立 Y2，顶部继续镜像 X1；
- [x] 双 X 顶部显示独立 X2，右侧继续镜像 Y1；
- [x] 四侧边缘始终只有一个所有者，不同时绘制主轴 far ticks 和副轴 ticks；
- [x] `Chart`/正式布局输入保存 X1/Y1 及可选 X2 或 Y2；
- [x] 逻辑曲线、参考线和数据连接点携带轴组合；
- [x] 映射函数接收轴组合，不固定读取 `chart.x/chart.y`；
- [x] `LayoutResult` 保存四侧布局、标签边界和命中区域；
- [x] 返回单轴后副轴标签和数值无残留；
- [x] 主、副刻度使用统一 formatter 和数学负号；
- [x] 无数据副轴不显示虚假 `0–1` 刻度；
- [x] 长刻度、科学计数法和长轴标签获得正确边距；
- [x] 左上、右上、右下角无重叠或裁切；
- [x] 主绘图区尺寸和长宽比保持既定策略；
- [x] `width_mm/height_mm` 保持现有单轴基础画布语义，不重新解释为纯数据区域；
- [x] 副轴和图外图例只扩展外围 resolved canvas，原单轴数据区域几何不变；
- [x] 内容边界只按可见内容紧凑扩展。

通过条件：布局模型同时服务预览和所有导出后端；不存在仅在预览中补偿的偏移。

### Phase 4：预览、选择与轴窗口

状态：`DONE`（证据：[`reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE4_8_INTEGRATION.md`](../reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE4_8_INTEGRATION.md)）

- [x] 预览按每条曲线绑定映射坐标；
- [x] 休眠曲线不显示、不命中、不参与选择高亮；
- [x] 双击四侧时能区分镜像边缘和独立副轴；
- [x] 轴编辑窗口使用统一置顶规则；
- [x] 模式切换立即更新预览和编辑器状态；
- [x] 左下角坐标信息：单轴显示 X1/Y1，双 Y显示 X1/Y1/Y2，双 X显示 X1/X2/Y1；
- [x] 窗口化、最大化、全屏使用统一 ToolWindow 策略；真实操作列入 Phase 10。

通过条件：预览只消费 document/layout 结果，不在 UI 层重复推导范围或轴角色。

### Phase 5：曲线设置与坐标轴模式 UI

状态：`DONE`（证据同 Phase 4）

- [x] 提供单轴、双 Y、双 X 三种互斥模式；
- [x] 曲线设置只展示当前可使用的轴；
- [x] 休眠曲线清楚显示“副轴已关闭，曲线已隐藏”；
- [x] 新曲线始终默认 X1/Y1；
- [x] 开启副轴不会擅自移动已有曲线；
- [x] 模式直接切换会休眠旧副轴曲线并恢复新模式副轴曲线；
- [x] 标签建议只在空轴首次分配时提供，不覆盖用户标签；
- [x] 仅在能可靠获得明确单位时提示多曲线单位冲突，不根据模糊列名猜测或自动换算。

通过条件：普通单轴用户不会看到不必要的复杂控件。

### Phase 6：导入文件与录入数据统一

状态：`DONE`（证据同 Phase 4）

- [x] 同一文件可创建多条曲线；
- [x] 支持共享 X 的 `X,Y1,Y2`；
- [x] 支持独立 X 的 `X1,Y1,X2,Y2`；
- [x] 支持多个文件分别生成曲线；
- [x] 支持 X/Y 误差列；
- [x] 支持录入数据的重复测量和自动误差；
- [x] 导入文件与录入数据混合时样式和绑定不串位；
- [x] 自动列名建议可修改，无法确定时不乱猜；
- [x] 重载列结构变化的文件时不静默绑定错误列；
- [x] 左栏卡片、删除、清除、保存和重新打开行为一致；
- [x] 导入源文件保持只读；录入数据继续使用现有嵌入与外部文件保存规则。

通过条件：导入文件与录入数据在生成数据源后进入同一条曲线/轴业务路径。

### Phase 7：图例、标注与图外画布

状态：`DONE`（证据同 Phase 4）

- [x] 单轴、双 Y、双 X 的图例默认均在图内；
- [x] 开启副轴不会自动迁移图例；
- [x] 双 Y 可选择图上方图例；
- [x] 双 X 可选择图右侧图例；
- [x] 用户手动位置不被模式切换覆盖；
- [x] 图外图例扩展紧凑画布，不压缩主绘图区；
- [x] 上方图例自动使用紧凑行列，右侧图例使用紧凑纵向/多列；
- [x] 休眠曲线的图例条目隐藏并按原顺序恢复；
- [x] 使用 Phase 1已保存的轴组合正确编辑和拖拽数据坐标参考线、箭头及标注；
- [x] 依赖休眠副轴的数据坐标对象同步隐藏；
- [x] 画布坐标普通文字不被错误绑定到数据轴。

通过条件：图例与标注在模式切换、保存重开和拖拽后不跳位、不丢失。

### Phase 8：导出与跨后端一致性

状态：`DONE`（证据同 Phase 4）

- [x] PDF、SVG、PNG 使用同一布局和内容边界；
- [x] 镜像刻度与独立副轴在各后端一致；
- [x] 隐藏曲线、误差棒、图例和标注不进入导出；
- [x] 数学文字、负号、线宽、点和误差棒样式保持规范；
- [x] 长标签和图外图例不裁切；
- [x] 输出完整且无大面积无意义空白；
- [x] 出版规范检查排除休眠对象，并读取 resolved layout 的最终物理尺寸和像素尺寸；
- [x] 基础画布尺寸与副轴/图外图例增加后的最终导出尺寸语义清楚且经过测试；
- [x] 导出不改变项目状态或数据文件。

通过条件：相同项目的预览、PDF、SVG 和光栅输出语义一致。

### Phase 9：全量自动化门禁

状态：`DONE`（自动化证据见 Phase 4–8 集成报告与 `target/b5p-audit/summary.json`）

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
cargo test --locked --workspace --doc
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo build --locked --release --package instplot-studio
python3 scripts/audit_b5p.py
git diff --check
```

- [x] 全量测试通过；
- [x] 文档测试通过；
- [x] Clippy 零警告；
- [x] release 构建通过；
- [x] 既有审计通过；
- [x] 无临时调试代码、测试文件或无关修改；
- [x] 单轴基线没有非预期变化；
- [x] 单轴布局快照、导出物理尺寸和像素尺寸与 Phase 0基线一致。

任一门禁失败时，读取原因、修复并重跑相关阶段及全量门禁，不能带失败进入真实安装验收。

### Phase 10：真实操作 QA

状态：`DONE`（证据：[`reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE10_QA.md`](../reports/INSTPLOT_STUDIO_DUAL_AXES_PHASE10_QA.md)）

至少执行：

- [x] 旧单轴项目打开、编辑、保存、重开；
- [x] 先导入再开双 Y；
- [x] 先开双 Y再导入；
- [x] 双 Y关闭、隐藏、保存、重开、恢复；
- [x] 双 Y直接切换双 X，再切回；
- [x] `X,Y1,Y2`；
- [x] `X1,Y1,X2,Y2`；
- [x] 两个文件；
- [x] 同文件多条曲线；
- [x] 录入数据及重复测量；
- [x] 导入文件与录入数据混合；
- [x] X/Y 误差棒自动范围；
- [x] 点线图的线、点、误差棒和图例作为同一逻辑曲线切换轴；
- [x] 用户主动隐藏的曲线在副轴关闭并重开后仍保持隐藏；
- [x] UI 和项目校验均拒绝单曲线 X2/Y2；
- [x] 左下角坐标信息在三种模式下正确；
- [x] schema 0–7代表项目全部迁移通过；
- [x] 出版规范报告最终 resolved 导出尺寸且排除休眠对象；
- [x] 图例图内、图上、图右及拖拽；
- [x] 长标签、科学计数法及三个重点角落；
- [x] PDF、SVG、PNG；
- [x] 窗口化、最大化和全屏；
- [x] 删除单数据源、删除副轴最后一条曲线、清除全部、再次导入。

QA 必须记录输入、操作、预期、实际结果和证据。发现问题后回到责任阶段修复，并至少重跑受影响阶段和 Phase 9。

### Phase 11：本机安装与最终验收

状态：`PENDING`

前提：Phase 9 和 Phase 10 全部通过，用户允许替换本机应用。

```bash
python3 scripts/install_studio_macos.py
```

- [ ] 关闭正在运行的旧应用；
- [ ] 只替换 `$HOME/Applications/InstPlot Studio.app`；
- [ ] 验证 bundle ID、版本、构建号、图标和签名；
- [ ] Spotlight 只出现一个 InstPlot Studio；
- [ ] 从 Spotlight 启动后复验最小单轴、双 Y、双 X和导出流程；
- [ ] 不在用户验收前提交 GitHub。

## 6. 分层测试策略

### 修改后立即运行

- 当前文件所属 crate 的编译检查；
- 当前模块的定向单元测试；
- `cargo fmt --all -- --check`；
- `git diff --check`。

### 每个阶段结束运行

- `cargo check --locked --workspace --all-targets`；
- `cargo test` 的相关 package/模块过滤；
- 与上一阶段相邻的回归测试。

### 模型、布局或导出接口变化后运行

- `cargo test --locked --workspace --all-targets --all-features --no-fail-fast`；
- 项目 round-trip 和旧 schema fixture；
- 预览/导出结构测试。

### 最终运行

- Phase 9 全部命令；
- Phase 10 真实操作；
- Phase 11 安装验收。

## 7. 强制停止条件

出现以下任一情况必须暂停推进并定位原因：

- 旧项目无法打开或保存后发生不可逆变化；
- 单轴预览、刻度、图例或导出发生非预期回归；
- X2/Y2 可以同时启用；
- 关闭副轴导致数据或设置被删除；
- 关闭副轴改写了用户持久 `visible`；
- 休眠曲线参与主轴自动范围或仍出现在导出中；
- 线、点、误差棒或图例在切换轴后脱离同一逻辑曲线；
- 单曲线能够保存为 X2/Y2；
- 主轴远端镜像刻度与副轴刻度同时占用同一边缘；
- 误差棒未包含在对应轴自动范围内；
- 导入文件和录入数据走不同双轴实现；
- UI 绕过事务/document 层直接修改项目；
- 布局、预览和导出出现多套坐标计算；
- 需要降低校验、删除测试或忽略 Clippy 才能继续；
- 出现无关大范围重构、依赖升级或工作区污染。

## 8. 回退与恢复

- 每个阶段通过后建立本地检查点；
- 阶段失败时只回退该阶段的任务修改，不影响用户原有内容；
- schema 迁移必须保留旧 fixture，不能只测试新项目；
- 中断后从计划文件的阶段状态、最后通过命令和本地提交恢复；
- 不以安装中的 `.app` 作为唯一成果，源代码、测试和项目迁移才是事实来源。

## 9. 最终完成定义

只有同时满足以下条件，计划才能标记 `COMPLETED`：

1. Phase 0–11 全部通过；
2. 单轴、双 Y、双 X三种模式行为与设计文档一致；
3. 导入文件、录入数据、误差棒、图例、标注、保存和导出全部覆盖；
4. 旧项目迁移和新项目 round-trip 通过；
5. 格式、测试、doc test、Clippy、release 构建和审计通过；
6. 窗口化、最大化、全屏真实操作通过；
7. 本机 Spotlight 只有一个正确应用；
8. 没有隐藏的 P0/P1 问题、临时绕过或未记录限制；
9. 用户完成本地版本验收后，才另行决定是否提交 GitHub。

## 10. 当前进度

- [x] 产品讨论已整理；
- [x] 双轴设计文档已建立；
- [x] 长任务实施计划已建立；
- [x] Phase 0：基线与隔离；
- [x] Phase 1：项目模型、schema 与迁移；
- [x] Phase 2：文档操作与独立自动范围；
- [x] Phase 3：布局、刻度与内容边界；
- [x] Phase 4：预览、选择与轴窗口；
- [x] Phase 5：曲线设置与坐标轴模式 UI；
- [x] Phase 6：导入文件与录入数据统一；
- [x] Phase 7：图例、标注与图外画布；
- [x] Phase 8：导出与跨后端一致性；
- [x] Phase 9：全量自动化门禁；
- [x] Phase 10：真实操作 QA；
- [ ] Phase 11：本机安装与最终验收。
