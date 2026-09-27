# InstPlot Studio 双轴 Phase 3 验收记录

日期：2026-09-27
分支：`codex/dual-axes`
范围：正式 layout、四侧刻度与标签、数据映射、命中区域和 resolved canvas。

## 完成内容

- `Chart` 正式输入增加可选 X2/Y2，以及稳定 NodeId 到轴组合的 series 映射。
- `AxisSpec` 明确记录是否存在可绘制数据；启用但无数据的副轴拥有边缘和设置入口，不显示虚假刻度。
- `LayoutResult` 保存 X1/Y1 及可选 X2/Y2布局和四个轴标签边界。
- 双 Y 时 Y2唯一拥有右侧边缘；双 X 时 X2唯一拥有顶部边缘；单轴仍由 X1/Y1绘制远端镜像刻度。
- series、误差棒、参考线和 annotation connector 均按自己的轴组合进行数据到画布映射。
- 主、副轴共用 locator、formatter、数学负号和文本测量路径。
- 副轴标签、刻度和命中区域进入正式 display list/hit map；回到单轴后不残留副轴 NodeId。
- 副轴所需空间通过外围 canvas 扩展获得，主绘图区宽高保持单轴基线；与图外图例的额外区域使用同一 resolved canvas 迭代。

## 验证结果

- 新增双 Y 独立映射、右侧唯一所有权、画布扩展且主绘图区尺寸不变测试。
- 新增双 X 独立映射、顶部扩展测试。
- 新增无数据副轴无虚假刻度、仍保持边缘唯一所有权测试。
- 新增 document 层双轴正式布局、休眠返回单轴后无副轴命中对象测试。
- 既有 21 项 layout 出版布局测试全部通过。
- Studio lib：151 通过、0 失败、1 个显式忽略的人工视觉夹具。
- Clippy `-D warnings`、格式检查和 `git diff --check`：通过。

## 阶段结论

Phase 3 已把双轴纳入唯一正式布局和导出显示列表路径，没有建立旁路绘制。单轴几何和既有布局测试未改变，双轴数据也不再被错误映射到主轴。下一阶段可在相同 hit map 和 NodeId 基础上接入预览交互与 UI。
