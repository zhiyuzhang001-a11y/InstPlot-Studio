# Windows GUI 实机问题与无人值守验收

## 最终结论（2026-10-10，北京时间）

source `be5da81119ae8ef45c7ae1638f9bed86f9e86a8e` 的完整虚拟 Windows GUI run37965535242、四项必需 CI、准确 Release QA37965541633-1 和独立公网校验均已通过。GUI 实际自动提示/点击、旧退出、原目录升级、唯一新窗口持续30秒、正常项目编辑器与原生退出0已逐项核验。用户另行确认新baseline正常打开并授权激活后，run38035232708激活同一QA的rc.3；用户最终反馈“这次没问题了，大概5s以内完成更新”。该速度是用户观察，不是分段性能采样。

产品 PR9、发布准备 PR19 与此前 QA 部署 PR18 已合并；公开 rc.3 技术 prerelease 与 OSS prerelease 渠道已发布，stable 未启用。新发布源码的 GUI/平台/公网证据见[rc.3 发布收尾记录](INSTPLOT_STUDIO_RC3_RELEASE_CLOSEOUT.md)。下文失败、禁止合并与“待运行”保留为阶段历史，不代表最终状态；复用方法见[工程复盘与复用指南](WINDOWS_UPDATER_ENGINEERING_PLAYBOOK.md)。

2026-10-09 run37963369054 / ba1f159：native退出码0/7自测实际通过。自动提示与真实点击后约8.25秒完成升级，候选可见存活30秒，但项目窗口标题检查失败；截图为初始标题InstPlot Studio与禁用灰画布。Windows健康UI在Committed清除health后仍return，刚绘制的禁用画布不会保证再触发一帧，故无后续事件时普通编辑器/项目标题没有刷新。补Committed后显式request_repaint，强制下一帧正常UI；不跳过健康门、不改变安装或恢复授权。此为产品修改，必须新source/new QA，不复用3ba成功包作为本次修复交付。新完整GUI与准确包仍待执行。

2026-10-09 run37960121663 / GUI test source d3d448e：真实自动弹窗、Tab/Enter点击、旧PID正常退出、原路径升级、唯一新版窗口持续可见30秒、项目/快捷方式/rc.3身份与事务completed均已取得证据。点击至completed约13.7秒；helper ready930ms、父准备955ms、完整proof120ms。该值是临时软件渲染VM/loopback TLS，不承诺用户公网环境同速。截图已人工核对。最后正常关闭检查失败，最终桌面截图已无Studio窗口；原脚本对Get-Process取得的非子进程使用延迟Process.ExitCode，没有保留原生退出证据，因此不能断言崩溃或正常退出。改为关闭前保留仅query/synchronize的原生句柄并绑定事务创建时间，正常CloseMainWindow后严格等待并读取实际exit code，任何非零/缺证据仍失败。仅测试脚本改动，不修改产品/包；完整GUI成功门仍待新run。

2026-10-09。用户授权自主修复并要求在虚拟 Windows 上验证，不再反复由用户操作另一台电脑。

## 已取得的证据

- QA37938590122-1，source d44895d5e0956f3b1e8e862bba1be54e1d2e4c85。
- 事务 ddac72ccc8956b52a63ee2bdd11b3d3d 从 rc.2 到 rc.3，stage=completed、last_error=null；候选 PID8016。
- candidate-startup.log：FIRST_CANVAS elapsed_ms=1456。证明渲染，不证明原生窗口可见/前台或持续存活。
- 用户观察更新超过150秒、未自动出现新版窗口；手动打开后是rc.3。启动旧rc.2时已有rc.3却无自动提示。
- helper-preflight累计阶段时间：request 0ms、installation16、original55、helper8041、installer-pair16009、resume31547、ready-installation31562、ready-installer40169、ready-published43652。累计值不能相加；其余总耗时未覆盖。

## 边界与待查项

不直接添加Inno第二启动项：助手已单次启动候选，第二启动会与进程绑定/健康确认冲突。不删除回退/签名/身份/ACL保护。

