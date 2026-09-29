# InstPlot Studio 发布运维清单

> 状态：仓库侧技术 RC 基础设施与 GitHub OIDC→阿里云 STS 信任链已验证；正式更新信任根配置中，商业平台签名尚未配置。

本文只记录实际操作入口和账户侧配置。安全契约、分阶段门禁与验收标准以
`INSTPLOT_STUDIO_GITHUB_OSS_RELEASE_PLAN.md` 为准。

## 1. 当前允许的发布级别

- 当前工作流只允许带 prerelease 段的 SemVer 技术 RC，例如 `0.1.2-rc.1`。
- Windows 安装器当前没有 Authenticode；macOS 当前只做 ad-hoc 签名，没有 Developer ID、公证和 staple。
- 在商业签名流程接入并验证前，`release.yml` 会拒绝 stable 版本，不能把技术演练描述为可信正式发行。
- 应用内生产更新入口尚未启用；仓库中只有独立的清单、签名、URL 边界和验证原型。

## 2. GitHub Environment

创建受保护 Environment：`release-production`。建议设置人工批准、仅允许 `main`、禁止绕过，并配置：

Secrets：

- `UPDATE_SIGNING_PRIVATE_KEY`（当前 Ed25519 PEM 私钥；不要添加下一把私钥）

Variables：

- `OSS_BUCKET`
- `OSS_REGION`
- `OSS_ENDPOINT`，例如 `https://oss-cn-beijing.aliyuncs.com`
- `OSS_PUBLIC_ROOT`，必须是固定 HTTPS 产品根，例如 `https://download.example/instplot-studio`
- `ALIYUN_OIDC_PROVIDER_ARN`
- `ALIYUN_ROLE_ARN`
- `UPDATE_KEY_ID`
- `UPDATE_PUBLIC_KEY_HEX`（32 字节，即 64 个十六进制字符）
- `UPDATE_NEXT_KEY_ID`
- `UPDATE_NEXT_PUBLIC_KEY_HEX`

当前和下一把公钥必须在构建 RC 前同时固化到应用；对应私钥离线生成并备份，只有当前私钥进入 Environment。

OSS 身份使用 GitHub OIDC 换取阿里云短期 STS 凭据，不保存长期 AccessKey。OIDC
角色的 `oidc:sub` 必须精确限制为仓库当前启用的稳定 ID 格式：
`repo:zhiyuzhang001-a11y@235626644/InstPlot-Studio@1379312831:environment:release-production`，角色权限只允许
`instplot-release/instplot-studio/*` 的 `GetObject` 和 `PutObject`。

## 3. 首次 RC 的准备顺序

1. 冻结 OSS Public Root、Bucket、地域和缓存/保留策略。
2. 离线生成当前及下一把 Ed25519 密钥，核对 key ID 与公钥。
3. 配置 `release-production`，但不在日志、Issue、PR 或源码中粘贴私钥。
4. 把 Cargo 版本提升为新的不可变 RC，例如 `0.1.2-rc.1`，新增对应 `CHANGELOG.md` 段。
5. 将固定 Public Root、当前/下一把公钥编译进客户端，完成更新 UI 和本地恶意 HTTP 服务测试。
6. 合并 PR，确认 required checks 全部通过。
7. 在干净且与 `origin/main` 完全一致的本地 `main` 上运行 `scripts/release.sh --dry-run`。
8. 明确批准后运行实际 dispatch；本地脚本随即退出，不持续轮询 GitHub。
9. GitHub Release 技术 RC 验证通过后，再以受保护 Environment 运行 OSS 发布。

## 4. 工作流职责

- `instplot-studio.yml`：普通 PR/main 三平台质量门。
- `verify-aliyun-oidc.yml`：只验证 GitHub OIDC 能否换取短期阿里云 STS 凭据；不读取签名私钥、不写 OSS。
- `release.yml`：冻结同一 source SHA，构建三平台包，真实安装/运行/卸载，核对草稿资产后发布 GitHub prerelease。
- `publish-oss-update.yml`：从既有 GitHub Release 重新下载并核对资产，签名新 metadata revision，先验证不可变公网对象，最后只切换目标 channel。
- `refresh-update-metadata.yml`：不改版本和 Release，只请求新的递增 sequence/expiry，并转交受保护 OSS 工作流。

发布工作流不会持续等待 OSS。OSS 是独立运行，可在 Actions 页面低频查看或交给静默 heartbeat 监控。

## 5. 不可变与恢复规则

- 已公开 tag、Release、版本化包和 metadata revision 永不覆盖。
- draft 恢复只接受相同 tag、相同 source SHA、相同资产字节；不同摘要立即停止。
- OSS 版本化对象只允许首次创建；已存在即停止。
- 只有 `channels/stable/latest.json` 或 `channels/prerelease/latest.json` 是可切换指针。
- RC 只改变 prerelease；stable 只改变 stable。工作流在切换前后逐字节检查另一个 channel。
- metadata refresh 必须使用更大的 `release_sequence`，并引用同一不可变 Release 资产。
- 清单有效期最长 120 天；运维目标是在剩余 30 天前运行 metadata refresh，避免客户端因过期清单无法确认更新状态。

## 6. 仍需用户/账户侧完成

- OSS Public Root、Bucket、地域与 OIDC RAM 最小权限已配置；只读短期凭据探针已于 2026-09-29 通过。Bucket 版本控制/WORM、缓存和费用/保留策略仍须在首次发布前复核。
- 正式当前/下一把 Ed25519 信任根已生成并配置工作副本；首次 RC 前仍须完成离线备份、公钥入客户端及一致性检查。
- Apple Developer ID、notarization 凭据。
- Windows Authenticode 证书或受控云签名服务。
- 至少一台非开发 macOS、Windows、Linux 设备的最终黑盒安装验收。

在这些条件完成前，最高状态只能是仓库侧原型或 unsigned technical RC，不能标记为
`TRUSTED_STABLE_RELEASE_COMPLETED`。
