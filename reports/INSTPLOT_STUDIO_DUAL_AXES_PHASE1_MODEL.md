# InstPlot Studio 双轴 Phase 1 验收记录

日期：2026-09-27
分支：`codex/dual-axes`
范围：项目模型、schema、历史迁移与验证；尚未启用双轴 UI 或渲染。

## 完成内容

- 项目 schema 从 7 升级到 8；schema 0–7 均沿既有逐级迁移链进入 schema 8。
- 增加互斥的单轴、双 X、双 Y 模式，以及 X1、X2、Y1、Y2稳定身份。
- 增加 `SeriesGroupRecord`，用稳定逻辑曲线身份归组 line、scatter、error bar，并在数据删除、复制、类型切换等既有路径同步维护。
- 每个逻辑曲线保存明确的 X/Y 轴绑定；验证拒绝 X2/Y2组合和缺少活动副轴记录的项目。
- X2、Y2记录可以同时保存在项目中作为休眠设置，但模式枚举保证任一时刻只启用一种副轴。
- 参考线和数据坐标箭头连接点具有轴组合；普通画布文字保持无轴绑定。
- `AxisMode::edge_owner` 给四侧边缘提供唯一所有者，作为后续布局层禁止镜像刻度与副轴竞争的模型事实。
- 历史项目迁移优先使用图例锚点，只做唯一、互补类型的确定性归组；歧义对象保持独立，且打开项目时返回明确警告。
- 迁移测试确认 artist 数量、顺序、属性和图例条目数量不因逻辑分组而改变。

## 修复的回归

首次完整库测试发现三处由逻辑分组完整性校验暴露的问题：删除数据源未同步清理逻辑组、清空最后数据源留下悬空成员、出版规范测试夹具新增 artist 时未建立逻辑组。三处均已修复，并纳入完整回归测试。

## 验证结果

- `cargo test --locked -p instplot-studio project::project_tests --no-fail-fast`：24 通过、0 失败。
- `cargo check --locked --workspace --all-targets`：通过。
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`：通过。
- `cargo test --locked --workspace --all-targets --all-features --no-fail-fast`：全部通过；Studio lib 147 通过、1 个显式忽略的人工视觉夹具，Studio GUI 70 通过，其余 workspace 测试全部通过。
- `cargo fmt --all -- --check`：通过。
- `git diff --check`：通过。

## 阶段结论

Phase 1 满足设计方案和实施计划的进入条件。没有删除旧字段、没有让模型依赖 UI 类型，也没有改变当前单轴渲染行为。下一阶段可在该模型上实现 document/transaction 层的模式切换、有效可见性和四轴独立自动范围。
