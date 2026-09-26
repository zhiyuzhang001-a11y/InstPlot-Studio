# InstPlot Studio Phase 1 机械拆分验收记录

日期：2026-09-25
结论：通过；可以进入 Phase 2。

## 1. 本阶段边界

本阶段只移动二进制内部代码、调整兄弟模块所需的 `pub(super)` 可见性，并让测试跟随应用模块迁移；没有改变项目格式、交互规则、数据导入语义、布局或导出算法。

## 2. 拆分结果

- `main.rs`：只保留模块组装、依赖导入和 `main` 入口，由约 3400 行降为 70 行。
- `startup.rs`：命令行参数、无界面启动命令和原生窗口启动。
- `app_state.rs`：`StudioApp` 及其 UI 临时状态、拖拽状态和编辑草稿。
- `app_controller.rs`：现有应用操作入口的机械隔离；其 3535 行规模是 Phase 2 的主要治理对象，不代表最终 Controller 设计。
- `app_ui.rs`：`eframe::App` 顶层界面组合。
- `ui_chrome.rs`：主题、窗口框架、按钮和通用界面外观辅助。
- `import_flow.rs`：导入后列选择及文档构建辅助。
- `editor_support.rs`：标签、出版检查文案、点形、线型和手动录入等编辑辅助。
- `canvas_support.rs`：预览映射、命中测试、选择框、缩放和数据导航辅助。
- `app_tests.rs`：原二进制单元测试迁移，测试内容和断言保持不变。

## 3. 阶段门槛证据

- `cargo fmt --all -- --check`：通过。
- `cargo test --locked --workspace`：211 个测试通过，0 个失败，1 个显式忽略的视觉检查测试。
- `cargo clippy --locked --all-targets -- -D warnings`：通过。
- Phase 0 的离线项目恢复契约测试继续通过。
- `main.rs` 不再包含数据解析、文档修改、布局或渲染实现。

## 4. 保留风险与 Phase 2 约束

- `app_controller.rs` 当前只是旧实现的机械容器，仍同时承担导入、删除、清除、打开、保存、导出和大量 UI 协调；Phase 2 必须按事务边界拆解，不能把它当作完成后的应用服务。
- `StudioApp` 仍直接持有 `document`、`session`、`workspace` 与 UI 状态。Phase 2 必须先定义所有权和提交规则，再迁移调用点。
- 不允许在 Phase 2 把导入解析、自动范围、标签解析或渲染算法塞进 Controller；Controller 只协调领域服务并原子提交结果。
- 所有语义改动都必须先增加结果导向测试，并继续保持 Phase 0/1 全量门槛通过。
