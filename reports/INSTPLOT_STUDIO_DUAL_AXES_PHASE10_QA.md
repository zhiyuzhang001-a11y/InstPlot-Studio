# InstPlot Studio 双轴 Phase 10 验收报告

日期：2026-09-27

## 结论

Phase 10 通过。测试使用未安装的本地 release 包 `/tmp/instplot-dual-qa.63BTcR/InstPlot Studio QA.app`，没有替换 Spotlight 正式应用、没有提交或推送 GitHub。真实操作、自动化契约测试和独立 agent 复核共同覆盖单轴、双 Y、双 X、数据生命周期、图例、窗口策略和导出。

## 真实操作

- 导入真实 `E-dsk_1_5V.csv`：文件卡片、X/Y 列、完整曲线、标签和自动范围正确。
- 导入多列 `fixtures/publication-v1/data.csv`：同文件新增第二、第三条逻辑曲线后立即绘制；系列选择与图例数量一致。
- 三种模式互切：双 Y 曲线在单轴/双 X 中休眠，回到双 Y 后恢复；无数据 X2/Y2 不显示虚假刻度。
- 将曲线绑定到 X1/Y2：Y2 独立范围与列名标签建议正确，左下角同时显示 X1/Y1/Y2。
- 双击右侧副轴：打开 Y2 编辑器；重复双击置顶，不产生重复窗口。
- 普通窗口、macOS Zoom 最大化、系统全屏均复验；Zoom 与全屏使用嵌入式工具窗口，普通窗口使用可移动独立工具窗口。
- 同文件系列入口保持紧凑，长文件名不再把新增按钮挤出卡片。

## 独立 agent 发现并闭环的问题

1. 点线图内部的 line/scatter 曾被左栏拆成多个系列入口。已改为按稳定 `SeriesGroupRecord` 枚举，线、点、误差棒和图例只有一个逻辑入口，并新增回归测试。
2. 新增同文件曲线曾只更换颜色而沿用点形。已按逻辑数据绑定循环分配不同颜色和点形，并新增同文件多曲线测试。
3. macOS Zoom 未可靠上报 `maximized`，工具窗口仍会脱离主窗。已增加基于显示器与窗口实际尺寸的保守判定，并完成最大化实测。
4. 设计文档末尾旧的 Phase 1进度说明已与页首和实施计划同步。

## 自动化覆盖矩阵

- X2 + X error、Y2 + Y error：独立自动范围包含误差棒端点。
- 用户 `visible=false`：关闭副轴、保存重开、重新启用后仍保持隐藏；休眠不改写持久显隐。
- 导入文件与录入数据混合：共用同一逻辑曲线和轴绑定路径，可分别绑定主/副轴。
- `X,Y1,Y2` 与不同 X/Y 组合：同文件可新增逻辑曲线并独立重绑列；X2/Y2 组合由 UI 和项目校验共同拒绝。
- 点线图 + 误差棒：始终作为一个逻辑系列切换轴和生成图例。
- schema 0–7迁移、schema 8 round-trip、休眠状态 round-trip：通过。
- 图例图内/图上/图右、跨区域拖拽、行列控制和紧凑外画布：通过。
- PDF、SVG、PNG：共享同一 resolved canvas，导出不改变项目或源数据。
- 删除数据源、删除最后一条副轴曲线、清除全部、再次导入：通过。
- 长标签、科学计数法、边缘所有权、出版规范 resolved 尺寸：通过。

## 最终门禁

- `cargo fmt --all -- --check`
- `cargo test --locked --workspace --all-targets --all-features --no-fail-fast`
- `cargo test --locked --workspace --doc`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo build --locked --release --workspace`
- `python3 scripts/audit_b5p.py`
- `git diff --check`

以上命令必须在最终修订后再次全部通过。Phase 11 仍等待用户确认，当前不替换 Spotlight 正式应用。
