# InstPlot Studio 双轴轴线、参考线与测量箭头 QA 记录

> 状态：`PASS / DELIVERY IN PROGRESS`
> 日期：2026-09-27
> 分支：`codex/dual-axes`
> 起始提交：`4d45bab76787b44d9a66a775bb5fd54fb73d380a`

## 实施结果

- Phase 0–7 已完成：schema 9、旧项目迁移、双轴 spine 颜色、参考线、文字连接线扩展、独立测量箭头、统一命中/窗口、Display List 和多后端导出均已接通。
- 真实 schema 8 fixture 已固定；schema 0–9 迁移、项目 round-trip、标签所有权和非法输入均有回归测试。
- 科学导引对象不进入逻辑数据系列、图例或出版规范的系列配色/marker/line-style 检查。
- 参考线 autoscale 开关、关闭后范围收缩、删除后范围收缩、相关轴绑定和越界提示已验证。
- 测量标签支持换行、拖拽和项目重开；同一对象不同命中部位只置顶一个编辑窗口。
- 配色切换按系列序号迁移 spine、参考线和文字连接线颜色；测量箭头固定黑色。

## 自动门禁

以下命令在最终工作树执行成功：

```text
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release -p instplot-studio
cargo run --release --locked -p instplot-studio -- --product-info
git diff --check
```

关键结果：

- Studio library：171 passed，1 ignored（显式 opt-in 的视觉 PDF 生成测试）。
- Studio UI/controller：79 passed。
- Layout publication suite：23 passed。
- `instplot-ui`：12 passed，包含普通、macOS 最大化和全屏窗口策略。
- 其余 workspace 单元、集成、导出、字体、数据和文档合同测试全部通过。
- Clippy 以 `-D warnings` 通过；release 构建和产品信息 `InstPlot Studio / instplot-studio / 0.1.0` 通过。

## 独立 agent 复验

- 独立 agent 首轮发现并复现：科学导引被误计入出版系列、连接线跨边界 Shift 吸附、参考线 autoscale 收缩、测量标签换行、越界提示、同对象重复窗口、参考线无关轴控件和 neutral palette 迁移等问题。
- 上述问题修复后，独立 agent 已复跑定向测试、workspace 全量测试、Clippy、doc tests 与 release build，结论为无功能 P0/P1。
- 独立 agent 对最新已安装构建完成普通、最大化和全屏真实窗口操作；连续参考线、水平双向测量箭头、对象编辑窗、配色方案、出版规范、重复命令置顶、点击画布不关闭等均通过。
- 最终独立结论：`PASS`，未发现 P0/P1/P2。

## Spotlight 交付

- 安装位置：`$HOME/Applications/InstPlot Studio.app`
- Bundle ID：`com.instplot.studio`
- 版本：`0.1.0`
- 构建号：`202609271729`
- `codesign --verify --strict`：通过。
- Spotlight 查询：仅返回上述一个应用。
- 应用已实际启动，主窗口显示本构建号和新版“参考线 / 测量箭头”入口。

## GitHub 范围

Phase 8 通过后上传当前源码分支并建立 PR。按用户要求，本轮不合并、不创建 release、不打 tag、不执行 Windows 构建。
