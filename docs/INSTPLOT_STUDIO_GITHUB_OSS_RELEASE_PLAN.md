# InstPlot Studio GitHub + OSS 自动发布与更新计划

> 后续方案（2026-10-01）：应用内原位替换、重启与恢复见 `INSTPLOT_STUDIO_IN_PLACE_AUTO_UPDATE_PLAN.md`（待确认、未实施）。本文件的“不实现自动覆盖”是原发布阶段边界，不代表新方案已完成；已有签名发布契约继续有效。

> 状态：`IN PROGRESS — OIDC VERIFIED; TRUST ROOT AND CLIENT IMPLEMENTATION`
> 制定日期：2026-09-29
> 当前发布基线：`v0.1.1`
> 目标首个演练版本：`v0.1.2-rc.1`
> 配套总规范：用户提供的《GitHub Actions + 阿里云 OSS 自动发布流程》

> 实施状态（2026-09-29）：Stage 0 仓库治理已完成；Stage 1 的清单/签名原型、Stage 3 技术打包定义、Stage 4/5/7 工作流骨架已实现。OSS Public Root 已冻结，GitHub OIDC→阿里云 STS 真实短期凭据探针通过，当前/下一把正式 Ed25519 工作密钥已生成；Stage 1 尚待离线备份和客户端公钥一致性门，Stage 2 生产更新入口正在实施，Stage 8 真实 RC 尚未开始。操作配置见 `INSTPLOT_STUDIO_RELEASE_OPERATIONS.md`。
> 本文件职责：把通用规范落实为 InstPlot Studio 的产品边界、代码边界、阶段门和验收标准。

本文件是 InstPlot Studio 的项目级约束。通用总规范提供可复用原则和示例；两者冲突时，以本文件冻结的阶段范围为准。本阶段不会实现通用规范示例中的“客户端启动安装程序、自动覆盖和重启”，只验证安装器自身具备用户手动执行的安装与升级能力。

## 1. 目标

建立一条不依赖开发电脑持续在线的正式发布链路：

```text
功能分支
  → Pull Request
  → Windows / macOS / Linux 质量检查
  → 合并 main
  → 本地一条 release 命令
  → GitHub Actions 构建并真实验证三平台发布资产
  → GitHub Release
  → 生成并签名 OSS 更新清单
  → 版本化上传 OSS
  → 公网重新下载并验证
  → InstPlot Studio 检查并提示新版本
```

完成后应达到：

1. GitHub 是源码、版本、CI、Release 和发布记录的权威来源。
2. OSS 是面向公网、尤其是中国大陆网络环境的稳定下载入口。
3. 任意电脑可从 GitHub Release 或 OSS 下载经过验证的对应平台安装包。
4. 用户点击应用左下角产品版本即可检查更新，看到版本和说明并下载正确资产。
5. 私钥、OSS 写凭据和平台签名凭据不进入源码、安装包、日志或公开产物。
6. 发布成功以真实安装、运行和公网文件验证为准，而不是只以编译成功为准。

## 2. 当前基线与明确缺口

### 2.1 已有能力

- 根 `Cargo.toml` 是预期的产品版本权威来源，当前版本为 `0.1.1`；现有 CI 和测试仍有少量 `0.1.1` 硬编码，必须先消除或改为受控的一致性检查，才能宣告真正单一来源。
- `.github/workflows/instplot-studio.yml` 已在 Windows、macOS、Linux 执行格式、测试、Clippy、release 构建和真实命令行工作流。
- `scripts/install_studio_macos.py` 可以在本机生成、ad-hoc 签名、替换和验证唯一 Spotlight App。
- GitHub 已有 Pull Request、tag、源码 Release 和 `CHANGELOG.md` 流程。
- 左下角只显示产品名和版本，当前点击后打开 GitHub Latest Release。
- 已有项目检查、数据导入、PDF/PNG/SVG 导出等适合作为打包后 smoke test 的真实命令入口。

### 2.2 尚未具备

- 没有正式的 Windows 安装包定义。
- 没有面向公开分发的 macOS DMG、Developer ID 签名和 notarization 流程。
- 没有 Linux 系统包和便携包。
- 没有独立的发布级 `release.yml`，当前 CI 不会生成和发布三平台资产。
- 没有 `scripts/release.sh`、`scripts/read-version.sh`。
- 没有 OSS staging、清单签名、上传和公网验证程序。
- 没有应用内 HTTP 更新检查、Ed25519 验签、版本比较和平台资产选择模块。
- 没有真实 OSS Bucket、RAM 最小权限、Secrets、Variables 和公网域名配置。
- GitHub `main` 当前没有分支保护；现有 CI 的路径过滤也未覆盖将新增的 `packaging/**` 和发布工作流。

## 3. 本阶段范围与非目标

### 3.1 本阶段包含

- 三平台可交付资产的制作和真实验证。
- GitHub 发布级 workflow、一条命令触发和 Release 资产发布。
- 多平台 OSS 更新清单、Ed25519 签名、原子上传和公网验证。
- 应用内手动检查更新、更新结果窗口、受校验的安装包下载和对应平台保存入口。
- 更新模块的离线、超时、安全、版本和平台边界测试。
- 一次完整的 `v0.1.2-rc.1` prerelease 演练；正式版使用新的不可变版本 `v0.1.2` 重新构建、验证和发布，不把 RC 原地改名或覆盖为正式版。

