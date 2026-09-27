# InstPlot Studio Phase 3 统一数据管线验收记录

日期：2026-09-25
结论：通过；可以进入 Phase 4。

## 1. 统一入口

新增库级 `data.rs` 和无 GUI 的 `DataImporter`：

- `read_file`：统一调用共享 `instplot-io`，并执行 Studio 必需的规范化；
- `import_file`：处理稳定数据集身份、重复导入替换、冲突拒绝、来源/拟合关联和原子合并；
- `import_manual`：处理粘贴列、多 X/Y 配对、重复测量、均值、样本标准差和标准误；
- `DataDiagnostic`：为文件和手动输入提供统一的 `code`、可选行号和原因；
- `DATA_FORMAT_CAPABILITIES`：集中声明文本与 Excel 格式能力，文件选择器从此表生成过滤器。

原二进制私有 `manual_data.rs` 已删除；其纯数据类型、解析函数和测试迁入库。`StudioSession::import_data_file` 只保留兼容包装，实际工作委托给 `DataImporter`。应用事务直接调用 `DataImporter`，文件解析与“加入当前图并自动布局”保持分离。

## 2. 数据与恢复契约

- 普通文件和手动数据进入 `FigureDocument` 后继续以嵌入数据持久化，项目离线恢复测试保持通过。
- 历史 `External` 来源仍按指纹恢复；丢失或变化时返回明确警告，不替换为其他数据，也不泄漏上一工作区缓存。
- 清除后再次导入使用同一管线，与首次导入保持一致。

## 3. 独立验证

新增 `tests/data_pipeline_contract.rs`，完全不启动 `eframe`/Studio 界面即可验证：

- CSV 文件读取并生成标准 `DataSet`；
- 粘贴重复测量生成均值、原始测量列和 SD error 列；
- 文件与手动输入都返回结构化诊断；
- 格式能力表包含 Excel；
- 历史外部源丢失时产生警告且不替代稳定数据集。

现有测试矩阵继续覆盖 TXT/CSV/DAT/TSV、XLSX、空值、数字列名、多来源、同名文件、来源+拟合、重复导入、冲突身份和被移除的数据区。

## 4. 阶段门槛证据

- `cargo fmt --all -- --check`：通过。
- `cargo test --locked --workspace`：218 个测试通过，0 个失败，1 个显式忽略的视觉检查测试。
- `cargo clippy --locked --all-targets -- -D warnings`：通过。
- 独立数据管线集成测试：3 项通过。

## 5. 后续约束

- `DataImporter` 目前作为 `instplot-studio` 库内稳定 module；尚无第二个真实 consumer，不提前拆成 workspace crate。
- 手动数据生成系列建议仍由应用层转为文档命令；Phase 5 可将其收敛成正式 `DocumentCommands`。
- 新增文件格式必须先更新能力表与独立测试，不能只修改文件对话框。
