# 原位更新助手验收记录

更新至：2026-10-02。测试属于未发布的 PR #9；产品原位更新入口默认关闭。

## macOS 实际证据

使用 `scripts/test_macos_update_gui.py` 建立私有、非 Spotlight 索引测试目录。旧版为实际 Studio rc.2，新版为隔离源码快照构建的 rc.3；不修改产品版本、不替换用户应用、不使用生产更新密钥。两个测试包均 ad-hoc 签名，由真实 GUI 首画布写回执，测试程序不代写健康回执。

- 正常升级：旧 GUI 正常关闭后，助手交换 bundle；新版打开保存项目，事务进入 completed，版本、二进制哈希、回执事务/nonce/进程绑定正确，旧版备份保留。实际窗口显示新版本，随后正常关闭测试窗口。
- 健康失败：仅在私有故障夹具中使健康回执路径不可写。新版正常退出，助手恢复旧版并重新打开保存项目；事务 rolled_back，旧版版本与哈希正确，失败候选保留，GUI 显示恢复提示，关闭提示后图形正常。
- 两种终态项目文件哈希不变；实际安装后/恢复后的二进制通过项目校验及 PNG/PDF 固定图导出。
- 另一次等待旧 GUI 超时停于 failed_before_apply，没有替换旧程序；它不是回滚成功案例，不计入上述健康失败通过证据。

机器本地 JSON、助手日志与导出图片保留于忽略的 `target/macos-update-qa/run.*`，不提交路径、进程标识、事务 nonce 等私有信息。

限制：验证的是包准备后的真实助手交换/健康/恢复路径，不是签名下载→DMG→用户重启按钮完整链路。quarantine、多实例、保存失败、未应用草稿等真实交互仍须继续测试，不能称为完整自动更新验收。

补充 DMG 组件实测：真实 ad-hoc 签名原生身份探针打包成只读 DMG，中文/空格路径的复制、签名/版本/哈希/协议验证和正常卸载通过；错误版本拒绝且不留下已挂载映像。5 项 macOS 助手测试通过。该探针不启动 GUI、不写健康回执，不能计为完整下载/重启链路。

首次 DMG 实测中，直接执行挂载映像内的身份探针导致 `syspolicyd` 拒绝卸载（`diskutil` 明确报告 dissenting process）。修复为挂载源静态校验 → 复制后静态校验 → 正常卸载 → 本地副本完整身份/协议校验；未强制卸载、未移除 quarantine 或绕过系统安全检查。正常卸载采用有限重试，持续失败保留暂存并停止准备；测试清理不再递归进入仍挂载的映像，也不在失败展开时再次 panic。

## Windows 恢复原型范围

`scripts/test_windows_installer_recovery.ps1` 只允许一次性 Windows CI 运行器执行；检测已有安装注册及快捷方式时拒绝操作。构建两份不同版本真实 Studio 和三个测试安装器，不发布资产。

必须验证：桌面快捷方式启用/禁用两种情况、用户级中文空格路径、取消安装保持旧版、正常升级、旧安装器降级恢复、产品图标、唯一卸载身份、用户文件保存、恢复后实际导出，以及卸载后的身份清理。新版本独有文件仅按测试夹具记录的哈希清除，绝不泛化删除用户文件。

结果由 CI 生成 `windows-update-recovery-prototype` 证据附件。提交 `90657ec` 四项 CI 全通过；运行 `36877329229` 的 results.json 已读取，桌面快捷方式启用/禁用两例全部断言通过，实际版本顺序 rc.2 → rc.3 → rc.2，原路径、用户级身份、取消安装、升级恢复、错误目标/图标拒绝、用户文件及卸载检查通过。恢复原型阶段完成，不等于完整应用内更新完成。Mac 未安装 PowerShell，不声称已本地运行；安装中断、恢复再次失败及真实 GUI 更新助手仍未覆盖，不开放 Windows 生产更新入口。

首轮 Windows 恢复原型失败于第一次中文路径安装后的快捷方式字符串校验；另外三项 CI 通过。安装器日志确认安装与快捷方式创建成功，已安装程序身份/哈希及注册路径已通过，但原校验未输出实际快捷方式目标，不能凭日志断言根因。现改用原生 `IShellLinkW` 读取 Unicode 目标与图标，规范化已有文件的长/短路径别名后严格比较，不调用快捷方式自动修复；添加错误目标及错误图标拒绝测试。该修复等待 Windows CI 运行，不计为已通过恢复验收。

上述首轮失败后的修复现已在 Windows CI 通过；原失败记录保留用于追溯。

提交 `bdf5c14` 四项 CI 全通过。运行 `36884824409` 的恢复附件已下载并读取；桌面图标有/无两例均确认 `native_registry_and_shortcut_discovery` 和 `portable_and_conflicting_registry_rejected` 为 true，实际版本顺序、安装/恢复哈希、原路径、用户文件、唯一卸载身份等断言全通过。这补齐原生发现的 Windows 运行证据，仍不是 Windows GUI 更新助手验收。

恢复原型待验新增项：只在一次性 CI 夹具的 Inno 定义里注入 `AfterInstall` 自退出，在真实新 exe/新增文件复制后中断；必须观察标记和新文件哈希，不能把早期取消冒充中断。随后故障旧恢复安装器拒绝启动，核验部分安装记录、用户文件与已准备恢复包哈希未变；再显式使用正常旧安装器恢复。新增文件被改动时，清理必须拒绝且保持文件哈希不变。两种桌面图标情况均重复上述步骤。

这些新增项尚未在 Windows 执行，不计为通过。Mac 没有 PowerShell；本地只完成脚本逐段审阅和差异检查，不以此代替 Inno 编译或 Windows 运行证据。`f8c5da9` CI 不包含这批后续故障测试；已等待其终态后集中提交新一批验收。完整助手仍须实现恢复失败的持久状态及停止重试，本原型不代替该实现。

上述新增故障原型现已通过：提交 `b518a8c` 四项 CI 成功，运行 `36893724170` 的 results.json 已读取；桌面图标有/无两例均确认 `interrupted_after_payload_restored`、`failed_recovery_kept_assets_and_user_data`、`modified_added_file_cleanup_rejected` 为 true，旧版恢复、用户文件、卸载及原生发现断言全通过。这里仍是原型证据，不是产品助手持久恢复状态或 Windows 实机 GUI 验收；后续只读租约批次不在该提交内。

## 后续门槛

### Windows 可信恢复资产（本地模块验收）

新增 `update_windows/assets.rs`，使用内置生产信任根验证本地缓存清单与签名；要求旧恢复安装器精确匹配已安装版本，新安装器同通道且严格更新。两包必须在退出 GUI 前均已存在，按签名清单核验平台、安装器类型、文件名、大小和流式 SHA-256，并在使用前重复校验；不允许把 latest 指针降级来获取恢复包。

本地 10 项 Windows 模块测试通过，覆盖安装绑定、路径、版本及资产签名/过期/缺失/篡改、错误产品/平台/通道/外部 URL、新旧角色交换、错误恢复版本、相同资产与准备后修改。测试签名只用隔离夹具密钥；未读取生产私钥。全目标、全功能 Clippy 通过。

这是可信资产与配对合同，不是 Windows 完整更新通过：下载旧版不可变元数据、私有缓存 ACL/锁、独立助手执行安装与健康确认仍须衔接。过期清单在准备阶段拒绝；已经准备好的离线回滚如何持久绑定信任证据，须在助手实现中单独验证，不能通过放宽新更新过期校验解决。

### Windows 进程身份与正常退出等待（组件）

`update_windows/process.rs` 使用只含查询/同步权限的原生进程句柄，核对 PID 对应进程创建时间及实际可执行文件路径，保留句柄避免等待期间 PID 复用误认。等待最长 30 秒；超时返回未退出，不获取终止权限、不强杀。新增 Windows 运行测试覆盖当前进程仍存活、错误创建时间、错误路径、零 PID 和超限等待拒绝。

真实源文件已通过 Windows x64 交叉类型检查及 Clippy，本地完整 Studio 全目标/全功能 Clippy 通过。Windows 运行测试须由包含该组件的新 CI 执行；该组件尚未接入外置助手，不计为原位安装链路完成。`bdf5c14` 的绿色 CI 不包含这部分后续进程组件。

提交 `f8c5da9` 的 Windows Quality 已通过；日志明确记录 `current_process_is_bound_and_never_forced_to_exit ... ok`，安装器恢复原型同样 PASS。macOS Quality 与 windows-audit 成功；Linux 在安装系统依赖时约 30 分钟后取消，尚未执行 Rust 测试，不是可合并的全绿结果。该轮不包含后续权限及安装中断测试，不能外推其通过。

先读取 Windows CI 真实恢复结果并修复范围内失败；再完成缺失的平台交互与安全边界测试。代码测试通过不自动授权合并、版本变更、Release、OSS 或本机用户应用替换。后续发布需另行批准。

### Windows 私有缓存及事务权限（实现，运行待验）

新增 `update_windows/cache.rs`：原生创建时指定当前用户 SID 为所有者、保护 DACL、仅该用户可访问；目录对子项继承权限。已有普通/外来目录拒绝，不改其 ACL。文件也核验所有者和授权，创建时显式指定所有者，避免直接依赖进程 token 的默认所有者。元数据使用私有新文件写入、同步后同目录替换；失败保留临时证据，不修补已有目标权限。

Windows `TransactionStore` 已强制私有目录/锁文件/状态文件权限，生产恢复资产读取同样检查目录、清单、签名和安装器文件权限。测试夹具改用同一安全创建 API，不删掉检查来迁就默认权限。新增 Windows 测试覆盖重复创建、普通目录拒绝、新文件/原子元数据替换、文件与目录类型错配。

Windows API、进程模块及实际事务/交换源文件的隔离交叉类型/Clippy 检查通过；检查器的路径适配器固定拒绝，不能当路径或权限的运行测试。Mac 全目标/全功能 Clippy 与 18 项相关本地测试通过。恢复资产克隆保留权限策略，使用前重复验证目录/文件权限。这批权限实现仍待新 Windows CI 实际执行；不能计为 Windows 更新助手端到端通过，原位入口保持默认关闭。权限及恢复原型故障批次进入下一轮 CI。

该权限批次现已在 `b518a8c` Windows CI 通过：日志确认 `private_directory_is_created_atomically_and_foreign_acl_is_not_repaired` 和 `production_private_acl_requirement_is_retained_by_clones` 通过。此前“待运行”记录保留为阶段历史，不再是这两项的当前状态。

### 安装器使用期间的只读租约（实现，Windows 运行待验）