### 3.2 本阶段明确不包含

- 应用自行退出、覆盖旧程序、提权安装和自动重启。
- 静默后台自动检查；首版只在用户主动点击版本入口时联网。
- 差分更新、增量补丁、自动回滚已安装程序。
- 自建 Web 服务、账户系统、遥测、下载统计或用户身份信息。
- App Store、Microsoft Store、Snap、Flatpak 等商店分发。
- 在没有证书时把 ad-hoc macOS App 或未签名 Windows 安装包描述为“可信正式签名”。

自动覆盖安装属于后续独立项目。第一阶段的“更新”含义是：安全发现新版本、展示说明、由应用下载并验证安装包、保存到用户选择的位置，由用户完成安装。浏览器只能作为查看 Release notes 或故障回退入口，不能用浏览器下载替代应用对最终文件的大小和 SHA-256 校验。

## 4. 冻结的发布与更新契约

### 4.1 权威关系

- 版本唯一来源：根 `Cargo.toml`。现有 `.github/workflows/instplot-studio.yml`、`apps/instplot-studio/src/lib.rs` 和 `scripts/validate_b1.py` 等消费者必须从 Cargo metadata/read-version 派生，或由一项自动同步检查证明与根版本一致；不得在每次发布时人工散改多个版本字面量。
- 发布说明唯一来源：`CHANGELOG.md` 的对应版本段，发布 workflow 可据此生成或提取 Release notes。
- Git tag、Cargo 版本、安装包元数据、文件名、GitHub Release 和 OSS 清单版本必须完全一致。
- Release 资产只能来自同一次 GitHub Actions 运行产生并通过验证的 artifacts，不从开发电脑临时补传。
- 已发布 tag、Release 和版本化 OSS 资产不可覆盖；代码变化必须提升版本。
- RC 使用完整 SemVer，例如 Cargo `0.1.2-rc.1`、tag `v0.1.2-rc.1` 和 prerelease Release；正式版必须提升为 Cargo `0.1.2`、tag `v0.1.2` 并重新构建，不能把 RC 的 tag 或二进制原地晋级。
- 本地 dispatch 前确认本地 HEAD 等于当时的 `origin/main`。release workflow 只接受 `github.ref == refs/heads/main`，并在事件触发时冻结唯一 `source_sha=${{ github.sha }}`；所有 checkout、构建、attestation、tag 和 Release 都显式指向该 SHA。运行期间可以要求该 SHA 仍是 main 祖先，但不得要求它继续等于已经前进的实时 main tip。

### 4.2 首版多平台资产标识

统一使用稳定的平台键：

```text
windows-x86_64
macos-aarch64
macos-x86_64       # 仅在项目确认继续支持 Intel Mac 后构建
linux-x86_64
```

一个平台可以有多个包型。`platforms[platform]` 必须包含 `preferred` 和 `packages[]`；应用默认推荐 `preferred` 指向的包，同时允许用户查看该平台的其他包。每个 package 至少包含：

- 下载 URL；
- SHA-256；
- 字节大小；
- 文件名；
- 包类型；
- 可选的最低系统版本。

运行时必须精确匹配 OS 与架构。未知平台、缺失资产或不受支持架构只显示明确说明，不退回到另一个平台的包。Linux 的首选项由 Stage 0 冻结为 DEB 或便携包；macOS 对外下载的首选项是 DMG，`.app` 只是 DMG 内部和打包测试对象。

### 4.3 OSS 清单契约

通用规范中的单一 `installer_url` 是 Windows 示例。InstPlot Studio 从首版使用可表达“多平台、多包型”的 `platforms`，避免后续 schema 迁移：

```json
{
  "schema": 1,
  "product": "instplot-studio",
  "version": "0.1.2-rc.1",
  "channel": "prerelease",
  "release_sequence": 1,
  "published_at": "2026-10-01T00:00:00Z",
  "expires_at": "2026-10-31T00:00:00Z",
  "key_id": "instplot-update-2026-01",
  "signature_url": "https://<public-root>/releases/0.1.2-rc.1/metadata/1/manifest.json.sig",
  "notes_url": "https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/tag/v0.1.2-rc.1",
  "platforms": {
    "windows-x86_64": {
      "preferred": "installer",
      "packages": [
        {
          "id": "installer",
          "url": "https://<public-root>/releases/0.1.2-rc.1/InstPlot-Studio-0.1.2-rc.1-windows-x86_64-setup.exe",
          "file_name": "InstPlot-Studio-0.1.2-rc.1-windows-x86_64-setup.exe",
          "package_type": "installer",
          "sha256": "<64 lowercase hex>",
          "size_bytes": 12345678
        }
      ]
    }
  }
}
```

