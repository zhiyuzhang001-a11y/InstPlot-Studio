# InstPlot Studio 双轴 Phase 4–8 集成报告

日期：2026-09-27

## 结果

Phase 4–8 的预览、交互、数据、图例/标注和导出链路已经接入 Phase 1–3 建立的统一双轴模型。单轴、双 Y、双 X 都沿同一条 `document → layout → resolved display → preview/export` 路径工作，没有增加第二套坐标计算。

## 主要实现

- 顶栏提供互斥的单轴、双 Y、双 X 模式；新曲线仍默认 X1/Y1。
- 曲线编辑器和左侧文件卡片都能选择当前模式允许的轴组合；关闭副轴后的曲线显示明确休眠提示。
- 四侧坐标轴通过稳定的 X1/X2/Y1/Y2 身份打开同一类编辑窗口；重复双击遵循既有置顶策略。
- 左下角坐标读数按模式显示 X1/Y1、X1/Y1/Y2 或 X1/X2/Y1。
- 同一文件卡片内可新增并切换多条曲线，支持共享 X 和独立 X 的列重绑定。
- 副轴第一次接收曲线且标签为空时，以对应列名建议标签；已有用户标签永不覆盖。
- 只对 `(unit)` 或 `[unit]` 这种明确单位提示同轴冲突，不模糊猜测、不自动换算。
- 参考线和标注箭头连接点可选择当前轴组合；副轴关闭时沿统一有效可见性规则隐藏。
- 图例继续默认图内，用户可手动选择图上或图右；模式切换不改写图例位置。
- PDF、SVG、PNG 使用同一 resolved canvas，并增加双轴跨后端一致性测试。

## 新增回归覆盖

- 副轴范围、刻度和标签可按身份编辑，且不能替换稳定 ID。
- 空副轴标签首次建议、用户标签保护。
- 三模式坐标读数只包含当前启用副轴。
- 明确单位解析边界。
- 双轴 PDF/SVG/PNG 的画布尺寸与正式 resolved scene 一致。
- 既有布局测试继续覆盖副轴边缘所有权、图外图例、长标签和紧凑画布。

## 门禁结果

- `cargo fmt --all -- --check`：通过。
- `cargo test --locked --workspace --all-targets --all-features --no-fail-fast`：通过，零失败。
- `cargo test --locked --workspace --doc`：通过。
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`：通过，零警告。
- `cargo build --locked --release --package instplot-studio`：通过。
- `python3 scripts/audit_b5p.py`：15/15 通过。
- `git diff --check`：通过。

## 后续门

Phase 9 自动化门禁已具备通过证据。Phase 10 仍需在未安装的本地 release 构建上完成真实窗口化、最大化、全屏和典型数据操作，并由独立 agent 复核；用户确认前不替换 Spotlight 应用、不推送 GitHub。