`VerifiedWindowsInstaller::pin` 在路径/权限检查后以仅 `FILE_SHARE_READ` 打开安装器，在句柄持有期间重新校验大小与 SHA-256；新旧配对均须取得租约后才允许请求旧 GUI 退出。任一获取失败会释放另一句柄，安装不开始。租约不序列化、不克隆，也不代替安装事务锁；不得提前释放后仅凭缓存描述执行安装。

新增 Windows 测试覆盖持有期间读取允许、写入/删除/改名拒绝、已有写句柄拒绝、释放后篡改再次获取拒绝。恢复原型在正常、取消、中断和恢复安装器运行期间都持有同等只读共享句柄，以实际验证 Inno 兼容性；这些新增测试尚待 CI，不计为通过。

Mac 10 项安装契约/资产测试及全目标全功能 Clippy 通过。隔离 Windows 检查器现包含实际签名清单、完整安装契约/原生发现/资产/权限/进程与事务源文件，Windows x64 全目标 Clippy 通过，不再使用拒绝型路径适配器；它仍只是交叉类型检查，不是 Windows 运行或 GUI 验收。

本轮本机 Studio `cargo test --locked --package instplot-studio --all-targets --all-features` 完整通过：335 项通过，1 项原有跳过，包括真实 DMG 组件及导入/录入、坐标轴、文字、持久化和导出回归。Windows 专有租约/ACL 用例不在此 Mac 数字内。

补充 4 项工作保护控制器测试通过：项目无 dirty 不隐藏录入草稿，数值/文字/工具草稿保留，写盘失败保持重启待确认且旧应用不关闭，执行重启动作时重新检查草稿及外部数据保存冲突。后两项新增；这些是控制器断言，未冒充真实保存对话框点击、取消或完整下载/重启 GUI 验收。

### Windows 安装访问锁与实际执行适配器（实现，运行待验）

`update_windows/access.rs` 为安装目录提供生命周期共享锁，外置助手需要同一安装的排他锁。生产锁根固定使用系统 LocalAppData 下的 Studio 私有目录，调用方不能选择第二个根绕开实例锁；以规范化 Unicode 安装路径生成锁身份。不同安装互不混用。仅 Windows `in-place-update-preview` GUI 启动接入共享锁，每个同目录实例都须持有，不能在获取失败后以无锁预览继续运行。默认构建未增加此启动要求，便携/不支持安装不能据此启动自动安装。

`update_windows/runner.rs` 实际通过原生 `Command` 执行受保护的安装器，要求对应安装排他锁、精确绑定的旧进程已经退出；新版本安装前复核当前用户注册、路径、版本与快捷方式选择。只接受同版本恢复或同通道更新，使用既定参数，不运行 shell、不提权、不强关、不自行启动应用。新日志须在私有目录中独占创建。运行对象保留包租约并借用排他锁；轮询未退出不能进入启动或恢复，超时必须保留对象和事务，不终止安装器。安装器退出后锁仍由助手持有，以继续核验/恢复。

新增 Windows 用例覆盖多实例释放前排他拒绝、错误安装锁拒绝、不同安装独立，以及正常关闭绑定子进程后才允许执行、执行期间/结束后锁保持。执行用例使用签名隔离测试 harness，不是真实 Inno，也不运行 GUI；实际 Inno 兼容性仍由恢复原型证明。上述完整 Windows 模块交叉 Clippy 已通过，运行测试待新 CI。GUI 启动接入须由 Windows 全量构建确认。

这只是安装访问与执行适配器接入；Windows 外置助手的持久请求/状态机、单次新版启动、首画布健康、恢复失败处理及真实 GUI 验收仍未完成。入口继续关闭，不计为整体自动更新通过。

提交 `4113a2c` 四项 CI 全通过，运行 `36896046217` 的附件已读取，两种桌面任务均确认 `installer_read_lease_execution=true`；原生租约的读允许、写入/删除/改名拒绝与已有写句柄拒绝测试通过。真实 Inno 正常安装、中断与恢复在持有租约时运行通过，原恢复断言仍全通过。

提交 `2dabb7b` 四项 CI 全通过；已读取该提交的 Windows Quality 运行 `36899505866` 日志，`all_instances_must_release_shared_access_before_installation`、`installer_test_child_waits_for_normal_parent_pipe_close`、`native_runner_requires_normal_exit_and_retains_exclusive_access` 均通过，实际 Inno 恢复原型同轮 PASS。这证明访问锁、正常退出与受限执行组件在 Windows 运行，不是完整外置助手或实机 GUI 更新通过。macOS 本轮完整 Studio 回归 337 项通过、1 项原有跳过，Windows 专属用例不计入本机数字。

后续下载权限接入：Windows 下载根改由同一固定私有根创建，临时安装包、原清单及签名都使用显式 owner-only 文件创建接口；不修旧/外来目录权限，不清旧缓存。下载后硬链接提交仍不覆盖目标，并新增文件/证据权限与拒绝覆盖断言，待下一轮 Windows CI。

该下载权限批次本机完整 Studio 全目标/全 feature 回归 337 项通过、1 项原有跳过；全工作区 Clippy `-D warnings`、格式与差异空白检查通过。实际 Windows 原生模块交叉 Clippy 通过；Windows GUI 下载权限用例尚未在 Windows 执行，不纳入本机测试通过数。

macOS 下载/DMG/用户重启完整链路的独立测试环境已构建：仅忽略目录中的源码快照使用专用公钥、本地 HTTPS CA、隔离缓存及 QA bundle ID；不修改产品信任根或系统证书，不用生产私钥/OSS。两个真实 GUI 版本和签名 DMG 已准备，旧 QA GUI 已启动，但本机锁屏使桌面操作被明确拒绝，尚未点击更新或执行替换；待用户解锁，不能计为完整 GUI 链路通过。

### 隔离 HTTPS 下载到真实 DMG 暂存（已通过，非 GUI 重启）

在忽略目录 `target/macos-update-chain.k5aYn3` 的专用源码快照中运行两个定向测试：

- `isolated_https_signed_download_chain`：默认 CA 拒绝测试站点；仅信任夹具 CA 的真实 HTTPS 请求成功，原清单及 Ed25519 签名通过验证，完整 DMG 流式大小/哈希校验与私有缓存证据保存通过。拒绝重复覆盖，错误哈希及提前取消不留下可用的最终包。
- `isolated_downloaded_dmg_staging_chain`：重新验签该网络下载的缓存清单，核对 DMG 大小/哈希，然后实际只读挂载、静态签名核验、复制到含中文空格路径、正常卸载，再运行本地候选的版本/产品/协议探针；候选二进制哈希与预期一致。原 QA 应用二进制及保存项目哈希未变。

两项定向测试均通过，证据保留在该夹具的 `signed-network-DMG 中文 */component-evidence.json`。专用 CA/公钥仅存在于测试快照，没有关闭 TLS 校验、导入系统证书、读取生产私钥、清除 quarantine、替换用户应用或发布资产。该证据补齐真实网络下载至真实候选暂存的组件串联，但没有点击用户更新/重启按钮，没有应用交换或 GUI 健康确认；不能称完整平台更新验收通过。Windows 下载权限提交 `ce26b7e` 已推送 PR #9，新 CI 结果另行低频核验。

### Windows 安装执行持久检查点（实现，Windows 运行待验）

前置下载权限提交 `ce26b7e` 四项 CI 已通过；已读取运行 `36903442800` 的 Windows job `110507978874` 日志，`signed_cache_evidence_is_owner_only_and_never_overwrites` 通过，真实 Inno 恢复原型同轮 PASS。它不包含下面的持久检查点批次，不能外推该批通过。

受限原生执行器现在必须持有 `TransactionStore`，核对已持久化事务的 ID、nonce、目标、阶段及资产角色后，独占创建并同步 `apply-installer.json` 或 `restore-installer.json` 的执行意图，再启动安装器。已有或部分记录拒绝重放，不删除或覆盖。实际 Child 句柄提供创建时间，避免 PID 查询复用；启动后记录 PID/创建时间，正常退出后同步退出码才向控制器返回退出。

启动后的检查点写盘失败保留 Child、包租约和安装排他锁，不因错误释放正在执行的安装器；轮询错误与超时均要求保留对象。只有明确未启动的前置/创建进程失败可记录 NotStarted。Intent 无 PID 是“不确定”，不是“没有运行”；Running 记录也不是当前存活/已退出证据，重启后的助手仍须原生绑定和等待，无法确认时拒绝自动重放或恢复。退出码零不代表安装身份、首画布健康或完整更新成功。

原生只读检查接口可在持有安装排他锁和安装器租约时刷新 Running 记录：精确绑定 PID、创建时间及安装器路径，先确认进程句柄已发出退出信号，再读取并持久化退出码。进程不可绑定（包括已消失、复用或不可访问）则报错，不推测退出。无 PID 的 Intent 仍不确定。该接口不启动安装器、恢复或 GUI，也不把退出码当健康证明。

新增用例覆盖缺失/零 PID、不完整退出记录、正常 Child 的持久 Running/Exited、退出检查点写盘失败时保留租约与锁、重复执行拒绝、模拟缺失 PID 检查点拒绝重放、跨事务及未知字段拒绝、真实存活隔离进程刷新与错误创建时间拒绝。本机实际 Windows 源文件交叉 Clippy 与工作区 Clippy 已通过；Windows 运行结果待该批提交 CI，不能借用前一提交证据。这仍不是完整 Windows 外置助手：可信旧资产取得/持久准备、检查后恢复决策、单次候选启动、健康确认及恢复状态机继续实施，默认入口保持关闭。

本批本机定向测试：共用事务 4 项及 Windows 安装契约/资产的跨平台 10 项均通过；格式、差异空白及仓库卫生通过。Windows 专有进程/写盘故障/持久检查点用例不在此本机测试数内。

### Windows 当前版本恢复元数据留存（实现，Windows 运行待验）

前一持久检查点批次提交 `7edd732` 四项 CI 全通过；已读取运行 `36906912505` 的 Windows job `110519645432` 日志，`uncertain_or_malformed_attempt_is_never_an_exit_signal` 与扩展后的 `native_runner_requires_normal_exit_and_retains_exclusive_access` 均通过，真实 Inno 恢复原型同轮 PASS。后者包含检查点写盘失败、租约/锁保持及精确进程刷新断言，但执行的是隔离测试 harness；不代表完整原生助手或 Windows GUI 更新通过。

增加不可反序列化的签名安装器描述能力，与“已下载并验证的安装器”分开。安装器原有清单签名、产品、平台、版本、类型、文件名及大小校验复用该描述解析，文件权限、大小/哈希和租约仍单独必需；只有描述而没有包不会成为可执行资产。本机 11 项安装契约/资产测试通过，新增缺包与坏签名断言。