- 发布程序一次生成版本化 `manifest.json`；`channels/<channel>/latest.json` 必须是该文件的字节完全相同副本，不能重新序列化。
- Ed25519 签名对象是这份 manifest/latest 的原始字节，签名以原始 64 字节保存于不可变路径 `releases/<app-version>/metadata/<release-sequence>/manifest.json.sig`；同目录保留字节完全相同的 `manifest.json`。
- `latest.json` 内的 `signature_url` 指向对应不可变 metadata revision 签名。客户端只能先对未信任的 `signature_url` 做严格 URL 限界并下载签名，然后依次尝试全部内嵌、未撤销公钥；某把公钥验签成功后，才完整解析清单，并要求签名覆盖的 `key_id` 与成功公钥严格一致。`key_id` 指向 A、实际由 B 签名必须拒绝。
- 不存在需要与清单同步覆盖的 channel signature，从而保证每个 channel 激活阶段只变更一个 latest 对象。
- 清单生成后不得重新格式化、改变编码、压缩方式或换行。
- `latest.json` 只接受 HTTPS URL，且所有资产 URL 必须位于编译进应用的允许前缀之下。
- 应用只接受已支持 schema、匹配 product/channel、有效版本、有效签名和当前平台资产。channel 由 Cargo SemVer 是否含 prerelease 段在构建时确定并固化：正式版本只查询并接受 `stable`，RC 版本只查询并接受 `prerelease`；不得通过 feature、环境变量或 CLI 切换，RC 发布绝不能修改 stable channel。
- URL 必须结构化解析：固定 `https`、精确 host/port 和规范化 path 前缀，拒绝 userinfo、fragment、反斜杠和编码穿越；每一次重定向都重新检查并限制总跳数。`notes_url` 使用独立的 GitHub allowlist。
- 不允许降级；相同版本显示“已是最新版”。预发布比较遵循 SemVer。
- 清单包含按 channel 单调递增的 `release_sequence`、`published_at`、合理的 `expires_at` 和 `key_id`。客户端在应用配置目录按 channel 保存已验签的最高 sequence，拒绝后续更低值；首次安装仍依赖 HTTPS 和正确缓存，静态 OSS 无法完全消除首次使用时的旧清单重放风险，文档必须诚实说明这一限制。
- 已过期清单不能用于宣告“已是最新版”或下载资产；客户端应提示无法确认更新状态，并提供受 allowlist 限制的 GitHub Release 回退入口。Stage 1 必须冻结过期窗口和续期提前量。
- 元数据续期不修改应用版本、Git tag、GitHub Release 或既有资产。它为同一 app version 创建更高 sequence 的新目录 `releases/<app-version>/metadata/<new-sequence>/`，引用原有不可变资产，预验证后只更新对应 channel 的 latest。历史 metadata revision 不覆盖；latest 与所指 revision 的 manifest 始终逐字节相同。
- 应用至少预埋当前公钥和下一把轮换公钥，按 `key_id` 选择；新公钥必须随旧可信版本提前发布。计划同时记录正常轮换、旧键撤销和私钥泄漏的应急步骤。
- 清单和安装包分别设定明确的最大字节数、Content-Type/Content-Encoding 规则；超限或类型异常在写满内存/磁盘前终止。

### 4.4 OSS 原子发布顺序

```text
1. releases/<app-version>/全部版本化安装包
2. releases/<app-version>/metadata/<release-sequence>/manifest.json
3. releases/<app-version>/metadata/<release-sequence>/manifest.json.sig
4. 从公网预验证全部版本化安装包、manifest 和 signature
5. 将与版本化 manifest 字节完全相同的内容覆盖为 channels/<channel>/latest.json
6. 从公网重新验证 channel 指针及其引用对象
```

`channels/<channel>/latest.json` 永远最后上传，它是激活该 channel 新 metadata revision 的唯一一步。stable 和 prerelease 分别使用串行并发组；同一 channel 不允许两个发布同时覆盖 latest 入口。

- 上传任何版本化对象前必须执行远端存在性检查；已存在即失败，不使用 `-f`，并尽可能用 Bucket 版本控制/WORM 或 RAM 条件强化不可变性。
- 只有 channel latest 允许受控覆盖；覆盖前保存上一份经过验证的原始字节。
- 版本化对象使用长期缓存和 `immutable`；channel latest 使用 `no-cache, max-age=0, must-revalidate` 和 ETag。客户端可使用条件请求；发布验证使用 cache-bust。回滚还必须刷新或失效 CDN/代理缓存。

若公网验证失败：

- 工作流整体失败；
- 不删除任何历史版本化资产；
- 如果 stable 已被激活，恢复上一份已经验证的原始版本化 manifest 字节；它会继续引用该版本自己的不可变签名，禁止重新序列化再签；
- 不修改或重建同名 Git tag/Release。

### 4.5 客户端行为

点击左下角产品版本后：

```text
显示“正在检查”
  → 下载 latest.json 原始字节
  → 严格限界未信任的 signature_url 并下载版本化签名
  → 依次尝试内嵌公钥验证原始字节
  → 完整解析并确认 channel、key_id、来源和大小上限
  → 解析并校验清单
  → 比较 SemVer
  → 选择当前 OS/架构资产
  → 显示“最新版”或更新窗口
  → 用户选择保存位置并点击下载
  → 应用流式下载到同目录的权限受限临时文件
  → 校验最终字节数和 SHA-256
  → 成功后原子改名并允许“在文件夹中显示”
```

- 网络和验签工作不得阻塞 egui 主线程。
- 同一时刻只允许一个更新检查；重复点击把已有更新窗口置顶。
- 超时、断网、服务器错误、签名错误和平台不支持必须用用户可理解的中文说明，不显示内部错误堆栈。
- 更新失败不影响绘图、保存、导入、导出和退出。
- 检查更新本身不自动下载安装包；只有用户明确点击下载后才传输资产。应用不自动运行安装器，不在启动时联网，不收集遥测。
- 下载支持进度和取消；失败或取消清理临时文件，应用退出时安全收尾后台任务。下载成功后仍由用户自行运行安装包。
- 更新 URL、公钥和允许前缀是正式构建常量。测试只能使用构造器依赖注入和 `#[cfg(test)]`，不得提供可进入 `--all-features` 正式二进制的运行时 URL/公钥覆盖 feature，也不读取生产环境变量或命令行参数改变信任根。
- 更新窗口遵循现有窗口置顶和单实例规则，并具备键盘焦点、取消和可访问标签；后台状态变化显式请求 egui repaint。

