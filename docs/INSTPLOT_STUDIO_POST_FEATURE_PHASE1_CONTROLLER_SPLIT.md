# InstPlot Studio 新功能后整合 Phase 1：StudioApp 协调适配层拆分

> 状态：`COMPLETED`  
> 日期：2026-09-28

## 结果

原约 5600 行的 `app_controller.rs` 已机械拆为职责模块：

- `lifecycle.rs`：初始化、导入入口、项目打开/保存及冲突确认；
- `export_workflow.rs`：手动数据和 PDF/PNG/SVG 导出工作流；
- `coordination.rs`：事务提交、Undo/Redo、消息与文档命令协调；
- `axis_editor.rs`：画布尺寸、轴范围、刻度和轴标签编辑；
- `selection_windows.rs`：选择、上下文目标、轴显示和文字标注入口；
- `data_workflow.rs`：录入数据、数据源卡片、XY/error 绑定和删除确认；
- `tool_windows.rs`：出版检查、配色、参考线草稿窗口；
- `interaction.rs`：画布工具事件、拖拽和对象窗口调度；
- `artist_editor.rs`：对象属性与标签输入编辑；
- 根 `app_controller.rs`：模块组装和跨模块共享纯 helper。

根文件现为约 316 行。拆分没有移动 `app_transactions.rs` 中的 `ApplicationController`，原子事务入口保持唯一。

移动到子模块的方法使用 `pub(crate)`，其有效可见范围与原来位于父模块时的 `pub(super)` 相同，没有形成公共库 API。

## 验证

- Cargo format：PASS；
- Studio 全 target check：PASS；
- 生命周期、保存冲突、窗口置顶、参考线/测量工具和导出放行定向测试：PASS；
- workspace 全 target/feature tests：PASS；
- strict Clippy：PASS；
- `git diff --check`：PASS。

本阶段只移动代码和调整等价的 crate 内可见性，没有改变文案、默认值、schema、布局或交互。