仅 Windows 预览构建在正常 latest 验签、水位检查且远端版本等于当前版本后，核对真实用户级安装身份，保存该版本的原清单/签名到固定 owner-only 下载根下的 `recovery-metadata`。按版本散列及清单散列寻址，清单/签名不可覆盖；单写者锁及最后原子更新的小指针保证不会发布部分证据。同版本序列回退、同序列不同内容、改动证据、外来 ACL 或坏指针拒绝，不修补权限、不清历史证据。默认构建不增加这一步，Mac/Linux 不受影响。

准备阶段读取只针对当前原生注册版本，重新验证签名、有效期、精确版本和签名序列与指针一致性；不修改普通更新的序列水位，不猜旧版 OSS 元数据序列，不扫描服务器历史发布。新增 Windows 用例覆盖元数据幂等留存、并发写者拒绝、指针写盘失败仍保持旧记录、序列/内容冲突、坏摘要、缺失/过期、路径注入及外来权限拒绝。实际 Windows 源文件交叉 Clippy、本机工作区 Clippy/卫生通过；这些 Windows 专属运行用例待下一提交 CI。

边界仍明确：这里只保留元数据，没有提前下载或准备旧安装器，不能计为恢复包或完整更新就绪。第一次使用该功能若没有已验证的当前版本清单，或留存清单已经过期，自动准备继续拒绝，旧 GUI 保持运行；不能以旧版本号猜地址、降低水位或跳过有效期来补齐。未部署新代码的旧客户端仍可能需要一次正常安装来引入助手；后续取得两个实际安装包、可信持久准备请求及完整助手决策仍继续实施，未授权安装、发布或修改 OSS。

本批本机 Studio 全目标/全 feature 回归 338 项通过、1 项原有跳过；工作区 Clippy `-D warnings`、Windows 实际模块及测试的交叉 Clippy、格式与差异空白检查通过。Windows 元数据留存专属测试只完成类型检查，等待新提交的 Windows 运行证据，不能使用前一批绿色结果代替。

### Windows 新旧安装包准备衔接（实现，Windows 运行待验）

`prepare_installers` 先原生发现当前用户安装及同通道新版，验证已下载候选并持有只读租约；只有精确的已留存旧版清单可用于恢复包定位。复用普通严格 HTTPS/重定向白名单/大小/哈希/取消下载函数，不覆盖现有包，不重复写不可变清单。完成后对新旧缓存重新验签与有效期检查，核验实际文件、版本角色并同时持有租约，复验安装身份/快捷方式选择。准备前后用随机独占文件检查目标目录可写，同步后只删除本次创建的探针；失败继续阻止准备，不修改项目或应用文件。

仅 Windows 预览构建接入“准备可信恢复包（技术验证）”与进度/取消状态；后台准备期间仍可编辑，并阻止另起检查覆盖工作状态。缺包、坏缓存、过期或取消都不请求退出/重启。成功只展示包准备结果并保持租约，结束准备保留缓存；没有安装或重启按钮。该对象没有持久化助手请求或就绪回执，不能作为退出旧 GUI 的依据，也不代表安装、首画布或恢复串联完成。默认构建不增加此入口，Windows 更新协议仍未开放。

新增 Windows 用例覆盖准备取消前不下载、下载后取消不报告就绪、已有坏包不被覆盖（之后必须验证拒绝）、中文空格目录写探针不改变用户文件且不残留；预览控制器另测准备时保持编辑与阻止重复检查、准备失败不请求退出。实际 Windows 模块及测试交叉 Clippy、本机工作区 Clippy 通过；Windows 专属运行及预览 GUI 全量构建仍待该批 CI，不称实机 GUI 通过。

本批本机 Studio 全目标/全 feature 回归仍为 338 项通过、1 项原有跳过；Windows 专有准备与 UI 状态测试不在本机通过数内。macOS 下载/重启实机链路仍待解锁，Windows 真实 GUI 环境仍未提供；继续完成安全实现，不以这些环境门槛虚报整体结束。

### Windows 外置助手请求与就绪能力（实现，运行待验）

前置元数据/包准备提交 `f10648d` 四项 CI 已全通过。已读取运行 `36914050168` 的 Windows job `110543531569` 日志：三项元数据留存、三项准备组件及两项预览控制器用例均通过；该轮恢复附件 `results.json` 的桌面 on/off 两例无 false 断言，版本未改，真实 Inno 恢复原型通过。它不包含以下新增助手请求批次，不能外推这些新用例通过。

新增 `helper_request.rs`：仅从原生确认的正在运行旧应用复制助手到固定 owner-only 事务根下的随机事务目录，独占创建、同步并核对原二进制哈希，保持助手及新旧安装包的只读租约。请求记录精确安装位置、版本、桌面任务、旧 PID/创建时间、原二进制哈希、新旧原清单哈希与安装包哈希/大小；不接受任意命令，结构拒绝未知字段。创建请求仍不会退出、启动助手或执行安装。

进入等待态前重新验证原生身份、旧进程、助手副本、两份签名证据与安装包；只有精确匹配的 Prepared 事务可转 WaitingForExit，重复或跨事务请求拒绝。该接口留给工作保护后的重启控制器，目前未接入 GUI 重启入口。

助手读取端仅接受固定事务目录中的实际运行副本和对应源版本；锁定同一事务，要求 WaitingForExit，重新验签两份缓存并持有租约、绑定仍存活的旧进程及真实注册身份。就绪回执独占创建，绑定事务 ID、nonce、实际助手 PID 与原生创建时间。父端须以自己持有的 Child 句柄核对实际路径/存活/出生时间和回执，再复验安装身份；回执不是安装授权、健康确认或完成记录。

新增 Windows 用例覆盖跨身份/nonce/阶段/模式拒绝、未知字段拒绝、就绪不等于健康、助手私有字节复制/持有期间写删拒绝/拒绝覆盖，以及复制真实测试 harness 后的存活子进程就绪、错误创建时间/nonce和已退出子进程遗留回执拒绝。测试 harness 不是真实 GUI 或完整助手。实际 Windows 模块及测试交叉 Clippy、工作区 Clippy 已通过；新用例等待 CI 运行，不能列为已通过。

尚未接入 Windows `--apply-update`/健康 GUI 启动、持久单次候选启动与失败恢复控制器，更新协议仍为 0；GUI 也没有安装/重启按钮。安装状态/新增发布文件记录、配置保护、进程退出后的执行/健康/恢复串联及真实 Inno 原生执行器集成证据继续实施。加载等待请求采用新鲜有效的签名清单；不通过回拨验证时间来恢复过期或不确定事务，缺乏可信证明时继续保留证据并阻止自动执行。

### Rust 原生执行器与真实 Inno 集成（实现，Windows 运行待验）

新增显式忽略的 Windows 专有测试，仅允许 GitHub Actions Windows 一次性 runner 加专用 opt-in 环境运行，所有安装/安装器路径限定于 RUNNER_TEMP。既有恢复原型对桌面快捷方式 on/off 两例分别调用；测试使用仅 cfg(test) 的签名密钥与私有 ACL 缓存，不读取生产私钥或修改生产信任配置。

真实旧 Studio CLI 进程以原生句柄绑定并正常退出，新旧安装器提前取得租约。实际 Rust 执行器在持久事务与目标排他锁下直接调用真实新 Inno，再调用旧 Inno 恢复，逐次核验原生注册/快捷方式、CLI 版本、同目录身份、原二进制哈希及用户项目哨兵哈希。已有原型负责按已知哈希清理本次 QA 安装器新增的测试文件，不扩大到用户文件或恢复备份。

安装运行对象在检查点错误时仍保持租约和锁，不强制终止安装器；一次性 runner 的工作流超时是此集成测试的外部终止界限，不冒充生产助手的超时恢复策略。成功后分别写 `desktop-on/off-native-runner.json` 和 apply/restore 日志，由 PowerShell 再核对回执字段，防止测试筛选错误却将零项测试当作成功。

本机工作区 Clippy、Windows 实际模块和测试交叉 Clippy、格式及共用事务 4 项测试通过。尚无本批 Windows 运行证据，不能借用此前 PowerShell 原型成功来证明 Rust/Inno 集成已通过。没有候选 GUI 单次启动、健康回执或自动恢复控制器，因此事务不标 Completed 或 GUI 回滚通过；完整外置助手仍继续实现，默认入口保持关闭。

本批 Studio 全目标/全 feature 回归 338 项通过、1 项原有跳过，仓库卫生通过。新增真实 Inno 用例只在 Windows 显式执行，不包含在上述本机运行数量中；本机未安装 PowerShell，脚本新增段运行/解析由 Windows CI 验证。

### Windows 单次候选启动与健康见证（实现，Windows 运行待验）

前一助手请求批次 `35a5dbb` 四项 CI 全通过；运行 `36917760953` 的 Windows job `110555867560` 日志已确认四项 `helper_request` 用例通过，包括实际复制 harness 的原生子进程就绪校验。已读取同轮恢复附件 `results.json`：桌面 on/off 两例全部断言通过，生产版本未改。此证据不包含 `a6ff826` 的 Rust/Inno 集成，也不包含以下新组件。

`candidate.rs` 为未来助手控制器提供单次候选启动预留与健康见证，不自行启动 GUI。预留要求精确持久 Applying 事务、安装器持久退出码 0、仍受租约保护的候选包、目标排他锁、原生新版安装/桌面任务身份；独占创建并同步 `candidate-launch.json` 后才允许控制器另行启动。已有、部分或不确定记录一律拒绝第二次预留，不把缺 PID 当作未启动。

绑定仅接受控制器实际持有的 Child，核对原生路径/出生时间和存活，保留原生句柄后记录并持久转 AwaitingHealth。检查点写盘失败必须由控制器保留 Child 与锁/租约；组件不强杀、不自动重试。健康确认要求精确记录、完整身份/nonce、原生 PID/出生时间、初始化与首窗口回执及仍存活的候选；只有 Completed 写盘成功才返回通过。恢复前另可查询确切绑定的候选是否退出，缺见证或记录不匹配继续拒绝；退出本身仍不等于允许恢复，还需要目标排他锁和安装/文件恢复契约。

新增 Windows 用例覆盖单次预留/中断记录拒绝、未持久及跨事务拒绝、未知字段/资产身份篡改、实际存活测试 Child、错误 nonce/PID/出生时间/初始化/首窗口、候选提前退出，以及健康提交故障不报告成功、匹配见证可持久提交。回执仅在隔离单元测试中由夹具写入，不是 Studio GUI 产生的真实首画布证据。本机 Windows 实际模块及测试交叉 Clippy、工作区 Clippy 通过；专有运行仍待新提交 CI。

