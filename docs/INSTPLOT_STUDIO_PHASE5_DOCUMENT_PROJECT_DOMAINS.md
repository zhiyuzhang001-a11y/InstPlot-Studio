# InstPlot Studio Phase 5：文档领域与项目存储拆分

状态：已完成
完成日期：2026-09-25

## 目标

在不改变项目文件格式和用户行为的前提下，把图形文档、自动范围和项目存储从大型源文件拆成职责明确、可独立测试的模块。

## 已完成内容

- `document` 按 `axes`、`objects`、`series`、`datasets`、`autoscale` 和兼容 API 拆分；原有公共类型与调用路径保持兼容。
- 自动范围成为纯领域计算：`compute_data_bounds` 只处理数据、误差棒和明确纳入范围的对象；`apply_visual_padding` 单独处理视觉留白。
- marker 的 pt 尺寸不再混入数据坐标；误差棒会扩展数据范围，普通标注不会意外改变范围。
- `project` 按 schema、fixture、migration、validation 和 storage 拆分。
- `decode_project`、`open_project`、`save_project` 的兼容公共接口保留；项目 schema 版本与文件格式未改变。
- 原有迁移、fixture、保存、恢复和文档往返测试保留并通过。

## 新增契约测试

- `apps/instplot-studio/tests/document_domain_contract.rs`
  - 误差棒进入数据范围；
  - 标注不污染数据范围；
  - marker 的屏幕尺寸不改变数据坐标范围。

## 阶段门禁

- 完整测试：229 passed，0 failed，1 ignored。
- `cargo clippy --locked --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`：通过。

## 兼容性结论

本阶段只改变内部边界，没有更改项目 schema、导入流程、UI 行为或渲染结果。文档和项目存储现在可以分别演进，Phase 6 可在稳定的 `ResolvedFigure`/文档快照边界上产品化渲染与导出。