## 5. 仓库目标结构

```text
.github/workflows/
├── instplot-studio.yml              # PR/push 三平台质量门
├── release.yml                      # 发布级构建、安装验证、Release
├── publish-oss-update.yml           # OSS 签名、上传、公网验证
└── refresh-update-metadata.yml      # 不改应用版本的受保护 metadata 续期
packaging/
├── windows/                         # Windows installer 定义与验证
├── macos/                           # App/DMG、签名和 notarization
├── linux/                           # DEB/便携包定义与验证
└── update/
    └── prepare-oss-release.py       # 多平台清单与 Ed25519 签名
scripts/
├── release.sh                       # 本地只检查并 dispatch
├── read-version.sh                  # 读取 workspace.package.version
└── verify-public-release.py         # 公网签名、大小、哈希验证
apps/instplot-studio/src/update/
├── mod.rs                           # 公共更新服务接口
├── manifest.rs                      # schema 与严格验证
├── version.rs                       # SemVer 和升级判断
├── platform.rs                      # OS/arch 资产选择
└── client.rs                        # 有界 HTTP 获取与验签
```

实际实现时可根据现有模块边界调整文件数量，但不得把网络、验签和版本比较全部堆入 `shell.rs` 或主程序。

## 6. 分阶段实施与阶段门

### Stage 0：基线与发布前置治理

#### Stage 0A：只读基线

1. 记录当前分支、HEAD、工作区、`v0.1.1` Release、现有 CI 和本地 Spotlight App 状态。
2. 列出当前支持的目标平台和架构，不把“CI 能编译”自动视为“正式提供安装包”。
3. 确定每个平台首发包类型：
   - Windows：per-user 安装器；
   - macOS：App + DMG；
   - Linux：DEB + 便携压缩包。
4. 确认 Intel macOS 是否进入首发范围，以及 Linux 客户端默认推荐 DEB 还是便携包。
5. 记录 Apple Developer ID、notarization 和 Windows Authenticode 当前是否具备；缺少时明确 prerelease 技术演练限制。
6. 建立发布文件名、平台键、包型 ID 和最大允许下载大小规范。

#### Stage 0B：前置治理

1. 列出并消除现有版本硬编码，至少覆盖 `.github/workflows/instplot-studio.yml`、`apps/instplot-studio/src/lib.rs`、`scripts/validate_b1.py`；新增版本一致性门，证明 Cargo、二进制、包元数据和测试期望同源。
2. 扩展或移除现有 CI `paths` 过滤，确保 `packaging/**`、全部发布 workflows、发布脚本和更新模块的 PR 必然运行三平台质量门；为 YAML、Shell 和 Python 发布程序增加 lint/测试。
3. 为公开仓库 `main` 启用 branch protection/ruleset，设置 required checks、禁止绕过失败门并保护 tag；这一步属于用户账户侧配置，仓库文档记录实际规则。

阶段门：平台、架构、默认包型、签名现状、版本来源和 GitHub 保护规则不再含糊；新增发布路径不会绕过 PR 质量门。

### Stage 1：锁定清单、安全和版本契约

1. 先实现独立数据模型、唯一确定性 JSON 序列化规则和固定测试向量。生产 manifest 只由发布端一个实现生成；字段顺序、UTF-8 无 BOM、LF、尾随换行和禁止重复键均冻结，并用原始字节 fixture 验证，避免 Python/Rust 各自解释“canonical JSON”。
2. 生成仅用于测试的 Ed25519 密钥与有效/篡改签名 fixtures；正式密钥绝不进入仓库。
3. 为以下情况建立测试：
   - 正确清单和签名；
   - 原始字节变化、错误签名、错误公钥；
   - schema/product/channel/version/sequence/日期/过期时间/key ID/URL/平台键错误；
   - SHA-256、大小和文件名错误；
   - 同平台多包型、preferred 缺失或指向不存在 package；
   - 重复键、未知必需字段、过大清单、无效 UTF-8、错误 Content-Type/Encoding；
   - 相同版本、升级、降级、预发布版本；
   - 当前平台缺少资产和未知架构；
   - URL 跨域、HTTP、用户名密码、片段和非许可重定向。
4. Python 发布端和 Rust 客户端必须消费同一组 JSON/签名测试向量。
5. 用户确定正式 OSS HTTPS Public Root、product path 和 notes allowlist；这些值一经进入 RC 信任根，不能被运行时配置替换。
6. 离线生成正式当前 Ed25519 密钥和下一把轮换密钥，确定各自 `key_id`；私钥只进入离线备份及后续受保护的 GitHub Environment，两个公钥作为产品常量提交。
7. 写明密钥正常轮换、双钥过渡、撤销及泄漏应急流程，并承认旧客户端在私钥泄漏后无法只靠 OSS 自救；自动检查确保“下一把私钥”未出现在仓库或 GitHub，轮换启用前只离线保存。
8. 定义客户端按 channel 保存最高 `release_sequence` 的持久化位置、原子写入和损坏恢复；定义首次使用时无法完全防止旧签名清单重放的威胁边界。
9. 冻结 metadata 过期窗口和提前续期时间，建立受保护的 metadata-refresh 工作流：只引用现有不可变 Release 资产、递增 sequence、写入新 metadata revision、预验证后更新对应 channel，不创建或修改 tag/Release。

