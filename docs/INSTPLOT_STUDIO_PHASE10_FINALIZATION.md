# InstPlot Studio Phase 10：清理、文档与持续集成

状态：已完成
完成日期：2026-09-25

## 清理结果

- 删除四个正式 crate 中原型时期遗留的独立 `Cargo.lock`，统一由 workspace 根锁文件管理。
- 生产源码、Cargo 依赖、CI 和当前验证脚本不再引用已迁移的四个 `*-spike` 包。
- 历史 layout compatibility fixture 已更新到正式 crate 路径并单独通过 28 项测试。
- 严格 Clippy 未发现死代码、未使用接口或警告。

## 文档

- ADR：`adr/023-modular-product-boundaries.md`；
- 新产品组装：`docs/INSTPLOT_EXTENSION_GUIDE.md`；
- 各正式 text/render/export/ui/layout crate 均有职责与边界 README；
- 模块化计划为每个阶段保留了可验证证据文档。

## CI

主工作流现在固定执行：

1. workspace 格式检查；
2. 所有 target、示例和集成测试；
3. workspace 文档测试；
4. 全 target、全 feature、`-D warnings` Clippy；
5. 历史 layout 兼容测试；
6. 第二产品的真实 CSV → XY → 标签 → SVG 流程；
7. release 构建及既有项目/交换包/导出检查。

## 规模与构建测量

- 正式 workspace：7 个 package；
- 完整解析依赖图：450 个 package（包含所有平台和可选 feature 的 metadata 结果）；
- `apps/` 与 `crates/` 下 Rust 源文件：70 个；
- 当前机器暖缓存 `cargo check --locked --workspace --all-targets`：约 1.23 秒。

这些数据作为趋势基线，不作为跨机器性能承诺。当前模块均有独立职责或第二 consumer，没有发现需要立即重新合并的无价值细分 crate。

## 最终门禁

- `cargo fmt --all -- --check`：通过；
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`：通过；
- `cargo test --locked --workspace --all-targets --no-fail-fast`：262 passed，0 failed，1 ignored；
- `cargo test --locked --workspace --doc`：通过；
- `cargo run --locked -p instplot-demo ...`：真实生成 204954 字节 SVG；
- Python 验证脚本语法检查：通过。

## 结论

计划的十个阶段全部完成。项目现在有明确的状态所有权、统一的数据/文本/文档/渲染管线、可复用 UI 外壳、第二产品证明以及持续集成边界；后续功能可以沿现有模块扩展，无需重新堆回 Studio 主文件。