边界不变：Windows 产品/协议探针、发布文件/配置保护、真实单次 GUI 启动、候选正常退出请求、健康失败恢复及旧版启动控制器尚未完整串联，协议仍为 0，安装/重启入口仍未开放。macOS 真实按钮链路继续等待桌面解锁；不将此组件或 CI harness 算作整项验收完成。

### 当前 Windows 固定发布文件的交接/恢复核对（实现，运行待验）

现有生产 Inno 的 `[Files]` 仅发布 `instplot-studio.exe` 与 `LICENSE`，没有可泛化删除的文件树。原版 exe 哈希沿用实际旧程序复制证据；助手请求新增原版 LICENSE 哈希并要求匹配运行旧构建内嵌文本。许可文件的路径、类型、精确大小、内容哈希都核对；缺失、变更（包括同长度）、目录或重定向均拒绝，不自动修复或覆盖。准备、进入等待、助手加载/就绪、父进程确认就绪均复验两个原版文件。

新增恢复后核对接口要求精确 Restoring 事务、原目标排他锁、绑定旧进程已退出、仍可信的旧恢复包及持久安装器退出码 0；再核对实际旧版注册/快捷方式任务、原版 exe 和 LICENSE 哈希。接口本身不执行安装器、不清理新增文件、不恢复配置、不启动 GUI，也不将事务标记 RolledBack。未知/用户文件从不纳入删除范围。

新增专有用例覆盖缺 LICENSE 证据/错误证据请求拒绝、原文件通过、缺失/类型/大小/同长度内容变化拒绝、错误恢复 exe 拒绝及用户项目哨兵不变。包装契约测试精确约束当前两个 Inno 发布文件；未来增加发布文件必须补齐记录/恢复及协议兼容，不能以这轮固定集合测试宣称任意未来包可完整恢复。当前请求格式尚未发布、无安装入口；缺新增字段的旧私有请求拒绝而不修补。

本机 Windows 实际模块与测试交叉 Clippy、工作区全目标全 feature Clippy、11 项跨平台安装/资产回归、格式和卫生通过。新增 Windows 专有运行尚待该批 CI。配置保护、候选协议/发布文件完整性探针、真实 GUI 健康失败恢复及旧版重启仍继续实施，默认入口保持关闭。

### Rust/Inno 路径兼容失败与修复（Windows 重验待运行）

`a6ff826` 的 Quality 运行 `36919485214` 中 Linux/macOS 成功，Windows job `110561655056` 失败；尚不是全绿。实际安装器日志 `desktop-on-native-apply.log` 已读取：Rust 传入的 `/DIR=\\?\D:\…` 被 Inno 6.7.1 识别为包含非法 `?`，退出码 3，未进入新版复制。这不是安装成功或恢复通过；后续提交继承同一问题，需要新的运行证据。

修复仅在进程参数边界将本地盘 canonical/verbatim 路径转换为 DOS 写法，目标身份、记录、锁及缓存继续使用原 canonical 路径。转换前拒绝网络/设备、遍历、保留设备名称、非法字符及尾点/空格等有歧义写法；转换后以实际存在的文件/目录或新日志的已存在父目录做 canonical 同目标核对。参数仍逐项传给 Command，不用 shell、不自行追加转义引号。

新增 Windows 用例覆盖中文空格路径、已有/未创建日志、canonical 目标不变和歧义写法拒绝；原目录参数断言同步为 DOS 写法，实际 exe 身份仍断言 canonical。不删集成测试、不放宽私有权限或签名/租约。Windows 实际源文件及测试交叉 Clippy、本地 11 项安装/资产回归已通过；真实 Inno 升级/恢复仍需修复后 CI 重跑。
# Windows 候选端健康回执能力补充（组件实现，GUI 未接入）

候选启动记录新增实际 exe 的 SHA-256 与只读文件租约；助手保持该租约至候选原生进程退出，退出后才允许显式释放，以便后续恢复安装。候选端读取固定私有事务目录，复核安装注册身份、运行版本、exe 哈希、LICENSE、事务 ID/nonce 与实际 PID/创建时间。只读事务快照不竞争助手持有的写锁，仍执行权限、结构、身份及 64 KiB 上限检查。

候选端接口只允许在调用方确认首个真实画布后使用：等待助手持久绑定进程后独占写健康回执，仍等待助手持久提交 Completed；不自行宣布更新成功。未绑定超过 5 秒、未提交超过 75 秒、身份变化、既有回执均拒绝；私有停止请求只返回正常退出意图，不强杀进程。新增测试覆盖这些组件边界及存活期间拒绝释放 exe 租约。Windows 运行证据须由后续 CI 提供，不能以交叉编译代替。

本批本地 Studio 全特性库回归 217 通过、1 忽略；共用事务 4 项通过；工作区全目标全特性 Clippy 与 Windows 实际模块/测试交叉 Clippy 通过。Windows 专属测试未在本机运行，待 CI。

本批没有接入 Windows GUI/CLI 健康启动或完整助手控制器，Windows 更新协议仍为 0，默认入口保持关闭。实际配置写入保护、单次 GUI 启动、项目恢复及失败恢复端到端验收仍未完成；上述接口不构成完整自动更新验收。
# 真实 Rust/Inno 重验与普通窗口启动保护

修复提交 `eba96fb` 四项 CI 通过。已核验运行 `36923218593` 的 Windows 恢复附件（artifact `11193502818`）：`desktop-on-native-runner.json`、`desktop-off-native-runner.json` 均记录真实 Rust 执行器升级及恢复成功、用户数据不变；`results.json` 两例均包含 `rust_native_inno_apply_and_restore=true`，其他恢复/快捷方式/卸载断言为 true，生产版本未改。附件范围明确为安装器恢复原型，不是 GUI 自动更新；新健康握手提交 `d4223b5` 尚需独立 CI 证据。

Windows 预览普通 GUI 在持有目标共享锁后检查固定私有事务根：该目标处于 WaitingForExit、Applying、AwaitingHealth、RecoveryRequired 或 Restoring 时拒绝普通启动；Prepared 和严格终态不阻止启动，其他安装目标的有效记录不互相阻挡。目录身份、私有权限、完整事务校验失败不静默略过；最多检查 4096 条保留记录，超限要求人工复核，不删除恢复证据。共享锁继续阻止安装器与普通启动竞态。新增 Windows 测试覆盖等待/应用/健康等待/恢复、无记录、Prepared、RolledBack、另一目标及损坏记录；实际运行待 CI，未接候选专用 GUI 入口。以后候选入口必须严格加载事务并独立获得共享锁，不能普遍绕过此保护。
# Windows 保存项目重启交接（组件衔接，待运行验收）

新增 `WindowsResumeProject`：只绑定已存在的规范路径主项目文件，记录 SHA-256/大小，流式限额 128 MiB，拒绝安装目录内项目、重定向路径、变更/损坏及备份替代；校验项目能重新构建图形。外部源非致命警告不否定项目内保留数据，实际无法构图仍拒绝。不保存、修改、迁移或修复用户项目。

Prepared 助手增加 `set_resume_project`，契约为工作保护完成并冻结编辑之后、进入 WaitingForExit 之前调用；保持只读租约并原子保存绑定，超大请求拒绝。助手读取和旧进程等待前重新核验，助手会话继续持有租约。候选单次启动记录包含同一绑定，候选加载先复核、持有租约并提供精确重开路径；记录变化仍拒绝。未来控制器必须传递助手会话的绑定，GUI 必须打开该主项目后才能调用首画布回执，不能用一个空白/示例画布代替工作恢复。

新增 Windows 测试覆盖中文空格路径、持有期间写删拒绝、错误哈希、安装目录内项目拒绝、有有效备份时损坏主项目仍拒绝、项目内容不变，以及候选启动记录绑定/变更拒绝。实际 Windows 运行待 CI。macOS 本地全特性库及二进制回归通过（库 217 通过/1 忽略，二进制 113 通过）；工作区全目标全特性 Clippy 通过。扩大后的隔离检查器使用完整实际 Studio 库源及测试完成 Windows 交叉 Clippy；不包含 HTTP TLS 依赖/主 GUI 二进制，不代表完整 Windows 构建或运行。直接工作区 Windows 交叉构建因本机缺 MSVC SDK 的 `assert.h` 在 ring 构建处受限，未绕过或削弱生产依赖。

完整 Windows 助手控制器、GUI 项目重开/健康接口和故障恢复仍未接通；协议仍为 0，默认入口关闭，未合并或发布。
# 预览候选 GUI 与安装前中止衔接

`d4223b5` 质量运行 `36925795400` 成功，已读取 Windows job `110582670415` 的六项候选组件测试通过记录（含原生绑定/助手提交等待、超时不写回执、错误健康与单次预留）；它不是真实 GUI。`7fb71db` 运行 `36926547438` 成功，已读取 job `110585180242` 的普通启动保护测试通过记录。当前保存项目批次及以下新增衔接须重新验收。

Windows 仅 `in-place-update-preview` 构建增加内部 `--update-health <事务目录>` GUI 路由；加载端严格复核原生安装/进程与私有事务，随后获得同目标共享锁。仅已加载的候选健康启动可跳过普通窗口的未完成事务检查；编辑、快捷键和拖入仍被首画布阶段门拦截，Pending 每 100 ms 请求重画，只有助手持久 Completed 才放行，停止/错误走正常关闭。绑定项目在首画布前实际重开，并核对 workspace 的主项目路径；重开失败或备用文件不能用示例画布提交健康。主项目打开确认同时补到现有 macOS 健康入口。未启用 Windows 重启按钮、`--apply-update` 或协议 1，普通默认构建不接受新的 Windows 健康参数。

安装前中止新增助手接口：必须仍为 WaitingForExit、没有 apply/restore 安装器意图或候选启动记录，且原生旧安装及固定原版文件复核通过，才能持久 FailedBeforeApply（不是 Completed/RolledBack）。任一不确定执行证据存在时保留原状态/证据，不冒称取消成功；原因限额与 nonce 脱敏沿用共用事务。普通窗口允许严格 FailedBeforeApply 终态，修正早期启动保护遗漏该终态的问题。新增测试涵盖原版核验失败、部分执行记录、重复中止、Applying 中止拒绝与安全终态启动。

本地启动参数两项通过，Studio 全特性库回归 217 通过/1 忽略、GUI binary 114 通过，工作区全目标全特性 Clippy 与实际 Windows 库源/测试交叉 Clippy 通过。Windows 新 GUI 二进制编译/测试待 required CI；未进行 Windows 实机 GUI 更新或本机锁屏绕过。完整助手执行、单次实际候选启动、失败恢复和剩余 macOS 真实按钮验收仍未完成，不开放默认入口、不合并或发布。