阶段门：清单契约冻结；Python 和 Rust 对全部正反例结论一致；正式 Public Root、当前/下一把公钥、key ID 和轮换规则已确定。若用户尚未提供这些条件，只能保留测试密钥原型，Stage 2 和后续正式阶段门不得标记通过。

### Stage 2：应用内安全检查更新

1. 新建独立 update 模块，不让领域文档、渲染或项目格式依赖网络代码。
2. 选择轻量、可审计且支持超时和逐跳重定向控制的 HTTPS 客户端；钉住版本并经过依赖审计。
3. 在后台任务中从构建期固定 channel 下载清单和版本化签名，设置连接/读取/总超时、响应大小上限、重试上限、精确允许 origin/path 和每跳重定向检查。
4. 只在验签前最小解析 `signature_url` 并严格限界；依次尝试内嵌公钥，验签成功后才完整解析，并要求 `key_id` 与成功公钥、`channel` 与构建期 channel 一致。
5. 把当前页脚从固定 GitHub hyperlink 改为更新检查入口；已有 GitHub Latest Release 作为明确的回退链接保留在更新窗口中。
6. 实现 `Idle / Checking / UpToDate / UpdateAvailable / Failed / Unsupported` 状态机。
7. 实现用户触发的下载状态机：选择保存路径；拒绝跟随符号链接和静默覆盖现有文件；使用独占创建、同目录权限受限临时文件；流式大小/SHA-256 校验；必要时 `fsync`；同文件系统原子改名；处理跨文件系统失败；设置安全最终权限；提供进度、取消、失败清理和“在文件夹中显示”；不自动打开或运行安装包。
8. 添加中文界面文字、窗口置顶规则、重复点击规则、键盘操作和可访问标签。
9. 用行为测试证明正式二进制不能通过环境变量、CLI 或 Cargo feature 覆盖生产 Public Root、公钥和 allowlist。

定向测试：检查/下载状态机、异步完成、重复点击、关闭窗口、进度/取消、退出收尾、超时、错误恢复、最终文件哈希和错误不污染其他应用状态。

阶段门：本地测试 HTTP 服务可以完整模拟所有结果；正式构建不能被环境变量或命令行参数改写公钥和允许前缀。

### Stage 3：三平台正式打包与真实验证

#### Windows

- 制作 per-user 安装器，包含主程序、许可证、图标和必要资源。
- 验证静默安装、实际启动、真实项目/导出 smoke、覆盖安装、重启和卸载。
- 验证安装后 EXE 与候选二进制身份及哈希一致。
- 正式可信公开发行必须 Authenticode 签名并验证签名链；若无证书，只能完成 prerelease 技术演练，并明确 SmartScreen 限制。

#### macOS

- 从空 staging 生成唯一 `.app`，校验 Bundle ID、版本、架构、图标和资源。
- 生成并挂载 DMG，验证其中 App 能复制、签名验证和真实启动。
- 正式公开发布要求 Developer ID 签名、notary submission、staple、`spctl` 和 validate；本地 ad-hoc 脚本继续只服务本机，不冒充公开签名。
- 构建结束清除可被 Spotlight 索引的 staging App。

#### Linux

- 生成 DEB 和便携压缩包。
- 在受控环境真实安装 DEB，检查 desktop entry、图标、许可证、可执行文件和卸载。
- 解压便携包并执行真实命令 smoke；记录动态依赖和最低发行版，不能仅因可解压就称为便携。

阶段门：每个平台 job 上传唯一、命名规范、版本正确且经过真实安装/运行验证的 artifact；构建目录临时包不混入 Release。Hosted runner 的进程启动只属于自动技术验证，Stage 8 的非开发桌面黑盒验收不能省略。缺少平台商业证书时只能标记相应平台“技术演练通过”，不能宣告“可信正式发行完成”。

### Stage 4：发布级 GitHub Actions

1. 新建 `release.yml`，仅接受 `workflow_dispatch.release_tag`。
2. release gate 检查：
   - tag 与 Cargo 版本一致；
   - `github.ref` 必须是受保护的 `refs/heads/main`；使用事件中的 `github.sha` 作为冻结 source SHA，不在长时间运行后与实时 main tip 再比较；
   - 对应 CHANGELOG/发布说明存在；
   - 对同名远端对象分类处理：不存在时正常开始；存在未公开 draft 时，仅当 tag、冻结 source SHA 和恢复身份匹配才允许恢复；已有 tag 指向同一冻结 SHA 可继续；tag 指向不同 SHA、已有公开 Release 或 draft 身份/摘要不符时停止；
   - 三平台普通质量门已具备同等或更强验证。
