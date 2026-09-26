# InstPlot Studio 代码整合封板计划

状态：待执行
制定日期：2026-09-26
适用仓库：`zhiyuzhang001-a11y/InstPlot-Studio`
本地仓库：`<repository-root>`
目标分支策略：从当前 `main` 建立 `codex/codebase-finalization`，通过 Pull Request 合并回 `main`

## 1. 目标

本计划负责把已经完成的功能、模块化重构、测试、文档和发布工具整理成一个可维护、可复现、可在 GitHub 审查的正式基线。完成后必须同时满足：

1. 本地工作区干净，所有保留、移动、删除和忽略的文件都有明确理由。
2. Git 历史有条理；提交按职责组织，并且每个推送的提交都可构建、可验证。
3. GitHub `main` 包含本地确认过的完整代码，不遗漏新模块、资源、测试或文档。
4. 数据导入、文档状态、文字语义、布局、渲染、导出和 UI 的依赖方向清楚，没有重新堆回主程序。
5. 当前确认的基础功能全部可用；R01–R16 验收结论继续成立。
6. macOS、Windows 和 Linux 的自动门禁与实际支持范围一致。
7. 从全新检出可以只依赖仓库内容和已声明依赖完成构建、测试和关键工作流。
8. 形成明确版本、发布说明、Git 标签和后续开发规则。

“整合完毕”不等于把所有内容塞进一个提交，也不等于继续扩充功能。它表示当前功能范围已经冻结、代码边界稳定、仓库历史清楚、验证可重复，后续功能能从该基线独立开发。

## 2. 当前基线

计划制定时确认：

- 本地分支：`main`。
- 本地 HEAD：`961e5bafec333b0942864329ad099b35f78a64aa`。
- GitHub `origin/main`：同一提交 `961e5bafec333b0942864329ad099b35f78a64aa`。
- 远程目前只有 `main`，没有 tag。
- 当前有约 100 个已跟踪路径发生修改或删除。
- `git status` 有 54 个未跟踪入口；展开后包括正式 crate、拆分模块、资源、测试、ADR、计划、报告和安装脚本，合计约 3.4 MB。
- 四个已迁移的原型目录处于删除状态；`layout-engine-spike` 仍作为兼容性夹具保留。
- workspace 已包含 Studio、第二应用及 text/render/export/ui/layout 正式 crate。
- 重构后的真实操作验收为 R01–R16 全部 PASS。
- 全工作区测试、文档测试和严格 Clippy 已通过；当前安装应用构建号为 `202609261002`。

这些事实只是执行起点。正式提交前必须重新获取远程状态，不得假设 GitHub 始终没有变化。

## 3. 范围与非目标

### 3.1 本次包含

- 当前工作区所有已跟踪修改、删除和未跟踪文件的逐项分类。
- 正式 crate、应用模块、测试、资源、文档、CI 和安装脚本的整合。
- 旧原型迁移完成后的删除确认。
- 重复代码、无效入口、错误依赖方向和本机路径泄漏检查。
- Git 提交整理、远程分支、Pull Request、CI、合并和 tag。
- 干净检出下的构建、测试、安装、Spotlight 和核心工作流复验。
- 完整的版本记录和后续开发约束。

### 3.2 本次不包含

- 双 Y 轴、inset axes 或其他尚未进入当前功能基线的新功能。
- 为了缩短文件而继续机械拆分模块。
- 更换 GUI 框架、项目格式或字体体系。
- 无证据的大规模命名重写或风格重写。
- 强制重写 GitHub `main` 历史。
- 未签名、未说明限制的公开二进制分发。

执行中发现问题时，只修复会破坏当前功能、构建、数据兼容性、模块边界或发布可靠性的问题。新的产品想法单独记录，不混入封板提交。

## 4. 不可破坏的规则

