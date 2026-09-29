# InstPlot Studio 双轴 Phase 0 基线

> 状态：`PASS`  
> 日期：2026-09-27  
> 分支：`codex/dual-axes`  
> 基线提交：`560438d`（`v0.1.0`、`origin/main`）

## 产品与项目基线

- 产品版本：`0.1.0`；
- 项目 schema：`7`；
- Rust toolchain：仓库固定 `1.98.0`；
- 开始时除双轴设计/实施文档外无其他工作区修改；
- 本阶段未替换 Spotlight 应用、未推送 GitHub。

## 自动化结果

- `cargo fmt --all -- --check`：PASS；
- `cargo test --locked --workspace --all-targets --all-features --no-fail-fast`：PASS，零失败；
- `cargo test --locked --workspace --doc`：PASS；
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`：PASS，零警告；
- `cargo build --locked --release --package instplot-studio`：PASS；
- `git diff --check`：PASS；
- `python3 scripts/audit_b5p.py`：PASS，15/15；
- B5P P0 工作流：PASS，29/29；
- prototype 和共享核心兼容检查：PASS；
- fixture 未改变、无机器本地路径：PASS。

## 现有测试基线

- Studio library：143 passed、0 failed、1 ignored（按需生成视觉检查 PDF）；
- Studio GUI：70 passed、0 failed；
- layout publication tests：19 passed；
- layout scale tests：9 passed；
- export PDF/SVG/PNG、字体和光栅后端测试：PASS；
- UI headless、窗口策略和第二产品测试：PASS。

## 单轴视觉与尺寸基线

B5P 的五组正式基线（single-source、source-fit、multi-source、disabled-row、missing-value）全部满足：

- 基础 figure canvas：`85 × 65 mm`；
- PDF page：约 `240.94489 × 184.25197 pt`；
- PNG：`1004 × 768 px`（300 dpi）；
- PDF 字体：批准且嵌入；
- PDF/PNG resolved layout：一致且未裁切；
- 项目数据：embedded，handoff checksum 正确；
- 项目检查与出版规范检查：PASS。

证据位置（构建产物，不提交仓库）：

- `target/b5p-audit/summary.json`；
- `target/b5p-p0-validation/summary.json`；
- `target/b5p-p0-validation/artifacts/`。

## 基线维护修复

首次运行 B5P 审计时，应用、导出和项目检查均通过，但两个审计脚本仍硬编码旧 schema `6`，与当前正式 schema `7` 不一致，导致五组 artifact 检查误报失败。

本阶段只将下列门禁期望同步到当前 schema `7`：

- `scripts/validate_b5p_p0.py`；
- `scripts/audit_b5p.py`。

修正后完整审计通过。该问题属于既有审计规则过期，不是双轴功能或当前应用缺陷。后续 Phase 1升级 schema 时必须同步更新并重跑这两个门禁。

## Phase 0结论

当前单轴功能、导入/录入数据、误差棒、图例、布局、项目保存、出版检查及 PDF/SVG/PNG 导出具备干净、可重复的实施基线。Phase 1可以开始；任何后续非预期单轴尺寸、布局或测试变化均视为回归。
