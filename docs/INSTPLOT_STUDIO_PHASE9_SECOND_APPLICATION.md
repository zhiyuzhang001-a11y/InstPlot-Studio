# InstPlot Studio Phase 9：第二个真实应用验证

状态：已完成
完成日期：2026-09-25

## 第二个产品

新增独立 workspace 应用 `apps/instplot-demo`，产品名为 **InstPlot Quick**。它不是 Studio 源文件的复制品，而是通过 Cargo 依赖复用：

- `instplot-ui` 的 `Branding`、`FeatureSet`、`ShellEvent` 和 `AppServices`；
- `instplot-studio` library 的统一数据导入、文档、标签和 SVG 原子导出服务；
- 下层正式 text/render/export/layout crate 由依赖图统一提供。

## 不同功能组合

Quick 启用数据导入与 SVG 导出，关闭项目文件、手动录入、标注、出版检查、PDF 和 PNG。该组合证明功能开关不是 Studio 固定菜单的别名。

## 真实流程

CLI 接口：

```text
instplot-demo INPUT X_COLUMN Y_COLUMN X_LABEL Y_LABEL OUTPUT.svg
```

集成测试和实际运行均完成：

1. 导入 CSV；
2. 校验并选择 X/Y 列；
3. 重绑定自动创建的系列；
4. 修改 X/Y 标签；
5. 自动范围；
6. 原子导出 SVG。

实测命令生成 204954 字节 SVG，标签文字和画布均来自正式渲染管线。

## 阶段门禁

- `cargo test -p instplot-demo --all-targets`：通过；
- 实际 `cargo run --locked -p instplot-demo ...`：通过；
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过；
- `cargo fmt --all -- --check`：通过。

## 结论

复用边界已由第二个可构建、可运行的产品验证。它无需复制 Studio UI 或渲染代码即可完成核心科学绘图流程。