# Windows 路径边界修复与独占候选启动

`45cae5b` 的 Windows quality 运行 `36929863421` 因保存项目库测试失败；audit 运行 `36929863534` 的 formal-workspace-tests 亦失败，摘要没有单项断言，不借此宣称所有 audit 故障已解决；macOS/Linux quality 通过。quality 失败断言为安装目录内项目必须拒绝：项目路径 canonicalize 后带 Windows verbatim 前缀，调用者给出的安装目录仍为 DOS 写法，直接 starts_with 错误放行。capture/pin 现在均拒绝重定向安装路径并规范化安装目录后比较路径组件，补充 DOS/canonical 两种写法及 pin 拒绝测试。候选项目测试建立真实安装目录，不削弱生产要求。修复后的 Windows 运行证据仍待新 CI。

候选启动新增消耗单次预留及排他锁的 owned-process 接口：仅核验安装器成功与原生安装后的公开预留可启动；固定健康参数，不使用 shell。释放排他锁后由已认证候选取得共享锁，普通窗口仍被活动事务挡住。CreateProcess 成功后即使绑定或写盘失败也保留 Child 和租约，不重试、不强杀；失败/退出不算健康。恢复前必须先取得真实 owned Child 退出证据并持久写入，或记录确定的 CreateProcess 失败，再释放二进制租约；损坏记录拒绝，不据此自动重放或宣布恢复成功。

新增原生测试覆盖真实启动失败、真实测试进程提前退出和注入启动后检查点失败；测试进程不是 Studio GUI。实际 Windows 库及测试交叉 Clippy、工作区全目标全特性 Clippy 已通过；Windows 运行及完整助手/恢复 GUI 链仍待验收。默认入口关闭、协议 0，不合并、不发布、不替换 Spotlight、不清理恢复证据。

## 安装前最后复核状态门（本地实现，Windows 运行待验）

`WindowsHelperSession::enter_applying` 必须持有精确安装目录的排他锁，原生旧进程已正常退出；请求原文与已加载请求一致，helper-ready 精确绑定当前助手 PID/创建时间、事务 ID/nonce。再次验证注册安装身份、原版 exe/LICENSE、助手复制体、新旧签名包绑定/有效性及主项目只读绑定后，才由 WaitingForExit 持久切换 Applying。任一错误不切换、不安装、不强杀、不启动窗口。切换本身不是安装成功，更不是健康或恢复成功。

新鲜执行与安装前中止共用已有 apply/restore/candidate 意图检查，部分记录也拒绝，不删除记录获取重试资格。新增 Windows 组件测试覆盖 Prepared/外来 nonce 拒绝、复核错误保持 WaitingForExit、三类部分意图保持原文、合法一次状态切换、重复切换及应用后中止拒绝；不运行安装器，不冒称真实 helper/GUI 链通过。实际 Windows 全库源/测试交叉 Clippy、工作区全目标全特性 Clippy 通过，Windows 原生运行待 CI。完整助手控制器与配置/恢复保护尚未完成；协议和默认入口不变。

## 候选退出到恢复状态门（组件衔接，真实恢复 GUI 未完成）

`enter_restoring_after_candidate` 只接受当前助手保留的 owned 候选对象，不接受单独从磁盘推断退出：同一事务目录/当前持久阶段、真实 Child 的 PID/创建时间/退出码与私有退出记录完全一致，或该对象确证 CreateProcess 未启动且对应记录完全一致；exe 租约必须已在持久退出证据后释放。Applying/AwaitingHealth/RecoveryRequired 以外的状态拒绝，成功完成态不得回退。丢失、修改、未知字段、部分证据或未持久的阶段均拒绝。

随后还必须持有目标排他锁、原版进程确证退出、新版安装器持久退出码 0、旧恢复包仍符合原请求与信任/有效性要求，且没有既有 restore-installer/recovery-launch 意图，才持久写 RecoveryRequired 再写 Restoring；第一次写成功、第二次失败不伪装为终态。该方法不安装旧包、不重开 GUI、不标 RolledBack，也不授权从磁盘重建丢失的句柄。

扩展真实测试 Child 的组件测试覆盖释放前拒绝、已知未启动与退出证据篡改拒绝、其他事务目录拒绝、未持久阶段拒绝、恢复执行阶段拒绝；进程仍是测试程序，不是 Studio GUI。实际 Windows 库/测试交叉 Clippy 和工作区全目标全特性 Clippy 通过，Windows 原生运行待下一批 CI；完整控制器、原版 GUI 健康及恢复项目可用性仍未完成。入口保持关闭，不发布或清理恢复证据。

## 恢复健康回执共用规则（接口完成，平台调用链待衔接）

新增 `UpdateTransaction::accept_recovery_health`：只在 Restoring 接受旧版本、精确产品/平台/目录/事务 ID/nonce、恢复进程 PID/创建身份及初始化/画布就绪一致的回执，再在内存进入 RolledBack。调用方须先以保留的原生存活句柄和独立恢复启动记录验证真实恢复 GUI，并负责持久写盘；接口不执行系统进程校验、不启动旧 GUI、不保存项目，不能单独作真实恢复证据。既有通用 transition API 和 macOS 调用路径保持不变，Windows 完整控制器必须使用上述严格接口，不能靠直接 transition 代替恢复健康确认。

新增共用用例覆盖错误阶段、失败候选 PID/版本、外来产品/平台/路径/事务/nonce、初始化或画布缺失、空/零原生身份、合法恢复及拒绝重复提交；本机五项事务测试通过，工作区及实际 Windows 库/测试交叉 Clippy 通过。Windows 专用恢复启动记录、原版 GUI 首画布回执和完整助手调用链仍待实现/验收。

`57227cb` 的 Windows audit 运行 `36932270906` 已通过；下载 artifact `11196987874`，读取 validation/summary.json 确认 formal-workspace-tests（workspace/locked/all-targets）为 pass。附件未包含单项测试日志，因此不把摘要说成已逐项读取所有断言；不覆盖后续本地提交的 Windows 运行状态。Windows Quality 和真实 Inno 附件仍按其最终结果独立核验。

本批本机 `cargo test --locked -p instplot-studio --all-targets --all-features` 完整通过：库 218 通过/1 原有忽略，二进制 114 通过，集成契约 8 通过，共 340 通过/1 忽略。包含既有数据、绘图、轴、文字、持久化和导出回归；不是实际 GUI 更新操作。Windows 专属测试不计入本机数量，未解锁或绕过锁屏，未替换用户应用。安装前与恢复状态门的本地提交等待本轮 Windows Quality 最终证据后合批推送，避免取消仍在工作的验证。

## 四项 CI 重验与恢复角色预览接入

`57227cb` 四项 CI 全部通过。已读取 Windows job `110604256701` 日志，保存项目范围拒绝、实际启动失败、真实测试 Child 提前退出三项测试通过。下载真实恢复 artifact `11197267579`：desktop-on/off-native-runner.json 均 applied/restored/user_data_preserved=true；results.json 两例的快捷方式、取消、原路径、原生发现、卸载身份、故障恢复等断言均 true，生产版本未改。范围仍为原生 Rust/Inno 安装器原型，不是 GUI 更新。随后普通推送本地合批至 `76d7aaf`，新 CI 待验，不借旧提交结论。

新本地预览实现增加 LaunchPurpose Candidate/Recovery，复用原有 owned Child、健康窗口及编辑保护，不另建 GUI 流程。恢复预留公开接口先要求原版恢复安装器持久成功、旧注册/固定 release 文件验证及排他锁，再记录旧 exe/项目只读绑定；内部 `--update-recovery-health` 仅 Windows preview 可用，严格验证恢复角色和运行旧版本。恢复 launch/health/stop/exit/未启动证据均用独立文件，新版遗留停止请求或健康回执不得影响旧版。已有缺 purpose 的候选记录按 Candidate 解释，不把旧记录解释为 Recovery。

旧 GUI 首画布实际重开主项目后写自己的健康回执，助手必须保持精确原生存活句柄，核验旧版本及回执，持久提交 RolledBack；GUI 等到提交后才放行编辑。恢复进程无绑定超过 5 秒、助手未提交超过 75 秒、记录/角色改变、已有外来回执均拒绝或正常关闭，不强杀、不再次安装。恢复失败的进程不能进入“候选失败后重新恢复”状态门，保留证据人工处理。

新增 Windows 组件测试覆盖独立角色/文件、新版回执隔离、错误恢复版本、提交写盘失败保持 Restoring、提交前编辑门状态 Pending、合法原生句柄健康提交与重复/角色变化拒绝；用的是当前测试进程，不是实际 Studio GUI。恢复 CLI 增加精确单路径参数测试。实际 Windows 全库/测试交叉 Clippy、工作区全目标全特性 Clippy、本机既有 startup 两项通过；Windows GUI 二进制及新专属用例待 CI。没有开放协议 1、重启按钮或 apply CLI，发布旧包尚不支持此内部恢复命令，兼容迁移要求不能省略。完整助手执行/真实更新恢复、配置保护审计及 macOS 下载按钮链继续未完成。

## 安装器 owned 锁与阶段衔接（本地实现，真实 Inno 重验待 CI）

`RunningWindowsInstaller::start_owned` 将目标排他锁移入真实安装器对象，保留已有 borrowed 接口和全部原生/包/阶段检查。`take_owned_access_after_exit` 只在实际 Child 已退出且退出检查点重新持久写入后交出 owned 锁；存活、记录写盘失败、borrowed 模式或重复转移都拒绝，错误时对象仍持有锁、Child 与包租约。转移后对象不能再次轮询或转移；接收方仍持锁，必须完成后续身份核验再交给单次 GUI 启动，不把交接当健康成功。

助手新增固定 `start_candidate_installer` 和 `start_recovery_installer_after_candidate`：先取得包只读租约，再调用既有严格状态门，然后进入 owned 原生执行器；日志固定在事务目录，不接受任意命令。不接 GUI 重启按钮或 apply CLI，不替代工作/配置保护、兼容协议和完整控制器。启动前失败保留持久意图供检查；启动后写盘失败仍返回保留实际进程的执行对象。

既有原生租约故障测试改走 owned 模式，额外验证检查点被占用时拒绝交锁、退出后接收方仍阻止其他实例、重复转移/转移后轮询拒绝。真实 Rust/Inno 原型的升级和恢复均改走 owned 模式并确认一次交接，最后释放的是恢复返回的同一锁。实际 Windows 全库/测试交叉 Clippy、工作区全目标全特性 Clippy 通过；这些修改的 Windows 原生执行与真实 Inno 附件待下一批 CI。未修改生产版本、信任根、默认入口或发布状态。

