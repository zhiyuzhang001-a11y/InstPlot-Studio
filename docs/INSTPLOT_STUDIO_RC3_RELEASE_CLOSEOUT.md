# InstPlot Studio rc.3 发布与更新机制收尾

记录日期：2026-10-10（北京时间）。本文是已完成工作的证据索引，不是新发布或迁移授权。

## 当前状态

- 公开版本为 `0.1.2-rc.3`，属于技术 prerelease，不是可信 stable。
- [产品 PR9](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/pull/9) 已正常合并，merge SHA `21ca68de7a0ee21a8dafe1c870155d0ee81e47b3`。
- [发布准备 PR19](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/pull/19) 已正常合并，冻结发布源码为 `1f44301875a3e528707dfc1bc23e201e6dd5e16a`。
- 合并前准确 head 四项必需检查成功、无未解决审查；未使用管理员绕过、强推或降低验证门。
- [GitHub Release](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/tag/v0.1.2-rc.3) 为非草稿 prerelease，targetCommitish 与冻结源码相同。
- 受保护 OSS prerelease 渠道已激活 rc.3；stable latest 发布前后均为 HTTP 404，未创建或激活 stable。
- 生产当前/下一把公钥保持不变；未公开测试私钥、临时 CA、私有事务材料或 rc.4 测试候选。

## 构建、发布与独立公网核验

[Release run38044627921](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/38044627921) 从冻结源码构建三平台包，Linux 安装、Windows 安装/卸载、Mac 打包/挂载及草稿资产核验成功，随后发布 GitHub prerelease。

[OSS run38045238162](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/38045238162) 通过现有 `release-production` Environment 和 GitHub OIDC 消费同一次 GitHub Release 的资产，没有为上传重新构建。先创建并验证不可变对象，再切换 prerelease latest，最后复验公网和另一个渠道隔离。

发布后另行运行 `scripts/verify_public_release.py`：配置两把生产公钥，验证发布者签名，并调用实际 Rust 客户端进行完整清单语义校验；重新下载四个平台包，逐项核对大小和 SHA-256。不是仅用通用 Ed25519 库验签，也不是只看 CI 绿色或上传退出码。

- 产品根：`https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio`。
- channel：`prerelease`；version：`0.1.2-rc.3`；release_sequence：`3`。
- expires_at：`2027-01-08T00:00:00Z`，不超过 120 天限制。
- latest 与 `releases/0.1.2-rc.3/metadata/3/manifest.json` 逐字节相同。
- 已验证 manifest SHA-256：`e8529018d58a6d61d47b1091890e6e87ac0800a6999684c8a397f8324a74b098`。
- Windows 签名清单的完整 `windows_in_place` descriptor 与 Release sidecar 的 `contract` 相同，安装器摘要与 sidecar 的 installer_sha256 相同。
- 同一冻结源码、Release 资产摘要与 OSS run 共同形成来源链；签名清单本身不含 source SHA/run 字段，不能声称签名直接认证了这些不存在的字段。

独立核验回执是本地取证摘要，不是新增的公开服务器 receipt 协议。公开可复核入口为上面的 Actions、不可变 Release 资产、签名元数据与校验和文件。

## 公开资产

严格集合为四个平台包、`windows-in-place.json` 和 `SHA256SUMS.txt`，共六文件。[公开校验和](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/download/v0.1.2-rc.3/SHA256SUMS.txt) 覆盖五份数据资产。

- Windows：`InstPlot-Studio-0.1.2-rc.3-windows-x86_64-setup.exe`，6,763,093 bytes；SHA-256 `c186ca019ee2949f0c2951592e463c4de7a950f850d65628a2e52605dc8b4562`。
- macOS arm64：`InstPlot-Studio-0.1.2-rc.3-macos-aarch64.dmg`，7,152,994 bytes；SHA-256 `0190637fd2b43f0fac2a134ea20b337b814d167127b7707ba208b4ad54071eec`。
- Linux deb：`InstPlot-Studio-0.1.2-rc.3-linux-x86_64.deb`，6,222,916 bytes；SHA-256 `cb896255cb7b81315e058bedb6a006827560dbae748ee5048fe1d6bd54711615`。
- Linux portable：`InstPlot-Studio-0.1.2-rc.3-linux-x86_64.tar.gz`，7,430,353 bytes；SHA-256 `35952b1b10004f03c30b2a8fad30dba7e469bc7d2336c9caf648e16417989012`。
- Windows sidecar：`windows-in-place.json`，534 bytes；SHA-256 `88f3a50b516a441de9f4480924593b94f9ea27385ea5d2a4458de63dbb69250b`。
- 校验和文件：540 bytes；SHA-256 `196e523a08b003c28f4a3abd0c0f3e40797a6c6445c43735298a60e56780cd46`。

Windows descriptor 的固定 executable SHA-256 为 `cef7c7637eceda37a1524cfc4c5e191af0bfadb0dad2919fe5481f82ccfd50be`，LICENSE SHA-256 为 `b2f74e5f3d905d0eea1c315d189ffcaadc64c1d1a151e9ac5c173c66068cefe1`。两者不是安装器摘要。

## GUI 证据及跨 SHA 复用边界