3. 所有 job 显式 checkout 同一 `source_sha`；所有第三方 Actions 钉死完整 commit SHA，不使用宽泛 `secrets: inherit`。
4. 平台 job 构建并验证资产，各自上传 artifacts；设置明确 timeout、重试上限、磁盘检查、artifact retention 和取消行为。
5. Release job 只下载本次运行 artifacts，生成 `SHA256SUMS.txt`；可选生成 SBOM 和 GitHub artifact attestation，建立 source SHA 到二进制的来源证明。
6. 先创建指向 `source_sha` 的 draft Release，并记录 release database ID、tag 和 run ID；上传全部资产后核对资产集合、文件大小、摘要、目标 SHA 和签名状态，再发布为 prerelease；失败只留下不可见 draft 供诊断，不公开半成品。
7. 冻结中断恢复规则：现有 draft 必须属于相同 tag/source SHA；已上传资产摘要相同则可幂等续跑并补齐缺失资产，摘要不同立即停止。发布前显式创建或验证 tag 指向冻结 SHA；已有 tag 指向同一 SHA 可继续，指向不同 SHA 必须停止。不得为重试删除、移动或覆盖已经公开的 tag/Release。
8. prerelease 完整发布成功后才允许调用 OSS workflow。
9. `release-status` 显式检查每个 `needs.*.result`；任何失败、跳过或取消都不得报告成功。

阶段门：在不配置 OSS 写凭据的测试模式下，可以构建、验证并生成完整本地 artifacts；失败不会创建不完整正式 Release。

### Stage 5：本地一条命令发布入口

新增 `scripts/read-version.sh` 和 `scripts/release.sh`：

- 只允许从干净、同步的 `main` 运行；
- 检查 Git、`gh`、身份、版本、发布说明和远端 tag；
- 只 dispatch GitHub workflow，随后立即退出；
- 不在开发电脑构建安装包、上传 OSS 或保存正式私钥；
- 不自动取消与本发布无关的 workflow。

阶段门：使用 dry-run 或专用测试入口证明全部拒绝条件；脚本不会修改源码、tag、Release 或 OSS。

### Stage 6：OSS 基础设施与最小权限配置

这是用户账户侧配置最集中的阶段；Stage 0/1 已需要用户提前冻结平台范围、Public Root 和正式公钥，Stage 8 还需要用户批准真实发布：

1. 创建或确认与 Stage 1 信任根完全一致的专用 Bucket、地域、HTTPS 公网 Endpoint/自定义域名，并设置版本化对象与 stable 指针的 Cache-Control。
2. 使用 GitHub OIDC 换取阿里云短期凭据；身份提供商、受保护角色和只允许写入 `instplot-studio/` 前缀的最小权限策略已经配置，不使用阿里云主账号或长期 AccessKey。
3. 建立受保护的 GitHub `production` Environment，配置审批、分支/tag 限制和 Secret 访问边界。
4. 在 `release-production` Environment 配置 `ALIYUN_OIDC_PROVIDER_ARN` 和
   `ALIYUN_ROLE_ARN`；工作流只请求短期 STS 凭据，角色的 `oidc:sub` 精确限制为
   `repo:zhiyuzhang001-a11y@235626644/InstPlot-Studio@1379312831:environment:release-production`。
5. 将 Stage 1 已离线生成的当前正式 `UPDATE_SIGNING_PRIVATE_KEY` 放入 Environment Secret；下一把轮换私钥继续离线保存，未轮换前不上传。
6. 配置产品 Variables：Bucket、Endpoint、Public Root、Product Slug、当前 key ID、公钥和资产文件规则，并与应用常量做自动一致性检查。
7. 将 Apple Developer ID/notarization 凭据和 Windows Authenticode 凭据放入同一受保护 Environment 的独立 Secrets，只让各自平台签名 job 读取。macOS 使用临时 keychain/API key 并在 `always()` 清理；Windows 使用临时证书存储或受控云签名并在 `always()` 清理，随后用平台工具验证最终签名。
8. 未配置平台证书的 RC job 必须显式走 `unsigned-technical-preview` 分支，Release 名称、说明和机器可读 summary 均标出限制；production stable job 不允许静默跳过签名。
9. 固定并校验 `ossutil` 版本及下载哈希。
10. 检查 RAM/OIDC 权限不能覆盖版本化对象，只能创建新版本路径和受控更新 channel latest；如 OSS 权限模型不能完全表达，由 workflow 的远端存在性门和 Bucket 版本控制/WORM 双重保护。
11. 检查日志不会打印 Secret、私钥或含凭据 URL；不使用 `secrets: inherit`。

阶段门：使用非正式测试前缀完成上传、读取和权限拒绝验证；身份不能写到产品前缀之外，不能覆盖版本化对象，公网只有读取权限，production Environment 未经允许不能运行。

### Stage 7：OSS 发布、原子激活和公网验证

1. `prepare-oss-release.py` 从同一次 Release artifacts 生成多平台清单、哈希、大小和签名。
2. 校验工作流私钥导出的公钥等于应用嵌入公钥。
3. 对每个版本化远端对象先做存在性检查；存在即停止，禁止 `-f` 覆盖。按 4.4 上传全部版本化对象并从公网预验证，最后才激活当前发布明确指定的 channel latest；RC 只能激活 prerelease，正式版只能激活 stable。
4. 上传时设置明确 Content-Type、Content-Encoding 和 Cache-Control；签名是原始 64 字节，版本化对象长期 immutable，stable 指针必须重新验证。
5. `verify-public-release.py` 从公网重新下载原始清单、版本化签名和每个平台资产：
   - 使用 URL 解析器验证 scheme、host、port 和规范化 path，禁止 userinfo、fragment、反斜杠和编码穿越；
   - 限制重定向次数并逐跳检查；
   - 验证签名；
   - 验证 product、version、sequence、key ID、大小和 SHA-256；
   - 使用 cache-bust 避免旧 CDN 内容。