## 健康提交确认丢失与设置写入范围

`76d7aaf` 四项 CI 全部通过，Windows Quality job `110615059725` 中恢复健康事务测试通过，Studio 库 254 通过/2 忽略、二进制 112 通过；真实 Inno 原型两次运行均通过。此结论不覆盖后续恢复角色、owned 安装器锁及本节新修改，也不是实机 GUI 更新验收。

健康状态原子写入可能成功、返回确认却失败。新增只读 `verify_committed_health`：必须以保留的原生存活句柄、同一不可变启动记录、精确角色健康回执及实际持久 Completed/RolledBack 共同确认。失败不能仅凭终态字符串判成功；成功也不重新提交、不重启 GUI、不自动回滚。补测写入前失败与写入后确认丢失，重复只读核验、错误回执/角色及进程退出拒绝。本机五项事务测试及实际 Windows 全库/测试交叉 Clippy 通过，Windows 运行待新 CI。

当前 eframe 依赖关闭 default-features，实际 feature tree 没有 persistence；App 未实现持久化 save/on_exit 覆写。健康待确认 UI 提前返回，跳过文件导入、快捷键及编辑/保存流程；显式项目保存仍由用户操作触发。因此当前应用设置恢复白名单为空，不备份或覆盖整个用户目录。此为当前构建的源码/依赖审计，不是任意未来版本的保证。未来新增持久设置须重新列出精确写入白名单并验收；项目及手动数据另行保护，签名更新缓存/安全水位不得回退。完整助手控制器仍须落实工作冻结、版本协议兼容及失败阶段衔接，入口保持关闭。

## 未启动候选 GUI 的安装失败恢复门（本地待 Windows 运行）

新增 `start_recovery_after_failed_installer`，只处理 Applying 阶段真实新版安装器非零退出、尚无候选 GUI 意图的情况。必须由仍保留实际 Child 的执行器证明同一事务目录、签名包、原生 PID/创建时间/退出码及持久退出记录完全一致，且 owned 排他锁已在持久退出后交接；磁盘记录独自不足，正在运行/未知退出/零退出/记录篡改/其他事务拒绝。旧 GUI 必须已退出，旧恢复包绑定并持有只读租约，候选启动、恢复安装器或恢复 GUI 的既有/部分意图都拒绝。成功只进入 RecoveryRequired/Restoring 并启动一次受限旧安装器，不是 RolledBack 或健康成功；部分状态写入失败保留检查，不重放。

新增状态门组件测试覆盖错误阶段/nonce、原生复核失败不改变 Applying、三类部分意图不覆盖、合法一次切换及重复拒绝；扩展实际测试 Child 用例，验证锁转移前拒绝、转移后非零退出证明、篡改 PID 和外来事务拒绝。测试 Child 是接收 Inno 参数的 Rust 测试程序，不是真实 Studio GUI，也不冒称 Inno 故障恢复全链通过。实际 Windows 全库/测试交叉 Clippy 通过；新 Windows 运行待下一批 CI。完整控制器、启动前失败/不确定意图的人工检查，以及实机 GUI 门槛继续保留。

## 可轮询助手阶段引擎（未接生产入口，Windows 运行待验）

新增 `WindowsUpdateController` 保留实际安装器/GUI 对象、包租约及排他锁交接状态；每次 poll 仅执行一步，不等待线程、不启动定时监听、不强杀。成功安装后经原生身份核验单次预留/启动候选；候选健康成功或丢失确认后的精确只读复核成功才 Completed。真实非零安装退出且未启动 GUI 走前述恢复门；候选健康失败须正常停止、持久 owned 退出及释放 exe 租约、重新取得目标排他锁，才能一次旧安装/独立旧 GUI 健康确认。恢复健康才 RolledBack，恢复安装器/GUI 再失败不再次恢复或重开。

等待健康限 60 秒、正常关闭及重取锁限 30 秒；真实安装器仍运行时继续保留对象，不以等待时间推断退出。轮询错误锁定 InspectionRequired，后续 poll 不重放；调用者必须继续持有该控制器供检查，不能错误路径直接退出助手或丢弃活进程的保护。持久阶段已完成却无法严格复核健康时停于检查，绝不发关闭请求或回滚。未绑定且实际已退出/确定未启动的候选走 owned 退出证明；活跃且检查点不确定的进程保留检查，不伪造健康。

新增阶段策略测试覆盖两种健康角色、全部非匹配阶段与 59/60/90 秒边界；安装前新鲜意图门同时拒绝 recovery-launch，apply 与取消测试均增加该项。实际 Windows 全库/测试交叉 Clippy、工作区全目标全特性 Clippy 通过；新策略及真实引擎调用链尚未 Windows 运行。引擎只提供显式后置预检接口，无生产调用者/CLI；工作冻结、兼容协议探针、助手就绪/旧进程等待的外层调用和真实 GUI 测试仍必须完成，不能仅凭引擎存在开启入口。

阶段引擎继续加入 `wait_after_preflight`：认证复制体先持久写精确 helper-ready，父窗口确认后才可正常关闭。每次 poll 用保留的原生旧进程句柄非阻塞观察，不强关；旧 GUI 退出且所有支持实例放开共享锁后，才取得目标排他锁并通过既有安装前最终复核。就绪后 30 秒到期无论刚退出还是锁未满足均不启动安装；旧身份/固定文件及无执行意图复核成功后进入 FailedBeforeApply，否则 InspectionRequired，不伪称取消或回滚成功。新增 0/29/30/31 秒策略边界测试，Windows 运行待 CI。仍未接父窗口工作冻结、兼容协议和 Windows apply CLI，因此没有开放重启/安装入口。

父窗口的 `confirm_ready` 进一步要求同一 WaitingForExit 持久事务与请求原文未变，并在原安装/固定文件复核之后重新观察实际 owned helper Child 的创建身份与存活；就绪文件本身不授权退出。该检查通过只读 snapshot，不申请助手持有的写锁，不修复请求。新增组件测试覆盖助手写锁持有时正常读取、Prepared/Applying 与外来 nonce 拒绝、请求中的桌面任务变化拒绝且不改回原文。实际 Windows 全库/测试交叉 Clippy 通过，Windows 运行待 CI。外层父窗口冻结/启动调用尚未接入。

本机本批完整 `cargo test --locked -p instplot-studio --all-targets --all-features` 退出码 0：库 218 通过/1 原有忽略、二进制 114 通过、集成契约 8 通过，共 340 通过/1 忽略；工作区及实际 Windows 库/测试交叉 Clippy 通过。这不包含 Windows 专属策略/原生运行，也不是用户界面点击完成更新的验收。当前 GitHub head `5392542` 三项通过、Windows Quality 运行中；后续本地三项提交单独等待推送验收，不借旧证据。

`5392542` 四项 CI 后续全部通过。Windows Quality job `110624371219` 日志明确显示候选精确持久健康/丢失确认测试、独立恢复角色/旧健康提交测试、owned 原生安装器锁测试以及恢复健康 CLI 参数测试通过；Studio 库 255 通过/2 忽略、二进制 113 通过。真实 Inno 两次运行通过；附件 `11199607448` 已下载至 ignored target/windows-recovery-5392542.ySghua，桌面 on/off 原生 JSON 均 applied/restored/user_data_preserved=true，results.json 两例无 false 断言、生产版本未改。仍仅安装器恢复原型，不是实机 GUI 自动更新；后续控制器/非零失败恢复/退出等待提交需新一轮 CI。

## 0757b18 原生失败夹具修正

Windows Quality job `110632039722` 单项失败于 assets.rs 的非零退出证明：原测试假定 Rust test harness 收到 Inno 参数后返回非零，但实际 Windows 返回 0。生产验证正确拒绝零退出，因此不能把该测试计为失败恢复通过。库 259 通过/1 失败/2 忽略，该轮在真实 Inno 步骤之前停止，未生成恢复附件；不能借前轮附件覆盖此次提交。

修正为 cfg(test) 专属 spawn 适配器选择精确 `installer_failure_test_child`，只向该 Child 设置夹具环境，实际调用 process::exit(23)，持久记录另断言退出码精确 23。包验证、旧进程观察、写前意图、真实 Child 身份、检查点故障、租约及 owned 锁流程共用原实现。生产适配器仍固定 Inno 参数，无夹具环境/命令入口；共用参数用私有结构封装，不放宽验证。Windows 全库/测试交叉 Clippy 通过，修复后的 Windows 原生运行待 CI，不称本机已运行 Windows 测试。

## 健康后解除主项目只读租约（本地，Windows 运行待验）

阶段引擎确认真实存活 GUI 的精确持久 Completed/RolledBack 后，才释放该启动对象与助手会话持有的主项目只读句柄，避免助手仍存活时用户正常保存被共享权限阻止。跨事务目录、未提交、未绑定/退出/检查点不明、回执不一致均不得提前解除；解除只关闭已持有句柄，不写项目、不清理备份、不回退安全缓存。GUI 自己的健康启动保护仍待提交后结束。

扩展原生测试 Child/真实保存项目组件：未绑定和 AwaitingHealth 不解除，精确提交后可以打开项目写句柄但不改任何字节，重复合法解除可幂等核验，进程退出后拒绝确认。助手会话使用单线程 Cell 保存 lease 以允许控制器的受限终态释放，仍不提供任意资源释放入口。Windows 全库/测试交叉 Clippy 通过；真实 Windows 运行和 GUI 后续保存验收待本批 CI/实机，不据此开放入口。

`374b801` 修复后的 Windows audit 运行 `36942139416` 成功。附件 `11201056953` 已下载至 ignored target/windows-audit-374b801.0NqPh2，validation/summary.json 明确 formal-workspace-tests（cargo test --workspace --locked --all-targets）退出码 0/pass；附件没有单项 formal-workspace-tests.log，不能声称从此附件逐项读取了新断言。Linux/macOS Quality 同轮通过，Windows Quality 尚在运行，真实 Inno 附件仍须最终独立核验。PR #9 当前没有未解决 review thread。

本机补跑 macOS 助手五项组件均通过：真实 ad-hoc bundle 版本/哈希/篡改拒绝、真实签名 bundle 交换恢复、真实只读 DMG 身份/正常卸载及拒绝路径、精确 ready 绑定和只读状态文件边界。使用隔离夹具，不替换用户应用，不等于下载按钮/重启/保存对话框的完整 GUI 验收；桌面解锁门槛与 Windows 实机门槛仍未解决。