1. 不使用 `git reset --hard`、`git clean -fd`、强制 checkout 或其他会丢失当前工作的命令。
2. 未跟踪文件在完成分类前不得删除。
3. 不直接在 `main` 上堆叠封板提交；先建立专用分支。
4. 不使用 `git add .` 代替文件分类；必须按路径或逻辑分组暂存并复核。
5. 不强推远程 `main`，不绕过失败的 CI。
6. 不为了提交数量而制造不能构建的中间提交；必要时减少提交数量。
7. 不把用户专属绝对路径、本机缓存、QA 临时数据或安装产物提交到仓库；测试中可使用系统临时目录 API 或明确的通用临时路径。
8. 不删除原始科研数据；仓库测试只保留必要、可公开、体积合理的夹具。
9. 二进制字体、图标和基线图片必须带来源、许可或校验信息。
10. 每个阶段失败时停在当前分支修复，不用后续阶段掩盖失败。

## 5. Phase 0：保护现场并同步远程事实

### 操作

1. 记录当前 HEAD、分支、remote、Git 版本、Rust 工具链和工作区状态。
2. 执行 `git fetch --prune origin`，确认远程 `main` 是否仍是计划记录的提交。
3. 从当前 HEAD 创建 `codex/codebase-finalization`；当前未提交修改随工作区保留。
4. 生成一次只读清单：
   - 已修改文件；
   - 已删除文件；
   - 未跟踪文件；
   - 大文件和二进制文件；
   - 可能含绝对路径、凭据、临时目录或用户数据的文件。
5. 在仓库外生成可恢复的二进制 diff 和未跟踪文件清单；不得把恢复包提交到仓库。

### 分支分歧处理

- 如果 `origin/main` 未变化：继续执行。
- 如果远程有新提交：先保留本地封板分支，再审阅远程变化；通过普通 rebase 或 merge 整合，禁止强推。
- 如果远程变化与本地大范围重叠：先形成冲突清单和合并顺序，再继续，不在未理解冲突时批量接受任一侧。

### 门禁

- 当前工作没有丢失。
- 封板分支存在且起点明确。
- 远程事实已刷新。
- 恢复清单位于仓库外。

## 6. Phase 1：逐项分类工作区

每个变动路径必须归入下列一种：

- `KEEP`：正式源码、资源、测试、文档或脚本，应进入 Git。
- `MOVE`：内容正确但路径仍属于旧原型或不合适目录，应迁到正式位置。
- `DELETE`：已被正式实现替代且没有独立历史价值的旧代码。
- `GENERATED`：构建、导出、缓存或临时验收产物，不提交并补充忽略规则。
- `LOCAL_ONLY`：机器配置或个人工具状态，不进入产品仓库。
- `REVIEW`：来源、许可、用途或正确性不明确，阻止下一阶段。

### 必查项目

1. 四个已迁移 `*-spike` 目录的删除是否与正式 crate 一一对应。
2. `layout-engine-spike` 是否仍有唯一兼容性价值；若保留，README 必须说明其身份和退出条件。
3. `apps/instplot-demo` 是否继续作为第二 consumer，而不是复制 Studio 逻辑。
4. 字体、图标和视觉基线是否有许可、来源和 SHA 校验。
5. `reports/` 中哪些是正式质量记录，哪些只是重复或临时过程日志。
6. `.cursor/rules/` 是否属于团队开发规则；若提交，内容不得含本机路径和个人偏好。
7. 安装脚本不得包含固定用户名、临时路径或不可移植假设。
8. `.agent-token-manager/`、`target/`、输出文件和 QA 临时文件继续被忽略。
9. 检查大小写冲突、Unicode 文件名、软链接和 macOS 扩展属性风险。
10. 检查是否意外提交用户 CSV、项目文件、截图或日志。

### 产物

- 一份最终路径分类清单。
- 一份确认删除的旧原型映射。
- 一份需要保留的二进制资源及许可清单。

### 门禁

- `git status` 中每个路径都已归类。
- 没有 `REVIEW` 项。
- 所有删除都有替代位置或明确理由。