1. 原生根窗口须绑定同一PID/HWND，验证可见、未最小化、未cloaked、有显示器交集；前台是否允许应分别记录。FIRST_CANVAS不得冒充此证据。
2. 后台检查开始前写入节流时间，错误隐藏后重新打开可能跳过检查；需只对成功检查节流并保留并发锁及失败诊断。
3. 补父端准备/完整证明、正常旧GUI退出、安装、候选启动及健康提交分段计时，再根据证据优化；不把增加等待上限称提速。

## 虚拟 Windows 验证路线

本机未发现现成虚拟机；先复用GitHub Windows VM，不购置或安装付费虚拟化产品。manual desktop-smoke消费现有精确哈希QA包，不构建、不发布、不改变公网。脚本只允许临时GitHub Windows runner，拒绝现有产品注册/快捷方式；缺交互桌面必须失败。

首次仅确认普通GUI窗口和截图能力，**不是完整更新验收**。随后增加真实GUI自动更新端到端：旧版启动发现更新/弹窗，原路径安装，旧PID正常退出，新PID窗口可见且持续存活，项目/快捷方式正常，事务完成；保存截图和每阶段时间。发现桌面不可用时报告环境限制，不标为通过或让用户继续重复测试。

所有新产品源码修复须新source/new build/exact artifact；源码包绑定不能借用旧包。保留现有必需CI及保护门，PR9不合并。新独立QA发布前必须先通过本批原生GUI验收；不能自动根据旧QA确认激活新QA。

首次虚拟机 run37948572097（diagnostic source8d6d7df）已实际安装baseline并尝试启动：session2/user_interactive=true/readable_input_desktop=true；GUI exit2，日志`egui_glow requires opengl 2.0+`。因此不是桌面不存在，而是该runner图形能力不足。此run失败，不计普通GUI或更新通过。后续先在临时runner配置可核验来源的软件OpenGL或其他适配图形环境（仅测试环境，不改变用户包/签名/生产配置），再运行；不能通过删去真实窗口断言变绿。此图形环境问题与用户FIRST_CANVAS/窗口不可见问题不同。

测试环境补充：使用维护者pal1000/mesa-dist-win 26.2.4 MSVC x64发行包，固定70257286 bytes / SHA256 351fc8c8b695878ffb3eaa044b3ead08672a48b1a045e3c3e3975811df0f6695（GitHub发行资产digest）。仅临时测试安装目录增加opengl32.dll/libgallium_wgl.dll并设进程级GALLIUM_DRIVER=llvmpipe；不替换System32、不改安装包/产品EXE、不分发给用户。证据须标明软件渲染测试环境，不声称验证了用户GPU。

run37949546776 source7fb6fa1普通桌面smoke成功：rc2 PID7048可见窗口2066ms、rc3 PID7924可见窗口848ms，两窗口可见/未最小化/未cloaked/有显示器交集，截图人工核对并正常关闭。不是完整updater端到端。

确定的自动提示根因：旧KitOnly脚本两次cargo build均未传--release且复制target/debug；AppUpdateState启动检查对cfg!(debug_assertions)直接return。故旧公网QA包不可能启动自动检查，此前状态级单测漏掉真实构建配置。本批改KitOnly使用release，binary capabilities报告build_profile/startup_update_check_enabled，脚本与QA stage拒绝debug/关闭启动检查证据。Debug组件测试保留，其结果不再冒充Release自动检查。Debug哈希性能可能解释部分慢点，但150s仍需全链路计时，不能推断已提速。

独立复审补充：部署入口validate同样要求release与startup_update_check_enabled=true，拒绝缺失、debug及false，不能让旧Debug artifact借工具桥接进入新交付。旧对象不改，本批必须新source/new build。

后台检查修复：reservation持有跨实例文件锁直至网络及签名检查结束，仅成功检查后原子保存节流时间；失败释放锁但不记成功时间。定向回归验证并发拒绝、失败后允许重试、成功后5分钟节流和损坏记录拒绝。此项尚未在Windows原生执行，不等于完整GUI更新通过。