### Windows

[完整 GUI run38042762948](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/38042762948) 实际 tested SHA 为 `0048959384a0cac652e8aed65447b5b63f463350`。隔离 Release/TLS 测试为 rc.3 → rc.4，rc.4 只在测试源码快照内使用，未发布。

实际自动弹窗、一次点击、旧进程正常退出、原目录安装、唯一新版原生窗口、原项目正常编辑器、健康 completed/null error、项目/快捷方式/版本绑定、30 秒持续可见及原生候选 exit code 0 全部通过。截图人工核验；不以 FIRST_CANVAS、进程存在或事务 completed 单独充当正常 GUI 证据。

测试使用临时 Mesa 软件 GPU 与 loopback TLS，点击到 completed 约 8.85 秒不是公网性能保证。此前 rc.2 → rc.3 隔离 QA 的用户实机反馈“约 5 秒以内”单独记录在[工程复盘](WINDOWS_UPDATER_ENGINEERING_PLAYBOOK.md)，也不是自动性能采样或本次公开包用户验收。

### macOS

实际 tested SHA 为 `d05ec134b8d6ebdf1e042eb3e26dbad0c2ca2ee4`。使用独立 bundle/cache、临时双钥和严格 localhost TLS 的 Release DMG 测试 rc.3 → rc.4；未改系统 CA、hosts 或生产信任，未关闭证书验证。

完整自动提示/单次点击下载准备/正常旧退出/原位替换/自动重启、原项目正常编辑器、真实健康绑定、唯一 GUI 持续 30 秒、项目摘要不变、PNG/PDF 导出与严格 codesign 均通过。另以健康回执路径为目录制造真实候选写入失败，观察 helper 非零/rolled_back、原 bundle/项目恢复、失败候选保留及旧 GUI 自动恢复并持续 30 秒。不是下载失败，也没有伪造健康回执。

正常 GUI quit 后候选/恢复进程消失；未保留这些非子进程的实际退出码，不宣称它们 exit code 0。原始 journal/health、非敏感日志、项目和截图保留为本地证据；不提交机器路径、进程身份、nonce、私钥或源码构建快照。

### 输入等价性

`0048959` → `d05ec13` 仅修原型锁文件和 Mac 测试 fixture/测试脚本，没有改产品、Windows GUI 驱动、打包、信任或公开配置输入；Windows 证据按这些相关输入相同复用，明确记录实际 tested SHA。`d05ec13` → 冻结 merge SHA `1f443018` 的树差异为空。Mac 证据来自实际 d05 源码。

隔离签名 TLS GUI 测试不等于生产公网 exact binary 的端到端更新网络测试。发布资产的真实平台包门、公开签名/下载校验和 GUI 证据分别陈述，不混算。

## 公开 Mac 包与本地安装

公开 DMG 已只读挂载、复制后验证 strict codesign、生产 bundle ID `com.instplot.studio`、rc.3 与 arm64；正常卸载，无强制卸载或系统安全绕过。公开 binary SHA-256 为 `1b183750ace4894798fef6b94a1d46fb0871933a5df5a276aa39835a38c70cb1`。

复制出的准确公开包实际打开测试项目，窗口正常编辑器与底部 rc.3 可见；正常 GUI quit 后进程消失，项目 SHA-256 `43223070873628d1b423f0c1214f5f3ae02e3f422f095338b4f01ebe017b5e1a` 未变。不宣称未保存的退出码。

随后经用户授权，Mac 正式本地安装从公开 rc.2 覆盖为相同公开 rc.3，原安装位置不变，已再次实际核对正常窗口与版本；未改用户项目/配置。Windows 公开安装包交由用户操作，尚无本聊天内的新公开包实机安装确认，不推定已完成。

## 维护与未完成项

- Mac ad-hoc 未公证；Windows 无 Authenticode。商业签名、非开发设备覆盖和 trusted stable 仍是独立事项，不因 rc.3 成功自动完成。
- 隔离 QA 不能自行换用生产信任；迁到公开渠道需一次正常退出后覆盖安装。后续自动更新仍受候选签名、可信恢复资产、正常退出与健康门约束。
- 持续维护 metadata 的有效期与递增序列；续期复用同一不可变资产，不为续期重建或覆盖版本。
- [CI 分层验证策略](CI_VALIDATION_POLICY.md) 已归档，但分类器、缓存和 required checks 优化尚未实施；不声称 CI 已加速。
- [Velopack 评估](INSTPLOT_STUDIO_VELOPACK_FEASIBILITY.md) 与[历史计划](INSTPLOT_STUDIO_VELOPACK_MIGRATION_PLAN.md) 归档，不恢复迁移任务；当前继续维护已验收的现有更新机制。
- 本地已将可再生 Rust 编译产物移入废纸篓，保留源码、测试、开发配置、发布包与验收证据；没有删除项目 Git 历史或恢复资料。该清理不改变 GitHub 代码，首次重编译会重新生成缓存。
- 本次发布的低频监控任务已结束并删除，不持续监听，不恢复旧监控。

本收尾文档不重新创建 tag、Release、安装包、OSS 对象或更新指针，也不新增运行权限。