## 7. Phase 2：代码与架构封板审计

### 7.1 依赖边界

确认下列方向仍成立：

```text
Studio / Demo
    → application transactions and document commands
    → data / text / UI / export services
    → render
    → layout
```

重点检查：

- 数据、文档、文字和渲染核心不依赖窗口状态或平台 API。
- `AppController` 只协调事务，不重新承载领域算法。
- UI 不直接绕过命令层修改持久化文档。
- 预览、PDF、PNG、SVG 共用同一解析和布局结果。
- 项目文件不持久化 hover、弹窗开关、拖拽预览等临时状态。
- 第二应用通过公共接口复用能力，不引用 Studio 私有模块。

### 7.2 代码质量

- 删除已确认无调用的兼容层、重复 helper 和废弃入口。
- 统一公开类型和函数命名；公共 API 只暴露真实复用需求。
- 错误保持结构化，用户界面不显示内部 ID、Rust 类型名或调试字符串。
- 对复杂事务和兼容迁移保留解释“为什么”的注释，删除复述代码的注释。
- 避免新的超大模块；如果文件仍大，只有职责确实混合时才继续拆分。
- README、ADR 和实现名称必须一致。

### 7.3 功能冻结核对

以 R01–R16 为当前功能完成定义：

- 文件和粘贴数据导入、自动 XY、删除/清空/重导入；
- 坐标、主副刻度、标签和数学文字；
- 曲线、颜色、七种线型、点形和批量样式；
- SD/SEM、X/Y error bar 和自动范围；
- 图例、标注、连接线、多窗口和侧栏；
- 项目保存重开；
- PDF、透明 PNG、SVG；
- macOS 安装和唯一 Spotlight 身份。

发现回归时先添加或确认失败测试，再修复；不得借机新增未计划功能。

### 门禁

- 架构边界与 ADR 一致。
- 没有重复的生产实现或从 UI 绕过事务的写路径。
- 第二应用继续通过公共组件完成真实工作流。
- R01–R16 没有已知回归。

## 8. Phase 3：构造清晰、可验证的提交历史

建议提交序列如下，实际可根据依赖合并，但不得拆成不能构建的提交：

1. `refactor(core): promote production crates and retire migrated spikes`
   - workspace、正式 text/render/export/ui crate；
   - 从旧原型迁移的实现、资源和测试；
   - 被替代原型的删除。
2. `refactor(studio): separate application, document, data and project boundaries`
   - Studio 主程序变薄；
   - controller、transactions、state、document/project 子模块；
   - 导入、清除、保存和撤销事务。
3. `feat(studio): complete publication editing workflows`
   - 已确认的 UI、图例、标注、误差棒、窗口与侧栏行为；
   - 产品资源和 macOS 图标。
4. `test(studio): add contracts and real workflow regression coverage`
   - 集成测试、回归测试、视觉基线、第二应用测试；
   - 已确认的 QA 修复契约。
5. `build(ci): enforce workspace and release gates`
   - GitHub Actions、验证脚本、安装/发布脚本；
   - 三平台门禁和 release 构建。
6. `docs(studio): record architecture, extension and acceptance baseline`
   - ADR、扩展指南、阶段文档、验收报告和维护规则。

### 提交规则

- 每次暂存后检查 `git diff --cached --stat` 和完整 staged diff。
- 禁止混入临时文件、个人路径、测试输出和无关格式化。
- 每个提交至少通过与其范围匹配的测试；所有推送提交必须能构建。
- 如果源码和迁移后的测试不可分割，应放在同一提交，不追求形式上的小提交。
- 删除旧原型必须与正式替代实现处于同一提交或紧邻的可构建提交。
- 文档中的状态必须与该提交实际状态一致，不能提前写“已完成”。

### 门禁

- 提交序列能够解释重构意图和功能来源。
- `git show --stat` 不出现意外文件。
- 在分支任一推送点检出后都能完成基础构建。