本批窗口门：实际eframe根窗口的Win32 HWND经当前PID、GA_ROOT、IsWindowVisible、IsIconic、DWM cloaked、正面积及显示器交集核验后，才允许首次画布发布健康回执。候选与恢复共用，停止/绑定/有限健康等待仍保留。仅对既有GUI发一次Visible/Minimized(false)/Focus，不第二次spawn或增加Inno启动项；前台焦点许可不是可见性证明。新增不可见状态和无效HWND拒绝、不可见不写receipt及一次activation组件回归。窗口诊断记录不含nonce/项目内容，也不能授权安装/健康。

自主GUI夹具准备：固定loopback https://localhost:38443/instplot-studio，证书须有localhost SAN；无需改hosts、OS根证书或全局设置。临时git archive带专用标记且必须位于runner临时目录，才允许两个ureq Agent在快照里加载测试CA；仍严格验证TLS链/主机名，不提供disable验证选项，生产checkout及公有QA包不加入补丁。snapshotpatch记录前后源码与公开CA哈希，签名两版metadata后由本地TLS服务先提供baseline，再由私有control token切换candidate，不接触公网对象。该夹具不能冒充公网exactbinary网络证据。当前只实现准备/服务工具和3项快照guard回归；真实GUI输入驱动、TLS证书生成与新Windows运行仍待完成。

新增真实HWND原生测试构造自有根窗口，检查隐藏→显示→最小化→恢复→隐藏以及子窗口、非匹配PID、桌面外来窗口、销毁后句柄拒绝；只正常销毁自己创建的窗口。尚未原生执行，不计Windows通过。

helper-preflight诊断延伸到controller运行阶段：等待旧GUI退出、安装器已启动、新GUI已启动等待可见健康、完成/恢复/失败边界沿同一起始时钟记录。诊断写失败不改变控制流；没有删去慢验证或增加安装重试。父端准备/fullproof与完整真实GUI夹具计时仍待补。本轮Mac全目标全feature Clippy和隔离实际Windows源码/健康UI helper/tests MSVC交叉Clippy通过；不是Windows原生执行，更不是完整GUI通过。

本批现已补齐父端create/resume准备、完整proof耗时的私有诊断；proof freshness在完整校验结束时绑定，诊断写盘不会刷新授权。后台失败记录只有固定check-failed状态和时间，不保存服务端错误/URL/项目/密钥，失败仍可重试。

虚拟机run37956316698/source79c6原生窗口3项逐项通过，包括真实自有窗口显示/最小化/恢复和foreign/child/destroyed拒绝；fixture5项通过。完整GUI随后在快照中文源码写入CP1252编码时失败，尚未启动更新。前一run37955903004的mock源码CRLF失败已修。本批固定UTF-8字节写入并加入中文回归，不改严格TLS或源字节校验。public QA build37955904898/source71870e4的baseline Release编译成功，candidate因临时版本变更未同步Cargo.lock被--locked拒绝；补临时snapshot四个workspace包版本的精确同步，依赖pins不变且candidate保留--locked --offline。失败包不部署，新脚本输入必须准确记录新source/build。

新增手动desktop-e2e job和真实输入脚本：隔离CA与localhost SAN服务证书、严格TLS正反例及私有文件拒绝、真实Release两包、baseline首次检查与可信恢复记录、保留真实5分钟节流后本地切换candidate、启动自动弹窗、绑定前台窗口后Tab/Enter点击、旧PID正常退出、原目录替换、唯一新PID可见持续30秒、恢复项目/快捷方式/版本/事务completed及正常退出。独立复审指出的baseline可见性最终门、Enter前再查焦点、成功证据延后到正常关闭与服务清理后均已落实。夹具5项签名/隔离回归与本地定向测试通过，不代表此完整Windows job已运行。测试快照加载公开fixture CA不是公有QA二进制，公网确切包验证须单独执行。
