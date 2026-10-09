# Windows GUI 实机问题与无人值守验收

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