`374b801` 最终四项 CI 全通过。Windows Quality job `110635867340` 日志确认非零原生 Child 退出证明、助手写锁期间父窗口只读复核、安装前正常退出期限、已完成/外来阶段不关闭不恢复等新增断言通过；真实 Inno 两次运行通过。恢复附件 `11200684226` 下载至 ignored target/windows-recovery-374b801.HpayuQ：results.json 两例无 false 断言、production_version_unchanged=true；desktop-on/off-native-runner.json 均 applied/restored/user_data_preserved=true，desktop_shortcut 分别 true/false。范围明确 installer-recovery-prototype-not-GUI-updater，不代替 Windows 实机 GUI、父窗口启动助手或健康后实际保存验收。后续 `8834646` 的健康提交后项目 lease 释放仍须新 head 的 Windows CI。

## 独立助手生命周期（内部实现，Windows 运行待验）

`run_helper_after_preflight` 持有认证 helper 会话对应的完整控制器，250ms 间隔顺序推进，不强杀、不重放、不清理。只有 Completed/RolledBack/FailedBeforeApply 合法终态直接返回；引擎出错后停止阶段推进，只读观察实际 owned 安装器及 GUI Child。存活或观察错误均继续保留所有对象/租约/锁，不以超时推定退出；全部已退出后带原错误返回，未完成事务仍阻止普通应用误入，不伪造健康/恢复。该函数没有生产 CLI/UI 调用者，不能代替父窗口冻结、协议/产品兼容探针或真实 GUI 门槛。

策略夹具覆盖三类合法终态、安装中不提前结束、错误及 InspectionRequired 后无步骤重放、存活/原生观察错误均不释放，只有确证全部退出才返回。实际 Windows runner 用例补充检查点故障期间只读 owned 退出观察不修改 journal、不转交排他锁，已交接后仍可只读观察；这些新断言尚待 Windows CI。策略夹具不是原生 GUI 验收。

本批工作区全目标/全特性 Clippy 与实际 Windows 库/测试交叉 Clippy 通过；本机 Studio 全目标/全特性测试 340 通过、1 原有忽略、0 失败。macOS 回归不能代替仅 Windows 编译的新增运行断言，仍由下一批 CI 核验。

## 安装成功后、候选启动前的身份失败（本地，Windows 运行待验）

精确 owned 安装退出证明分离为只读退出码接口，仍要求原 Child、包绑定、持久 journal 与原生 PID/创建时间/退出码完全一致、无检查点错误且排他锁已交接；非零失败专用接口仍拒绝零退出。引擎预留候选失败时不直接把任意错误当恢复许可：仅在 Applying、无候选/恢复部分意图、原进程退出、排他锁/可信旧包有效，并重新核验候选确实身份无效或缺失时，允许真实退出 0 的安装器进入一次恢复。重新核验正常、权限拒绝/未知 I/O 错误、已有意图或不匹配持久状态停于 InspectionRequired。恢复成功仍需独立旧 GUI 健康确认，不提前 RolledBack。

新增策略测试区分零/非零退出、身份正常/无效/缺失、权限和未知 I/O；共用无意图状态门既有错阶段/nonce/原生证明失败/部分意图/重复拒绝测试继续适用。真实 Inno on/off 升级及恢复用例增加 owned 退出码精确 0 与非零恢复接口拒绝断言；原生故障夹具增加精确 owned 退出码 23。Windows 库/测试交叉 Clippy及工作区全目标全特性 Clippy 通过，新增 Windows 运行待推送后 CI，不能借前轮证明本批通过。

2026-10-02 00:16 UTC 核验 head `1b33d2d` 四项 CI 均在运行，本轮不持续轮询；尚未称其新增生命周期/项目 lease 断言已运行通过。随后只读检查桌面仍显示 Mac 锁屏，完整下载/DMG/重启按钮链继续待解锁；没有绕过锁屏、替换 Spotlight 或发布。

## 父窗口异步回执与工作冻结回归

Mac HelperReady 必须对应 LaunchingHelper，错阶段就绪不会关闭原窗口；准备失败和通道断线解除等待状态而不授权退出。合法就绪只产生一次 Close 请求，等待关闭期间仍锁住工作。通用异步更新 poll 在首个终结事件后立即消费 receiver，不允许排队的重复结果/后续进度覆盖终结状态；不增加后台监听。

五项新增测试覆盖终结重复拒绝、等待期间冻结且不关闭、失败解除冻结、断线不退出、合法就绪单次关闭且保持冻结、错阶段就绪不退出（等待与失败在同一用例）。`cargo test --locked -p instplot-studio --bin instplot-studio --all-features app_update::tests --quiet` 本机 17 通过、0 失败；工作区全目标全特性 Clippy 通过。消息夹具不是实际按钮/原生助手身份验收，也不宣称 Windows UI 运行通过。2026-10-02 00:26 UTC 远端 `1b33d2d` Linux 通过、其余三项运行中；本地 `9777fc4` 及本批等待该轮最终证据后再推送，不取消运行中的验收。

`1b33d2d` 四项 CI 已于 2026-10-02 00:36 UTC 核验全部通过，无未解决 review thread。Windows Quality job `110645188843` 明确新增 helper_runtime 两项策略、保存项目健康前后 lease 用例及原生 runner 用例通过，Studio 库 263 通过/2 忽略；真实 Inno 两次通过。恢复附件 `11202511045` 下载至 ignored target/windows-recovery-1b33d2d.H7RThh，两例无 false 断言、生产版本不变；desktop on/off 均 applied/restored/user_data_preserved=true，desktop_shortcut 分别 true/false。生命周期和项目 lease 组件由此有实际 Windows 运行证据，但仍不是 GUI 自动更新/实际重开后保存验收。后续 `9777fc4` 与 `077e546` 单独进入下一批 CI。

## 签名安装契约与兼容拒绝门（本地，尚未发布契约资产）

可选 `platforms.windows-x86_64.windows_in_place` 完整包含在既有原始字节签名中，严格字段：schema、helper_protocol、transaction_schema、candidate_health_protocol、recovery_health_protocol、executable_sha256、license_sha256。其他平台带该字段、无效 schema/零协议、畸形/大写哈希及未知字段拒绝；未知正数协议可手动下载，但当前自动应用只支持全部为 1 的契约。缺字段的真实旧清单仍能验签下载，不能创建自动替换助手。尚未发布任何带契约的生产包或清单，不推测 rc.2 恢复安装器具备健康接口。

复制助手创建与认证载入要求双方兼容契约，旧安装 exe/LICENSE 精确匹配可信恢复包的签名哈希；非预览构建没有 Windows 健康入口，拒绝创建助手。安装后和恢复后验签绑定固定文件，候选/恢复单次启动预留时在实际 exe 只读 lease 保持期间再次比对签名哈希，错哈希不写任何启动意图。文件读取固定路径、拒绝重定向、限制大小和实际读取量，不执行清单命令、修改用户数据或修复错误资产。

新增三项跨平台测试：旧清单下载与自动应用隔离；签名契约匹配固定文件/文件篡改/缺失/未重签契约篡改/未来协议拒绝；有效签名下的畸形契约和跨平台错放拒绝。资产测试本机 9 通过，原签名更新测试 4 通过，工作区 Clippy 和 Windows 库/测试交叉 Clippy 通过。新增 Windows pinned-binary 用例要求错哈希在写意图前拒绝、匹配后预留一次，原生运行待后续 CI。

发布生成/验证脚本目前仍只处理现行清单，不生成契约字段；必须在真实助手入口和兼容能力验收完成后同时接通，不能凭声明开放更新。旧 rc.2 客户端会拒绝未知字段，首次仍须手动迁入更新客户端，不能覆盖既有 rc.2 公共清单。此为计划内兼容门的接收规则，不代表 Windows GUI、父窗口或正式发布完成。

## Windows 预览复制助手命令与受限启动适配器（本地）

预览二进制新增精确 `--windows-update-helper` 单一事务路径命令，必须先经 WindowsHelperSession::load_waiting 完整认证：固定私有根、事务/schema/nonce/请求/当前复制体路径哈希、原生旧进程身份、新旧可信签名包/契约及固定旧文件，才能进入独立 helper loop。普通安装 exe 直接调用不能冒充复制体，普通构建不存在此入口；不新增用户可用 apply/restart 按钮，公开 Windows 协议仍 0。

PreparedWindowsHelper 新增预览固定 spawn 适配器，复核后单次持久 WaitingForExit 再启动复制体，参数固定为命令和字面事务路径；不使用 shell、不传任意环境/命令，返回实际 Child。CreateProcess 明确失败时，在精确旧 GUI 仍活跃、原安装/固定文件正常及无执行意图条件下持久 FailedBeforeApply；中止失败保留证据并报检查，不重复 spawn。成功后调用者必须继续持有 Child/Prepared 对象，原生 confirm_ready 成功才允许正常退出；当前 GUI 尚未接该调用及取消生命周期，不能把此适配器当作已可用自动更新。

新增 Windows 启动命令解析/缺参数/多参数/普通 exe 非私有事务拒绝测试，以及固定路径含中文空格/`&` 仍为字面参数且无自定义环境的命令构造测试。Mac startup 测试 3 项通过（其中验证非 Windows 不暴露该命令），工作区 Clippy 与实际 Windows 库/测试交叉 Clippy 通过；Windows 二进制 CLI 与真实创建/就绪/退出链待 CI/实机，不能声称本机已运行 Windows 命令。

2026-10-02 00:49 UTC head `21cf096` Mac Quality 与 windows-audit 通过，Linux/Windows Quality 运行中；只核验一次，本地契约和预览入口改动待本轮最终证据后再推送。未修改版本、发布、OSS、Spotlight 或生产私钥。

## 父侧持久取消与助手立即识别（本地，Windows 运行待验）

cancel_before_exit 只由匹配 PID/原生创建时间/实际 exe 的仍活跃原父窗口调用；私有写锁下要求 WaitingForExit、无任何 apply/restore/candidate/recovery 意图及原文件不变，才持久 FailedBeforeApply。已写成功但确认丢失时，只读验证精确取消终态与原文件后可幂等确认，不重写事务。父窗口在提交失败时仍冻结/open，不能仅因用户点取消就允许退出旧进程。

助手 waiting poll 优先检查精确事务取消终态、无执行意图及原安装/签名旧固定文件；匹配则正常终结并释放会话租约，WaitingForExit 继续既有等待，其他阶段/nonce/部分证据拒绝，不能转成取消或触发安装。父侧取消提交后仍必须持有 actual Child/Prepared 并等待该助手正常退出，再释放父侧项目只读句柄和解冻保存；本批没有把提交成功假称为进程退出或 GUI 恢复可写。

