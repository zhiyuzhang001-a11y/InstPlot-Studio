# InstPlot Studio Phase 8：可复用 UI 外壳

状态：已完成
完成日期：2026-09-25

## 外壳契约

`instplot-ui` 新增以下稳定边界：

- `Branding`：产品名、版本、构建标识以及统一的窗口标题/页脚格式；
- `FeatureSet`：数据导入、手动录入、项目文件、标注、出版检查及 PDF/PNG/SVG 导出的能力开关；
- `ShellEvent`：共享外壳可以发出的意图事件；
- `AppServices`：由具体产品把外壳事件翻译为自己的应用事务。

Studio 已实际使用 `Branding` 和 `FeatureSet` 生成标题、页脚与顶栏功能，不存在一套未接入产品的平行 API。

## 组件状态边界

组件说明位于 `crates/instplot-ui/README.md`：

- UI crate 不拥有文档、选择、草稿或撤销栈；
- 产品层拥有窗口开关和应用状态；
- 工具窗口规格只负责默认/最小尺寸及跨平台呈现；
- 预览只消费不可变 `ResolvedDisplayList`。

## SVG 产品路径

Studio 的 `FeatureSet` 声明 SVG 后，文件菜单、设置确认、应用事务和原子导出路径已经完整接通；SVG 不再只是底层后端 API。

## 阶段验证

- Branding 与 FeatureSet 独立单测通过；
- SVG 外壳事件到原子导出事务的集成测试通过；
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过；
- `cargo fmt --all -- --check`：通过。

## 结论

另一个应用现在可以用不同品牌与功能组合复用正式 UI、文本、渲染和导出 crate。Phase 9 将用第二个真实应用验证这一点，而不是依靠接口声明推断可复用性。