## 9. Phase 4：本地干净检出复验

不能只在当前长期工作目录验证。提交完成后建立临时干净 worktree，完全从 Git 内容执行：

### 必须执行

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
cargo test --locked --workspace --doc
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo build --locked --release --package instplot-studio
cargo test --manifest-path prototypes/layout-engine-spike/Cargo.toml --locked
```

并执行：

- 第二应用真实 CSV → XY → 标签 → SVG 流程。
- P0 项目创建、检查、出版检查、PDF、PNG、handoff 工作流。
- 合法 CSV/TSV/TXT/DAT/XLSX 及错误输入矩阵。
- 项目保存重开和 schema 迁移。
- PDF、透明 PNG、SVG 文件解析与关键几何检查。
- 源文件移除后项目仍可重建。
- 安装脚本在不依赖工作目录外文件的情况下生成应用。

### 仓库卫生检查

- 测试后 `git status --short` 必须为空。
- 搜索仓库中的 `/Users/`、`/tmp/`、访问令牌、私钥头和调试凭据。
- 检查所有 tracked 大文件及其必要性。
- `cargo metadata --locked` 成功，Git 依赖固定到不可变 revision。
- `Cargo.lock` 与 manifests 一致。
- README 中的命令可直接复制执行。

### 门禁

- 干净 worktree 全部通过。
- 测试本身不污染仓库。
- release 二进制只依赖仓库声明内容。

## 10. Phase 5：GitHub Pull Request 与远程 CI

### 操作

1. 推送 `codex/codebase-finalization` 到 `origin`。
2. 创建 Pull Request，说明：
   - 为什么重构；
   - 模块边界；
   - 删除了哪些原型；
   - 当前功能范围；
   - 数据和项目兼容性；
   - 测试与真实 QA 证据；
   - 已知非阻断优化项。
3. 在线复核 GitHub 的 Files changed：
   - 无意外二进制和用户数据；
   - rename/delete 被正确识别；
   - 没有漏传本地新模块；
   - 行尾和文件权限无异常。
4. 等待 Linux、macOS、Windows 矩阵全部完成。
5. 失败必须定位并修复；不得重复运行来掩盖确定性失败。
6. CI 通过后再合并到 `main`；不强推、不跳过检查。

### CI 工作流审计

- 路径过滤必须覆盖所有正式 crate、应用、验证脚本、工具链和关键 ADR。
- 格式、全 target 测试、doc test、全 feature Clippy、兼容测试、第二应用、release 和 P0 流程均为强制步骤。
- Windows 命令必须使用 PowerShell 正确处理路径和退出码。
- macOS 安装验证与普通 release 构建区分；不把本机 Spotlight 状态当作 GitHub runner 的稳定契约。
- CI action 固定到明确版本或 commit SHA。

### 门禁

- PR diff 与本地最终提交一致。
- 所有 required checks 通过。
- PR 描述足以让未来维护者理解这次整合。

## 11. Phase 6：合并、版本与发布基线

### 合并后操作

1. 获取最新 `origin/main`，确认合并提交包含 PR 全部提交。
2. 在本地 `main` 快进到远程合并结果，不覆盖其他工作。
3. 在合并后的 `main` 再运行一次快速门禁和 release 构建。
4. 核对 workspace 版本、应用版本、构建号、产品名称、bundle id 和图标。
5. 新建 `CHANGELOG.md` 或等效发布记录，包含：
   - 功能范围；
   - 架构变化；
   - 数据/项目兼容性；
   - 平台支持；
   - 已知限制；
   - 从旧开发版升级的说明。
6. 确认远程不存在同名 tag 后，在合并提交建立带注释 tag，例如 `v0.1.0`。
7. 推送 tag，并核对 GitHub 上 tag 指向正确的 `main` 提交。

### 二进制发布规则

- 本地 Spotlight 应用只替换 `$HOME/Applications/InstPlot Studio.app`，不创建重复副本。
- 安装后验证签名、bundle metadata、Mach-O UUID、启动和唯一 Spotlight 结果。
- 如果发布 GitHub 二进制，必须明确签名/公证状态、macOS 最低版本、架构和 SHA-256。
- 在正式开发者签名与公证方案确定前，可以发布源码 tag 和 release notes，但不得把本地 ad-hoc 签名应用描述成面向公众的正式安全安装包。

### 门禁

- 本地与 GitHub `main` 指向同一整合结果。
- tag 指向正确合并提交。
- release notes 与实际功能和限制一致。
- Spotlight 只有一个当前应用。

## 12. Phase 7：封板后的清理与维护规则

### 本地清理

- 确认 `main` 干净后才删除临时 worktree 和仓库外恢复包。
- 封板分支在远程确认合并后才允许删除。
- 不删除源 CSV、用户项目或验收所需的外部证据，除非用户明确要求。
- 清理操作必须可说明目标，不对工作区根目录执行递归删除。

### 后续开发规则

1. 新功能从最新 `main` 建独立分支。
2. 数据功能进入统一导入/规范化层。
3. 文档变化通过命令和事务，不由 UI 直接改内部状态。
4. 文本入口共用语义解析、字体与诊断。
5. 预览和导出共用 resolved scene。
6. 通用 UI 进入 `instplot-ui`；Studio 专属流程保留在应用层。
7. 每个 bug 先建立最小回归，再修复。
8. 项目格式变化必须带 schema 迁移、旧文件夹具和 round-trip 测试。
9. 新增平台行为必须同时考虑普通、最大化和全屏。
10. 发布只从干净、已合并、已标记的提交生成。

## 13. 最终完成定义

只有以下条件全部满足，计划才能标记“已完成”：

- [ ] 当前所有修改、新文件和删除项均已分类并处理。
- [ ] 没有本机路径、凭据、用户数据或临时产物进入 Git。
- [ ] 正式 crate 与应用边界通过审计，依赖方向符合 ADR。
- [ ] 旧原型删除与正式替代实现一一对应。
- [ ] 提交历史清楚，并且每个推送提交都可构建。
- [ ] 干净 worktree 的格式、测试、Clippy、doc、release 和兼容门禁全部通过。
- [ ] R01–R16 继续通过，没有新增 P0/P1 回归。
- [ ] GitHub PR diff 已人工复核。
- [ ] Linux、macOS、Windows CI 全部通过。
- [ ] PR 已合并，远程和本地 `main` 一致。
- [ ] 版本、CHANGELOG、tag 和发布说明相互一致。
- [ ] macOS 本机只保留一个可启动的当前 Spotlight 应用。
- [ ] 合并后 `git status --short` 为空。
- [ ] 最终封板报告记录 commit、tag、CI、安装构建号和测试证据。

## 14. 最终产物

执行完成后应交付：

1. GitHub 上已合并的代码整合 Pull Request。
2. 清晰的正式提交序列。
3. 干净且与远程一致的本地 `main`。
4. `CHANGELOG.md` 和版本 tag。
5. 可复现的 CI 与 release 构建。
6. 唯一的本机 Spotlight 应用。
7. 一份代码整合封板报告，至少记录：
   - 起点与终点 commit；
   - PR 与 tag；
   - 文件分类和删除结果；
   - 本地与远程测试结果；
   - release 与安装身份；
   - 尚存的非阻断优化项。

## 15. 执行顺序摘要

```text
保护当前工作
  → 刷新远程事实
  → 建立封板分支
  → 逐项分类所有路径
  → 审计架构、资源和仓库卫生
  → 构造可构建的逻辑提交
  → 干净 worktree 全量复验
  → 推送分支并创建 PR
  → 三平台 CI 与在线 diff 复核
  → 合并 main
  → 版本、tag、release 与本机安装
  → 最终报告和工作区清理
```

任一阶段门禁未通过时停在该阶段修复；不得为了“完成计划”把失败降级成文字说明。