扩展 cancellation 组件用例：等待状态不伪装取消；外来 nonce 拒绝；取消终态连续只读核验两次且原始 transaction 字节不变；原文件复核失败拒绝；四类部分执行意图拒绝且不修改证据；Applying 阶段不能识别为取消。实际 Windows 库/测试交叉 Clippy 与工作区全目标全特性 Clippy 通过，新运行断言及真实父窗口取消/租约释放仍待 CI/实机。

2026-10-02 00:59 UTC head `21cf096` Linux/Mac Quality 与 windows-audit 通过，Windows Quality 仍运行中；不持续监听、不取消该轮验收。后续本地改动继续保留待推送。

2026-10-02 01:08 UTC 核验 `21cf096` 四项 CI 全部通过，无未解决 review thread。Windows job `110651660121` 明确候选启动前身份失败策略、原生 installer runner 和父窗口重复终结回执测试通过，Studio 库 264 通过/2 忽略，真实 Inno on/off 两次通过。恢复附件 `11203122796` 保存于 ignored target/windows-recovery-21cf096.jSbXC2，范围仍为 installer-recovery-prototype-not-GUI-updater；生产版本不变，desktop on/off 均 applied/restored/user_data_preserved=true，快捷方式分别 true/false。本轮仅证明远端既有改动；签名契约、预览助手 CLI 和扩展取消门三批本地提交将进入新一轮 CI，不借旧 head 声称新增断言已运行或 Windows GUI 验收完成。

随后已推送 `f0dba8d`。本机对该 head 执行 `cargo test --locked -p instplot-studio --all-targets --all-features --quiet`，总计 349 通过、0 失败、1 忽略（库 221、主程序 120、集成 8）；包含实际 Mac 更新组件集成测试，但不等同完整下载/DMG/按钮重启 GUI 链。父窗口调用点仍明确阻止 Windows 启动助手，尚未接入保存确认后持有 Prepared/actual Child、就绪等待和取消后实际退出解冻；不以底层命令可用冒充界面已接通。不重复轮询或推送取消正在进行的新一轮 CI，本条本地证据留待下一批提交。

2026-10-02 01:20 UTC 核验 `f0dba8d`：windows-audit 通过，三平台 Quality 在仓库规范阶段失败，未进入新代码编译。日志定位为 helper_command 测试内示例用户目录绝对路径被规范检查拒绝；改用运行时临时根构造，仍保留中文、空格、`&` 与固定字面参数/无自定义环境断言，不修改 CI 规则。本地仓库规范检查和 Windows 库/测试交叉 Clippy 通过；新 Windows 运行断言仍待修正后的 CI，不使用 audit 成功冒充这些测试已运行。

## 父侧实际子进程与租约持有器（本地，未接通 GUI）

WindowsParentHelper 预览接口在既有单次 spawn 后保留实际 Child 与 Prepared，之后没有落盘/认证等可能抛错丢失持有对象的步骤。就绪每次复核真实进程与既有完整请求证明，仅缺文件且未超时可继续等待；证明完成后检查 10 秒界限，延迟回执不授权关闭。任何取消尝试后禁用就绪，必须先持久取消、再观察实际 Child 退出、最后再次验证取消终态/原文件，才释放父侧项目与资产租约。false/错误都保持 owner 和工作冻结；没有自动关闭或强杀。正常关闭前调用方必须再次复核，就绪结果不能长期缓存。

正常取消及时释放资源；误 Drop 时真实子进程活跃/退出状态未知，保守保留所有资源至父进程退出，避免把误 Drop 当取消成功。新增三项策略测试覆盖就绪精确成功/缺回执/权限及其他错误/超时；取消提交失败前不观察退出、活跃/未知退出不释放、退出后复核失败；误 Drop 的保留策略。工作区全目标全特性 Clippy 和 Windows 库/测试交叉 Clippy 通过，策略运行仍待 Windows CI；不是实际父窗口、真实资源释放或实机 GUI 证据。当前 AppUpdateState 仍未持有/调用该对象，默认入口不变。

2026-10-02 01:30 UTC `30dd4ca` Linux Quality 通过，其余三项运行中；本地持有器改动待该轮最终证据后推送，不取消正在运行的验收。

2026-10-02 01:40 UTC `30dd4ca` Linux/Mac Quality 与 windows-audit 通过，Windows Quality 仍运行；不重复轮询。父侧接入检查确认现有 shared 安装锁及未完成事务 startup 门不能单独证明既有第二实例已退出，父窗口正常退出前仍须实现原生唯一实例证明；助手之后拿不到排他锁虽然能保护文件，不能代替退出前门槛。本地 holder 不授予多实例安全能力，GUI 尚未接入，继续默认关闭。

2026-10-02 01:50 UTC 核验 `30dd4ca` 四项 CI 全部通过，无未解决 review thread。Windows job `110662318313` 明确签名契约严格验证、错 binary 哈希写启动意图前拒绝、复制助手字面命令、普通 exe 助手命令拒绝及扩展持久取消测试通过；真实 Inno 升级/恢复 on/off 两次通过。附件 `11204019579` 保存于 ignored target/windows-recovery-30dd4ca.m4iCdN，results 无 false 断言、生产版本不变，两例 applied/restored/user_data_preserved=true、desktop_shortcut 分别 true/false。仅证明该 head 的组件和 CLI 运行，仍非 Windows GUI 自动更新验收；后续 `403b82c` 父侧持有器尚待新的 CI。

## 原生父侧唯一实例门及取消后共享锁恢复（本地）

WindowsInstallAccess 新增 held 状态，未持有锁不能伪装 shared/exclusive。promotion 仅在真实同目标父进程和精确 WaitingForExit 请求复核后尝试原锁 shared → exclusive，不重复在已锁句柄上叠加不确定的锁操作；释放共享后真实非阻塞排他锁失败则恢复共享，恢复失败保持 held=false、报检查。拿到排他后再验证等待证明，证明失败保留排他而不授权关闭。WindowsParentHelper readiness 现在要求仍持有同目标父侧排他 guard，promotion 失败禁用就绪；取消真实 Child 退出且状态再次验证后，恢复同一 guard 的共享锁，才能释放项目租约，恢复失败保持冻结与 owner。没有枚举 PID、强杀或省略助手独立排他检查。

扩展实际 Windows 安装锁组件用例：缺等待证明不释放原共享锁；第二实例阻止提升且共享锁恢复；唯一实例可提升并阻止新的 shared；重复提升拒绝；提升后证明变化仍保留排他；未取消不恢复共享；取消后恢复共享并可重复确认。该用例的状态门使用隔离闭包夹具，真实持久请求门由生产接口验证，不能将闭包当作端到端退出/取消证据。Windows 库/测试交叉 Clippy、工作区 Clippy 通过；新增原生运行断言待下一轮 CI。父窗口 GUI 尚未接入，未开放入口、未修改版本或发布。

## 父窗口内部预览接通与退出边界（本地，GUI 实机仍待验）

AppUpdateState 现保留实际 WindowsParentOwned，沿既有保存/草稿保护后 launch_helper 内部路径绑定 primary 项目、单次启动、父锁提升、250ms 就绪/取消轮询及冻结分支。不增加公开安装按钮。启动失败保留 Box<WindowsParentStartFailure> 与项目租约，只有精确 Prepared/Waiting 无执行意图及真实原进程/文件证明的持久取消成功、原 shared guard 验证、失败 owner 释放后才解冻；Active 则还要求真实 Child 已退出、锁恢复。解释错误、缺 owner、phase 异常不得覆盖成解冻的普通 Failed。取消或超时不授权 Close；即将发出 Close 时再次复核完整证明，单次派发后不提供迟到取消；状态撤销不能沿用缓存 allow_close。

主窗口冻结分支原本对所有 close_requested 发 CancelClose，现只拦未授权关闭。新增实际 egui 主窗口 headless 用例验证提前关闭被取消、已获准关闭不被取消；仅模拟授权开关，不声称真实助手健康已运行。Mac app_update 测试 18 通过，保存/草稿 update_workflow 测试 4 通过；本轮全量回归 350 通过/0 失败/1 忽略（后续退出授权状态小补丁另跑 targeted 检查）。Windows 增加 missing owner/错误/重查不能解冻或退出测试，并扩展 Prepared 无 Child 持久取消拒绝原文件失败/执行阶段、成功到 FailedBeforeApply；新运行断言待 CI。

Windows 库及实际 app_update 源码的隔离交叉 Clippy 类型检查、工作区全目标全特性 Clippy 通过。ignored target/windows-native-check 加入 UI wrapper，仅类型检查使用不带 TLS 特性的 ureq 以避开本机缺 Windows C SDK；不运行网络、不修改生产依赖/锁文件、不冒充 Windows 链接、TLS 安全或 GUI 运行验证。真正 Windows CI 继续使用生产 Rustls 和既定依赖。测试夹具初次 headless 输出未清理 texture delta，按 egui 测试契约清理后通过；未改生产渲染逻辑。

2026-10-02 02:15 UTC（距上次核验超过 10 分钟）`2bac6b1` 四项 CI 全部通过，无未解决 review thread。Windows job `110669478773` 的 parent_helper 三项策略通过，真实 Inno 两次通过；附件 `11205930679` 在 ignored target/windows-recovery-2bac6b1.Pzeu2y，scope 仍为 installer-recovery-prototype-not-GUI-updater，无 false 断言、生产版本不变，on/off applied/restored/user_data_preserved=true、快捷方式分别 true/false。此次仅证明已推送 holder 策略，不借它宣称后续 `45e531e` 原生提升或本批 UI 实机通过。

## 只读预览能力探针（Windows 运行待验）

仅 Windows x86_64 的 in-place-update-preview 构建接受无参数 `--windows-update-capabilities`，多余参数拒绝；普通构建与其他平台拒绝。静态 JSON 区分组件协议与已验收更新能力，scope=preview-components-not-accepted-updater，公开协议 0、GUI 验收 false、公开应用入口 false。不会读取事务、注册表、私钥、联网或启动助手，不改变安装和版本。Windows 测试覆盖字段及命令运行，实际证据待 CI；Mac startup 四项测试通过，只证明非 Windows 拒绝及既有启动行为。不能用这个探针代替真实 GUI 验收或授权生产清单生成。

2026-10-02 02:28 UTC 单次核验 `2a8242d`：Linux/Mac Quality 已通过，Windows Quality 与 windows-audit 仍运行。未持续监听；新增父侧锁/取消/UI 断言尚未获得这一 head 的最终 Windows 运行证据。
