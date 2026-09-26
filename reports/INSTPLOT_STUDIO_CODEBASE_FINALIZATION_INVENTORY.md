# InstPlot Studio 代码整合路径清单

日期：2026-09-26
分支：`codex/codebase-finalization`
起点：`961e5bafec333b0942864329ad099b35f78a64aa`

## 结论

当前所有已修改、删除和未跟踪路径均已归类，没有待确认的 `REVIEW` 项。正式代码、资源、测试、文档和脚本进入封板分支；四个已迁移原型删除；生成物保持忽略；一条包含个人安装权限与路径的开发规则仅保留在本地 Git exclude 中。

## KEEP：正式产品与 workspace

- 根目录：`Cargo.toml`、`Cargo.lock`、`README.md`、`CHANGELOG.md`。
- `apps/instplot-studio/`：正式产品、资源、示例、领域契约与 UI 回归。
- `apps/instplot-demo/`：第二产品 consumer，用真实导入与 SVG 输出证明复用边界。
- `crates/instplot-layout/`：正式布局和兼容行为。
- `crates/instplot-text/`：语义文字、字体资源、许可、manifest 与 SHA-256。
- `crates/instplot-render/`：后端无关 Figure IR 与 display list。
- `crates/instplot-export/`：PDF、PNG、SVG 后端与视觉基线。
- `crates/instplot-ui/`：通用界面、窗口策略和产品 shell。

## KEEP：测试、CI 与工具

- `.github/workflows/instplot-studio.yml`：Linux、macOS、Windows 主质量矩阵。
- `.github/workflows/part-a-windows-manual-kit.yml`：历史 Part A Windows 人工证据包；仍可独立触发。
- `scripts/validate_*.py`、`scripts/audit_*.py`：历史和当前阶段验证入口。
- `scripts/install_studio_macos.py`：只替换 `$HOME/Applications/InstPlot Studio.app` 的本机安装器。
- `scripts/check_repository_hygiene.py`：阻止生成物、用户绝对路径、疑似密钥和异常大文件进入 Git。
- `.cursor/rules/studio-popup-window-layout.mdc`：适用于团队的跨平台窗口设计规则。

## KEEP：架构与质量文档

- `adr/023-modular-product-boundaries.md`：当前模块边界决策。
- `docs/INSTPLOT_STUDIO_MODULARIZATION_PLAN.md` 与 Phase 0–10 证据。
- `docs/INSTPLOT_EXTENSION_GUIDE.md`：第二产品复用说明。
- `docs/INSTPLOT_STUDIO_CODEBASE_FINALIZATION_PLAN.md`：本次封板执行计划。
- 当前 UI、文字、手动数据、QA 和工作流计划。
- 三份最新真实操作/修复报告，以及既有 A/B 阶段历史报告。

历史报告中提到已删除 prototype 路径属于不可改写的当时证据，不代表当前构建入口。当前 README、workspace、CI 和正式阶段文档只指向仍存在的生产路径或明确的兼容夹具。

## KEEP：唯一兼容性原型

- `prototypes/layout-engine-spike/`：保留为历史布局兼容测试 facade。
- 它被 workspace 明确排除，由 CI 使用独立 manifest 运行。
- 正式产品不依赖该原型；未来删除前必须先迁走剩余兼容快照和门禁。

## DELETE：已迁移原型

以下目录的实现、测试或资源已迁入对应正式 crate，确认删除：

- `prototypes/export-backend-spike/` → `crates/instplot-export/`。
- `prototypes/studio-render-spike/` → `crates/instplot-render/`。
- `prototypes/text-shaping-spike/` → `crates/instplot-text/`。
- `prototypes/ui-shell-spike/` → `crates/instplot-ui/`。

字体文件、许可证、manifest、视觉基线和快照均在正式路径中存在；`Cargo.lock` 不再包含这些旧 package 名称。

## GENERATED：保留忽略，不提交

- 所有 `target/` 目录。
- 仓库根 `tmp/`。
- `.agent-token-manager/` 导航索引。
- `crates/instplot-export/artifacts/` 本地生成的导出物。
- `__pycache__/`、日志和 `.DS_Store`。

这些目录可在最终验证后按需清理，但不作为源码提交的一部分。

## LOCAL_ONLY：不提交

- `.cursor/rules/studio-development-restart.mdc`。

该规则记录特定用户允许关闭本机运行应用的即时授权，并曾包含用户专属安装路径，不属于团队代码规范。它已加入 `.git/info/exclude`，不会进入 GitHub。

## 资源检查

- 应用图标：`apps/instplot-studio/assets/InstPlotStudio.png`，RGBA PNG。
- TeX Gyre Heros 四个字体文件与 GUST 许可、来源 manifest、SHA-256 一致。
- STIX Two Math 字体与 OFL、来源 manifest、SHA-256 一致。
- 导出视觉基线位于正式 export crate，单文件均低于仓库卫生上限。
- 未发现软链接、用户科研数据、项目文件、私钥或访问令牌。

## 路径与命名检查

- 已将正式文档中的用户专属 `/Users/<name>/...` 改为 `$HOME/...` 或仓库无关表达。
- 测试中使用 `/tmp/...` 的路径只属于隔离的临时用例，不是运行时依赖。
- 重复 basename、Unicode 项目名和带空格路径均有测试覆盖。
- `instplot-*` 是正式 package 命名；`SciPlot` 只可能作为历史仓库目录语境存在。

## 阶段门禁

- [x] 所有状态路径已归类。
- [x] 四个删除目录均有正式替代位置。
- [x] 生成物均被忽略。
- [x] 本机授权规则未进入版本库。
- [x] 字体与图标资源已识别并有许可/校验依据。
- [x] 没有未决 `REVIEW` 项。
