# Velopack 固定版本可行性审计

> 归档状态（2026-10-10）：此次评估未进入集成；后续继续修复并验收现有更新机制，已发布公开 rc.3 技术 prerelease。本文保留固定版本源码审计结论，不是当前迁移任务或新的授权；见[发布收尾记录](INSTPLOT_STUDIO_RC3_RELEASE_CLOSEOUT.md)。

日期：2026-10-09。结论：适合通用安装更新，但不是现有 Studio 更新安全/恢复契约的直接替代品。当前能力门不通过，不进入实际安装原型。

## 方法与范围

主 agent 从官方仓库克隆 tag `1.2.161`，确认为 commit `92d6a1c91716729d449034df5c50307dcce39493`。独立 agent 对同一固定版本重新核验执行、退出和恢复路径；二者一致。仓库发布元数据显示发布日期为 2026-09-29。

这是源码审计，不是 Windows 原生执行或 GUI 测试。没有修改第三方源码，没有安装 `vpk`/.NET 或应用，没有下载生产私钥，没有触发构建/发布。MIT 许可仅解决使用许可，不意味着功能和安全契约相同。

## 1. 强制终止其他实例：硬阻断

Windows `apply_package_impl` 在解包及可选 obsolete hook 后，无条件执行 `shared::force_stop_package(&root_path)`（第 159 行）。`HookRunMode` 只影响 hooks，不包围这一调用。

`force_stop_package` 枚举安装根内的运行进程，除当前 updater 自身外调用 `process::kill_process`；后者使用 `TerminateProcess`。这不是普通退出请求，也不限于最初发起更新的父进程。

先保存并退出主 GUI 并不能保证另一个安装实例不存在。因此薄适配调用 `wait_exit_then_apply_updates` 不能证明“不强杀”。未发现本次受审执行路径具有官方关闭此行为的配置；没有穷尽所有历史/未来版本。

证据：

- [Windows apply，固定源码](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/bins/src/commands/apply_windows_impl.rs#L151-L163)
- [目录进程终止](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/bins/src/shared/util_windows.rs#L81-L102)
- [原生 TerminateProcess](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/lib-rust/src/process_win.rs#L427-L436)

## 2. 退出等待：不是失败即停止

SDK 文档说等待 60 秒后放弃，但 updater 的 `operation_wait` 返回 `()`，等待错误只记日志 `Continuing...`。上层 `apply` 随后进入 `apply_package_impl`，没有根据等待结果中止。这与现有“不能确认退出就不安装”的契约不同。

此结论来自实际控制流；超时如何返回仍需原生故障测试，不能将未执行的测试标成通过。

- [等待逻辑](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/bins/src/shared/util_common.rs#L14-L26)
- [等待后直接应用](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/bins/src/commands/apply.rs#L24-L47)

## 3. GUI 健康失败恢复及备份保留：硬阻断

Windows 执行器将旧 current 目录改名到临时旧目录，并替换新目录；随后运行可选 updated hook、清理临时新旧目录，再返回成功。外层此后才 `start_package` 启动新版。

因此旧目录清理早于新版项目加载/首画布健康确认，不提供现有“健康成功前保留备份、失败自动恢复”的门。updated hook 不是 Studio 真实 GUI 健康回执。外层也会在 apply 返回错误时尝试启动应用，但不能据此推断旧文件一定已恢复。

末尾在传播 `action?` 前也清理临时目录；不能只引用注释中的 rollback 声称原子恢复可靠。实际中断与恢复行为未在 Windows 测试。

- [交换及清理顺序](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/bins/src/commands/apply_windows_impl.rs#L160-L239)
- [传播结果前的清理](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/bins/src/commands/apply_windows_impl.rs#L270-L280)
- [应用后才重启](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/bins/src/commands/apply.rs#L45-L63)

## 4. 包摘要不等于签名发布身份：需独立适配

静态 HTTP `HttpSource` 下载并反序列化 feed；包验证检查大小和 SHA-256，字段缺失时回退 SHA-1。该标准路径没有现有 Ed25519 签名清单认证及按源持久序列水位。

`AllowVersionDowngrade=false` 比较已安装版本与远端版本，不等价已见发布清单的序列防回退。不能因为存在 SHA-256 和 HTTPS 就宣布发布者签名合同已满足。

可研究自定义受信 `UpdateSource` 复用现有元数据验证，但还需证明选包、下载、缓存与应用时的绑定和篡改边界；不能默认这是零成本。即使解决 feed 认证，也不能消除上述终止和健康恢复差距。

- [静态 HTTP source](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/lib-rust/src/sources/http.rs#L40-L74)
- [包大小与摘要校验](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/lib-rust/src/manager.rs#L537-L560)
- [版本降级选项](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/lib-rust/src/manager.rs#L120-L141)

## 5. 安装目录与现有 Inno 迁移

Setup 有 `--installto` 参数，因此不能笼统说不支持自选目录。不过 Velopack 使用 root/current 等自有布局和安装身份，不等于现有 `D:\InstPlot Studio\instplot-studio.exe` 布局可无感覆盖。迁移仍须专用安装/注册/快捷方式及项目配置验证。

- [Setup 参数](https://github.com/velopack/velopack/blob/92d6a1c91716729d449034df5c50307dcce39493/src/bins/src/setup.rs#L96-L104)

## 决策与未完成项

已完成独立审查、固定源码取证、能力差距记录和计划修订。SDK 编译、双版本打包、空间测量、Windows 升级/故障实测、Studio 接入、OSS 及旧安装迁移均未执行。

不建议未经新决策直接接入官方原版：保持原契约会涉及 updater fork/额外控制器，违背此次减少自建复杂度的目的。也不擅自把自动健康恢复改成手动重装。

用户需要明确路线：继续寻找满足原契约的框架，或逐项批准新的通用更新契约（特别是其他实例终止行为和健康失败恢复方式）。任何情况下仍保留未保存工作保护、可信发布认证、用户数据隔离及生产启用审批。

此前“1–2 天原型，再 3–5 天集成”的估算不涵盖这些能力差距，当前不再作为有效交付预算。能力门未通过时不触发昂贵 CI，不继续为了展示进度构建无法安全启用的安装包。
