# B5P P7 Publication Check 与导出体验验收

> 状态：DONE（自动验收完成；最终 GUI 路径并入 P8）  
> 日期：2026-09-22  
> 上位计划：`docs/INSTPLOT_STUDIO_PRODUCT_POLISH_SHORT_PLAN.md`

## 完成内容

- finding 按 error、warning、information 分组；每项显示规则 ID、对象、事实原因、潜在影响
  和可执行修复建议。
- 带稳定 node ID 的 finding 可点击，直接同步对象树、selection overlay 和适用 Inspector。
- PDF/PNG 菜单先打开正式导出摘要，显示格式、毫米尺寸、Publication Check 汇总；PNG 额外
  显示持久化 DPI、精确像素尺寸和白色/透明背景选择。
- 当前 PNG DPI 与背景进入 Figure Document，保存重开后保持，并被同一 Publication Check
  与实际 raster export 使用。
- PDF 继续保留真实文字并嵌入固定字体；PNG 支持 300/600/1200 dpi 与受控 alpha 背景。
- 存在 Publication Check error 时导出按钮禁用；warning 保留给用户判断。
- PDF/PNG 均先完整 resolve/encode，再通过同目录原子写入替换目标；失败不会创建或截断为
  看似成功的空输出。

## 正确性约束

- Preview、Publication Check、PDF 和 PNG 仍共享同一个 resolved Display List。
- raster pixel dimensions 只由项目毫米尺寸与所选 DPI 计算，preview zoom 不参与。
- finding 的 `impact` 与 `remediation` 是稳定报告字段，CLI JSON 与 GUI 使用同一事实。
- TIFF 编码仍按总计划留给 B6；本阶段冻结的 raster 参数模型可直接复用。

## 验证结果

- P7 round-trip 测试验证 600 dpi/透明背景保存重开，并确认 Publication Report 使用 600 dpi。
- 每条现行 finding 都必须具有非空 impact/remediation。
- 白底和透明 PNG 均为有效且不同的输出；PDF 字体嵌入检查继续由现有规则和结构测试覆盖。
- workspace tests、Rust 1.98.0 Clippy `-D warnings`、格式与 diff 检查通过。

## 后续边界

- P8 运行 P0 全工作流、统一审计、真实 GUI 路径和 macOS 人工检查，并记录 Windows 未测边界。
- Lite producer/launcher 和任何发布操作均不属于本阶段，按产品负责人要求继续暂停。