6. 按 channel 保存上一份已验证 latest 原始字节；激活后验证失败时恢复该原始清单并执行 CDN 缓存刷新，不重新序列化或重新签名。

阶段门：公网验证全部成功；GitHub Release 哈希、OSS 资产哈希和清单完全一致；旧版本化资产仍可访问。

### Stage 8：`v0.1.2-rc.1` prerelease 端到端演练

1. 在功能分支完成实现和全部本地门禁。
2. PR 三平台 CI 全部通过后合并 `main`。
3. 执行一条本地 release 命令并关闭开发终端，证明发布不依赖本机持续在线。
4. 验证 GitHub Actions 对同一冻结 source SHA 自动完成打包、安装测试、draft Release 核验并发布 prerelease、OSS prerelease channel 上传和公网校验；stable channel 保持逐字节不变。
5. 在至少一台非开发环境设备上：
   - 从 GitHub 下载并安装；
   - 从 OSS 下载并安装；
   - 使用旧版应用检查到新版本；
   - 确认下载平台、架构、版本、大小和哈希正确；
   - 验证取消、断网和重试行为。
6. 独立 agent 对代码、CI 日志、Release、OSS 公网内容和客户端实际操作执行最终验收。

阶段门：所有自动门和黑盒操作通过；无未解决的安全、签名、平台或回滚问题。正式版必须将版本提升为 `0.1.2`，从对应新提交重新运行完整链路；不编辑或覆盖 RC tag、Release、二进制和 OSS 版本化对象。

## 7. 测试矩阵

### 7.1 发布脚本

- 非 `main`、脏工作区、本地落后/领先、无 `gh` 登录。
- Cargo 版本、输入 tag、CHANGELOG 版本不一致。
- 同名对象不存在时正常开始；匹配 draft/同 SHA tag 可恢复；不同 SHA tag、公开 Release 或不匹配 draft 必须停止。
- dispatch 后 `main` 前进时，所有 job、tag 和 Release 仍绑定原冻结 source SHA。
- draft 资产缺失、重复、大小/哈希不一致时不得公开 prerelease。
- 相同 draft/tag/source SHA 的中断任务可幂等补齐；不同 source SHA、不同摘要或已公开对象一律停止且不删除。
- job 失败、跳过、取消和超时不能被汇总任务误报为成功。
- workflow dispatch 成功后本地立即退出。

### 7.2 清单与安全

- 有效签名、错误签名、篡改字节、错误公钥。
- HTTP、错误 host/port、跨前缀 URL、userinfo、fragment、编码穿越、每一跳重定向越界。
- 缺字段、错误类型、过大响应、错误 Content-Type/Encoding、错误 schema/product/key ID。
- 哈希不一致、大小不一致、空文件、重复资产/包型、错误 preferred。
- 降级、相同版本、正常升级、RC/正式版比较、sequence 回退、过期清单和本地最高 sequence 损坏恢复。
- 系统时钟过早/过晚、临近过期、恰好过期、远未来时间，以及 app version 不变但 metadata sequence 正常递增。
- 当前/下一把公钥选择、正常轮换、未知 key ID 和撤销情景。
- `key_id` 指向 A 但由 B 签名必须拒绝；客户端逐一尝试内嵌公钥后必须核对成功 key ID。
- 版本化对象已存在时拒绝覆盖；channel 恢复必须逐字节等于历史 manifest。
- RC 只能更新 prerelease channel，正式构建只读取 stable；任一 channel 发布不得改变另一 channel 的原始字节和 ETag。

### 7.3 网络与 UI

- 正常、断网、DNS 错误、连接超时、读取超时、HTTP 404/500。
- 点击检查时 UI 继续绘制和编辑。
- 重复点击不产生并行请求；已有窗口置顶而非开多个副本。
- 关闭更新窗口不取消或破坏主文档；应用退出时后台任务安全结束。
- 下载进度、取消、临时文件权限、失败清理、原子改名和最终大小/SHA-256 校验。
- 中文错误信息不泄漏内部路径、密钥或调试对象。
- 键盘可操作、焦点顺序、屏幕阅读标签和后台完成后的 repaint。

### 7.4 平台与安装包

- Windows 安装、覆盖、重启、真实操作、卸载。
- Windows 签名链/SmartScreen 状态与 Release 描述一致。
- macOS 架构、Bundle ID、版本、Developer ID、notarization、staple、`spctl`、DMG 挂载和启动。
- Linux DEB 安装/卸载、便携包运行、动态依赖和最低发行版。
- GitHub 与 OSS 下载得到相同字节。
- 非开发电脑安装并完成最小真实数据导入和图像导出。

### 7.5 回归

- 更新模块不可改变项目 schema、数据文件或导出结果。
- 更新失败不影响启动、绘图、保存、Undo/Redo、导入和导出。
- 普通 CI 继续覆盖全部已有功能；发布 CI 不能替代 PR 质量门。
- 正式 release 二进制不接受环境变量、CLI 或 `--all-features` 带入的测试信任根覆盖。

## 8. 标准本地门禁

