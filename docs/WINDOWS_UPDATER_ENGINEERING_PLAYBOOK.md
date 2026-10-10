# Windows 桌面软件原位更新：实现、失败复盘与复用指南

记录日期：2026-10-10（北京时间）。本文来自 InstPlot Studio rc.2 → rc.3 的实际修复，不是新更新框架的设计提案。

## 结论与适用范围

用户已确认新 QA baseline rc.2 覆盖安装后正常打开，并在授权激活 rc.3 后反馈：“这次没问题了，大概5s以内完成更新”。这是一次实机用户观察，不是自动采样的性能基准，也不代表全部 Windows 版本、GPU、网络与安装布局已覆盖。

这次成功不是只延长等待，也不是换了更新框架。最终交付同时修正了真实包的构建配置、后台检查节流、父 GUI 与助手交接、候选窗口健康判定，以及健康提交后的正常编辑界面重绘。较早“安装成功/事务 completed”的结果没有证明用户能看到并使用新界面。

可复用的是流程分层、身份绑定、安全边界和验收方法。当前代码仍属于 [updater PR #9](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/pull/9)，该 PR 未合并；只合并了隔离 QA 的部署工作流。本文不授权启用生产更新，也不宣称这些模块已成为可直接引用的通用库。

## 从用户点击到可用新版

1. Release 应用启动后，在后台获取允许源内的更新清单并验证签名、通道、有效期和序列。只有已验证的新版才显示提示；用户点击“更新并重启”才下载、准备和安装。
2. 下载候选安装器，并准备与当前已安装版本精确匹配的可信恢复安装器。两包都校验签名证据、大小和 SHA-256；私有缓存和安装器只读租约防止校验后被替换。
3. 处理保存、草稿及项目恢复信息。后台准备唯一助手，绑定安装身份、原进程 PID/创建时间、事务与 nonce；助手持有事务写锁并发布原子 readiness 回执。
4. 父 GUI 确认助手就绪，再执行完整关闭前证明。只有证明仍新鲜、助手确实存活且事务仍允许交接，才正常关闭旧 GUI。回执本身不是关闭授权，诊断写盘不能刷新授权时间。
5. 助手等待绑定的旧进程正常退出和同安装的其他实例释放访问，再取得排他访问，执行已验证安装器，在原目录核验安装结果。不得强杀其他实例或在未知执行状态下重放安装。
6. 助手只启动一次候选 GUI，并持久绑定实际进程创建时间与路径。候选恢复项目，绘制首画布，并通过真实原生根窗口可见性门后写健康回执；助手据此提交事务。
7. GUI 观察到健康已提交后解除初始化保护，并明确安排下一帧，进入正常编辑器与项目标题。验收继续检查唯一新版进程持续可见、项目不变、快捷方式/版本正常，最后正常关闭退出码为 0。

健康失败与恢复仍走既有持久事务和可信旧包路径，不能通过删除缓存、伪造 receipt 或再启动一次应用来“修好”结果。

## 为什么前面失败，而这一版成功

### 真实包是 Debug，自动检查根本没有运行

旧 KitOnly QA 两次构建没有 `--release`，并复制 `target/debug`。应用启动检查遇到 `cfg!(debug_assertions)` 会直接返回，所以旧 QA 没有自动弹窗是明确的构建配置问题，不是靠增加轮询次数能解决的网络问题。

修复两版打包均使用 Release，并由实际二进制报告 `build_profile` 与 `startup_update_check_enabled`。打包、stage 和部署 validate 都要求 `release` / `true`，拒绝缺字段、Debug、false 和 null。Debug 组件测试可以保留，但不得代表 Release 用户行为。

### 失败的后台检查不应留下成功节流

检查开始前保存节流时间，会让失败后重开也跳过检查。修复为 reservation 持跨实例文件锁穿过网络和签名校验，仅成功检查才记录节流；失败释放锁、允许后续重试，并只保存固定的非敏感失败诊断。成功节流为 5 分钟，真实 GUI 测试保留并等待这个时间，没有删除状态缩短测试。

### 助手交接的等待与昂贵证明要分开

缺 readiness 时重复完整安装身份/文件哈希验证，既增加等待成本，也混淆了“没就绪”和“校验慢”。修复为等待时只做便宜的绑定回执探测，及时就绪后仍执行完整证明和最终新鲜性门。就绪预算调整不是提速证明；保留身份、安全与取消门才是前提。

准备、就绪、full proof、旧进程退出、安装、新 GUI 启动、健康和终态按各自边界计时；累计时间不能相加。旧实机约 150 秒的全部耗时没有完整分段证据，不能把它全部归因于 Debug、哈希或回退。

### 首画布不等于可见窗口；可见窗口也不等于正常编辑器

旧 `FIRST_CANVAS` 只证明渲染发生，不证明原生窗口可见。新健康门从 eframe 的实际根窗口取得 Win32 HWND，每次核验句柄有效、当前 PID、`GA_ROOT`、可见、非最小化、非 DWM cloaked、正面积且与显示器相交，之后才允许首次健康回执。候选/恢复共用该门，进程和事务绑定、停止请求、有限等待不变。

对同一个候选/恢复 PID 只发送一次 `Visible(true)`、`Minimized(false)`、`Focus`。前台焦点是否获准与窗口是否可见是不同事实。不得给 Inno 加第二个启动项，也不得在窗口没出现时再次 spawn。

更晚的真实 GUI 测试发现：事务 completed 后窗口仍是灰色禁用画布和初始标题。健康分支清除状态后 `return`，本帧已经在禁用 UI 中绘制，事件驱动界面不保证自行再画一帧。最终修复是在 `Committed` 后 `request_repaint()`，保留本帧 `return`，下一帧才进入正常编辑器；不跳过健康门、不直接落入已经禁用的 UI。

### 有些失败属于测试环境或证据采集

- GitHub Windows VM 有交互桌面，但最初缺 OpenGL 2.0，GUI 退出 2。只在临时 runner 安装目录使用固定来源/校验的 Mesa llvmpipe，不改 System32、不修改用户安装包。这是软件 GPU 环境证据，不代表用户 GPU。
- 1024×768 桌面触发产品现有 embedded 工具窗策略。只查独立 HWND 的测试误判“没有弹窗”；截图实际已经显示更新提示。夹具只调整自己绑定的主窗口尺寸，让既有策略自然使用独立工具窗，不伪造产品 open 状态。
- 对非直接子进程延迟读取 `Get-Process.ExitCode` 缺少可靠退出证据。夹具改为关闭前保留查询/同步原生句柄、绑定创建时间，正常关闭后等待并读实际退出码；真实 0/7 自退出用例验证判定。
- Windows 默认换行/编码导致快照 guard 和中文源码写入失败；使用显式 UTF-8 字节，并保留严格源字节比对。快照改 rc.3 时只精确同步四个 workspace 包的 lock 版本，不更改外部依赖 pins，继续 `--locked --offline`。

上述夹具修正不能冒充产品修复。产品源码变更必须重新构建准确源的包；仅测试变化也须公开实际测试 SHA 与包 SHA，并证明相关输入等价。

## 验收方法：避免再次出现“绿了但用户打不开”

把证据分成四层，任何一层都不能代替其他层：

1. 组件与原生安装恢复：签名/TLS、序列、ACL、租约、进程绑定、多实例、取消、健康、恢复故障矩阵；实际 Inno 升级和恢复，不只用 mock。
2. 真实 GUI：启动自动提示 → 实际点击 → 旧 GUI 正常退出 → 原目录安装 → 唯一新 PID 可见并持续存活 → 正常编辑器/项目标题 → 项目、快捷方式和版本 → completed → 正常退出 0。截图和原生窗口事实必须一起检查。
3. 准确公有资产：source SHA、build run/attempt、实际二进制能力、双公钥、完整 Rust 客户端清单语义、恢复契约、资产大小/SHA、精确 public inventory、部署回执和公网 latest。签名数学验证成功不等于客户端会接受清单。
4. 用户实机：真实安装布局、GPU、会话和操作体验确认。记录为用户观察，不伪装成 CI 采样，也不要求用户成为主要调试器。

隔离 GUI 夹具使用 localhost SAN 证书与严格 TLS，只在有标记的 runner 临时 git archive 注入公开夹具 CA，并记录前后源码哈希。不得关闭证书/主机名校验、导入系统 CA、改 hosts 或把补丁加入产品。该快照不是公网 exact binary，因此第 3 层必须另做。

## 最终可追溯证据

- 修复源码：[`be5da81119ae8ef45c7ae1638f9bed86f9e86a8e`](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/commit/be5da81119ae8ef45c7ae1638f9bed86f9e86a8e)。后续本文整理不改变该已验证的包来源。
- [完整 Windows GUI run 37965535242](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/37965535242)：`overall_success=true`，实际自动提示/点击、唯一新版窗口持续 30 秒、正常项目编辑器、原生退出 0；点击到 completed 约 8.9 秒，仅适用于这次软件 GPU / loopback 夹具。
- [原生 Quality run 37965537828](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/37965537828) 与 [audit](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/37965537798)：四项必需检查通过，原生窗口/后台 reservation 与两次真实 Inno 更新恢复已核验。
- [准确 Release QA build 37965541633 attempt 1](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/37965541633)：两版实际 Release、启动检查启用；完整 artifact 校验通过。
- [仅部署工作流 PR18](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/pull/18) 在自身四项 CI/审查通过后正常合并；[发布 run 37970607385](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/37970607385) 只发布新 QA baseline rc.2，未重建。
- 用户确认 baseline 后另行授权，[激活 run 38035232708](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/38035232708) 将同一 QA latest 切到 rc.3。独立公网验证再次通过。生产源和旧 QA 未修改，PR9 未合并。
- 2026-10-10 用户最终反馈更新正常、约 5 秒以内。用户实机没有采集同口径分段计时，不能用它计算与 CI 或早期 150 秒的精确加速比。

GitHub run/artifact 可能受保留期限约束；本文保留来源与结论，不承诺证据附件永久可下载，也不提交私钥、项目内容、机器路径或完整私有事务日志。

## 给其他软件复用时的检查清单

- 保留分层协议：发现/验证、准备、交接、安装、单次启动、健康提交、正常 UI；不要把“安装器退出 0”当作终点。
- 独立适配产品 ID、通道、可信公钥、允许 HTTPS 源、序列状态、用户级安装注册、快捷方式、路径和项目恢复。不得复用 Studio 的注册表身份、信任材料或 QA endpoint。
- 为实际 GUI 框架实现根窗口健康和提交后重绘；egui/eframe 的 viewport 命令不能原样套到别的框架。窗口可见是最低门槛，不保证全部功能可用，仍需应用自己的健康操作。
- 日志、nonce 和授权分开；诊断失败不授权更新，诊断成功也不是健康证明。不要复制本次具体等待时长作为通用默认值。
- 验证权限模型和恢复策略；本次是用户级 Inno 安装，不能直接外推到系统级、服务、驱动、MSIX、便携包或其他操作系统。
- 固定一次成功构建的不可变资产，由部署消费，不为上传重建。产品变化重新构建；测试变化明确输入等价性，不能套用旧源证据。
- 先部署 baseline、确认可信恢复资产，再经授权激活 candidate；保留受保护环境/OIDC和并发门，不绕过 required CI。
- 提取通用库应单独立项：先定义平台/安装器/GUI 适配接口及安全契约，再拆分代码与测试。本次整理不新增依赖、不迁移 Velopack、不自动启用其他产品。

## 实现与历史记录入口

- [应用内检查与后台交接](../apps/instplot-studio/src/app_update.rs)、[健康后正常 UI 重绘](../apps/instplot-studio/src/app_ui/shell.rs)。
- [父端助手准备](../apps/instplot-studio/src/update_windows/parent_helper.rs)、[控制器](../apps/instplot-studio/src/update_windows/controller.rs)、[候选健康](../apps/instplot-studio/src/update_windows/candidate.rs)、[原生根窗口](../apps/instplot-studio/src/update_windows/window.rs)。
- [安装执行](../apps/instplot-studio/src/update_windows/runner.rs)、[进程身份](../apps/instplot-studio/src/update_windows/process.rs)、[安装访问锁](../apps/instplot-studio/src/update_windows/access.rs)、[资产与恢复](../apps/instplot-studio/src/update_windows/assets.rs)。
- [真实 GUI 驱动](../scripts/test_windows_gui_e2e.ps1)、[隔离 TLS 夹具](../scripts/windows_gui_e2e_fixture.py)、[准确 QA 包验证](../scripts/windows_gui_qa.py)、[公网独立验证](../scripts/verify_public_release.py)。
- [阶段性问题原始记录](INSTPLOT_STUDIO_WINDOWS_GUI_FINDINGS.md)、[助手完整验收历史](INSTPLOT_STUDIO_UPDATE_HELPER_ACCEPTANCE.md)。历史“待运行”是当时状态；最新结论以最终记录和绑定 source/run 的证据为准。
