# InstPlot Studio Phase 6：产品化渲染与导出管线

状态：已完成
完成日期：2026-09-25

## 正式模块

原先验证用的四个核心包已升级为正式 workspace crate，并从 `prototypes/` 移入 `crates/`：

- `crates/instplot-text`
- `crates/instplot-render`
- `crates/instplot-export`
- `crates/instplot-ui`

生产代码和 Cargo 依赖不再使用 `*-spike` / `*_spike` 名称。

## 统一渲染契约

正式路径为：

```text
FigureDocument
  → resolve_document
  → ResolvedFigure { layout, display }
  → egui preview / PDF / PNG / SVG
```

- 预览后端接收 `ResolvedDisplayList`，不再解释文档。
- PDF、PNG、SVG 增加直接消费 `ResolvedFigure` 的正式 API。
- 应用导出事务先解析一次，再用同一不可变快照写文件和生成出版检查结果，避免前后两次解析产生差异。
- 旧的 document-based 导出 API 保留为兼容包装层。
- SVG 进入 Studio 的正式公共导出 API。

## 文件安全与视觉契约

- 所有落盘导出继续使用临时文件加原子替换。
- 新增失败导出保留已有目标文件的回归测试。
- 透明/白色 PNG、图外图例紧凑画布、固定物理尺寸、PDF 字体嵌入和 SVG viewBox 均由工作区测试覆盖。
- `render_export_contract` 验证 PDF、PNG、SVG 与预览共享同一 scene 尺寸，document 包装 API 与 resolved API 输出一致。

## 阶段门禁

- `cargo test --locked --workspace --all-targets --no-fail-fast`：255 passed，0 failed，1 ignored。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`：通过。

## 结论

渲染、文本、导出和预览边界现已成为可复用的正式 workspace 组件。后续 UI 设计系统可以只围绕稳定的 scene、组件与事件接口工作，不需要重新实现图形语义或导出逻辑。