实现阶段至少执行：

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
cargo test --locked --workspace --doc
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo build --locked --release --package instplot-studio
python3 scripts/check_repository_hygiene.py
```

并保留现有 release 二进制的项目检查、handoff、publication check、PDF 和 PNG smoke。发布相关 Python/Shell 程序还必须有独立测试，不能只在真实发布时首次运行。

## 9. 凭据、签名与用户操作边界

可以完全在仓库内先完成：

- 清单 schema、测试向量，以及只使用测试信任根的 Rust 更新模块原型；
- 打包定义和 GitHub workflows；
- release/prepare/verify 脚本及测试；
- 使用测试密钥和本地 HTTP 服务的完整演练。

正式 RC 客户端和安装包不能在仓库内凭占位值完成；必须先由用户冻结 Public Root、当前/下一把正式公钥和平台范围，再编译并重新执行全部验证。

必须由用户账户侧提供或确认：

- 是否正式支持 Intel macOS。
- Linux 首发默认推荐 DEB 还是便携包。
- OSS Bucket、地域、Public Root、自定义域名、缓存费用与历史对象保留策略。
- GitHub OIDC 到阿里云的信任已配置；仍需在合并后以无写入探针验证真实 STS 扮演和前缀权限边界。
- 正式当前/下一把 Ed25519 密钥的离线生成与备份，以及当前私钥进入受保护 GitHub Environment Secret。
- Apple Developer ID/notarization 凭据；
- Windows Authenticode 证书；
- GitHub production Environment、审批规则、`main` 分支保护和 required checks。
- 至少一台非开发 Windows/macOS/Linux 设备的最终安装验收。
- `v0.1.2-rc.1` 发布和后续 `v0.1.2` 正式发布的明确批准。

缺少商业平台证书不阻止代码和 prerelease 演练，但阻止把对应平台资产声明为完成可信签名的正式发行版。

## 10. 停止条件

出现以下任一情况时停止当前阶段，不用后续步骤掩盖：

- 需要把正式私钥、OSS Secret 或平台签名密码写入仓库或日志。
- 不能证明清单原始字节和客户端验签对象完全一致。
- 发布资产不是来自同一次已验证 workflow。
- 平台安装测试只能检查日志，不能验证最终进程或文件。
- 任一 channel latest 会先于对应版本化资产和 metadata revision 上传。
- 版本化 OSS 对象需要 `-f` 或其他覆盖写才能继续。
- 客户端允许从任意 URL、环境变量或未签名清单获取安装包。
- 测试信任根或 URL 覆盖能力会进入正式 release 二进制。
- 更新网络请求阻塞主 UI 或破坏现有编辑状态。
- 浏览器下载被误报为应用已经验证最终安装包哈希。
- GitHub、OSS 和应用显示的版本或哈希不一致。
- Release 没有绑定冻结 source SHA，或 draft 尚未核验完整就被公开。
- 需要覆盖既有 tag、Release 或历史版本化 OSS 对象。
- 工作范围开始扩展为自动提权、覆盖安装或自动重启。

## 11. 完成状态与定义

本计划使用两个不可混淆的完成状态：

- `TECHNICAL_RC_COMPLETED`：`v0.1.2-rc.1` 的功能、打包、GitHub prerelease、OSS prerelease channel、客户端检查/下载和非开发设备黑盒验收全部通过；缺少 Developer ID/Authenticode 时仍只能停在此状态。
- `TRUSTED_STABLE_RELEASE_COMPLETED`：平台正式签名、公证、production Environment、`v0.1.2` 新构建、GitHub 正式 Release、OSS stable channel 和最终非开发设备验收全部通过。

顶层计划只有达到 `TRUSTED_STABLE_RELEASE_COMPLETED` 才可标记为 `COMPLETED`。达到技术 RC 状态时必须明确记录尚缺的商业凭据或正式门，不得简写为“发布完成”。

以下技术与安全条件必须满足：

1. PR 和发布工作流的三平台门全部通过。
2. Windows、macOS、Linux 发布资产均来自同一冻结 source SHA 和同一次运行，并完成对应真实验证。
3. GitHub draft 在资产集合、大小、摘要、签名状态和 source SHA 全部核对后才发布；Release 包含安装资产、发布说明和 `SHA256SUMS.txt`。
4. OSS 的版本化资产和签名不可变，全部预验证后只覆盖目标 channel 的一个 latest 对象完成原子激活。
5. 公网重新下载后的签名、版本、大小和 SHA-256 全部通过。
6. 应用可以异步检查更新、验证签名、比较版本、选择正确平台/包型，并在用户触发后下载和校验最终文件。
7. 无更新、存在更新、断网、篡改、错误平台、取消、sequence 回退、密钥选择和重复点击均得到正确处理。
8. 更新功能不影响任何现有绘图、项目和导出能力。
9. 非开发电脑可分别从 GitHub 和 OSS 下载并完成一次真实安装与基本操作。
10. RC 只激活 prerelease channel；正式版由新的 `v0.1.2` 构建激活 stable，两个 channel 互不污染。
11. 独立 agent 最终验收为 PASS，且没有未解决的高优先级问题。

## 12. 后续独立项目

完成本计划后，如确有需要，再另行设计自动覆盖更新器：

- Windows：退出旧进程、安装器覆盖、启动新进程和失败恢复。
- macOS：下载、验签、公证校验、替换 App 和重新登记 Launch Services。
- Linux：根据 DEB、RPM、AppImage 或便携包分别处理。

该阶段必须使用独立 helper/updater 进程和独立安全评审，不应作为本计划的顺手扩展。
