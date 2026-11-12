# 最小 DMA 闭环评审记录

这是 2026-11-11 检查点的历史记录。当前 burst window/FIFO 结构和重新验证的结果
见 [吞吐与跨域评审](throughput-review.md)，不复用下文旧快照的结论作为新 RTL 的覆盖。

日期：2026-11-11。范围是现有 `DmaCopy`：单 channel、32 位寄存器端口、64 位普通
内存端口、Linear Copy 与 Fence32/64。最初闭环没有修改 DUT RTL；后续已删除冗余
mailbox 启动门控，并完成 [CDC/RDC 逐项处置](cdc-review.md)。没有扩展 CPU、外围
或其它 DMA 命令。**结构 reviewed gate 通过；物理 CDC/reset 与 MTBF 尚未签核。**

## 场景验证

同一份生成 RTL 快照用于仿真和实现检查；更新后的 testbench 增加目标频率模式，
时钟配置选择由 DUT/TB tests 共享。原来的随机时钟比例、相位和 jitter 场景保留。

| 检查 | 结果 |
| --- | --- |
| reg/core/bus = 500MHz/1GHz/1GHz，19 个独立 DUT test × 8 seeds | 152 PASS；7 个 checker 负例全部被拒绝 |
| 随机异步时钟，19 tests × 2 seeds | 38 PASS；7 个 checker 负例全部被拒绝 |
| 目标频率 TB 自检，5 tests × 2 seeds | 10 PASS；仅自检实例做周期检查 |
| 共享进程清理逻辑后的真实 channel-reset 场景 | PASS |
| 场景功能覆盖 | 已声明的 64 个 bins 全覆盖；不是全部未来功能覆盖 |

目标频率回归的 DUT 模块覆盖率：

| 模块 | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100% | 86.21% | 100% |
| CopyEngine | 93.09% | 78.07% | 85.90% |
| CopyMemoryEndpoint | 98.70% | 81.45% | 90.32% |

覆盖率只合并成功 DUT runs，排除 TB selftests 和负例。这里只证明本切片的功能，
不证明全部分支、参数矩阵、吞吐目标或模拟电路级时序安全。

## 综合评估

使用 12nm 非 ULVT 库、Genus 和三域独立约束。允许库、实际映射单元身份及阈值属性
审计通过；没有用库文件名过滤代替最终审计。库/PVT、部署配置和原始报告保持私有。
本表数据来自删除 mailbox 启动门控之前的快照；不将旧的面积、功耗和 slack 当作
修改后 RTL 的结果。随后带实例约束的重新综合结果另列在表后。

| 项目 | 本次结果 |
| --- | --- |
| 时钟周期 | reg 2ns；core/bus 各 1ns，异步分组 |
| 原生 setup slack | 0.0ps，setup gate 通过，**没有余量** |
| 时序约束覆盖 | 无缺项，检查计数为 0 |
| 映射单元 | 26,587 个；244 种映射引用均通过非 ULVT 审计 |
| Cell area | 10,446.852，按所选库的面积单位；不是芯片面积 |
| Vectorless power | 约 20.409mW，使用显式默认活动参数；不是实测 workload 功耗 |
| generic / map / opt | 约 810 / 325 / 206 秒 |

这些是无布线综合估计，不包含 CTS、寄生、hold、reset recovery/removal、同步器
MTBF 或 CDC payload 路径的物理签核。现有串行 burst/mailbox 数据通路仍有明确的
吞吐限制，见 [最小 Copy 切片](minimal-copy.md)，不能把频率目标通过解释为吞吐闭合。

进一步用同一映射数据库把 core 周期收紧到 0.9ns，原生 slack 变为 -100ps，流程正确
拒绝；这是门槛灵敏度测试，不改变 1ns 的设计目标。最新端口解析与映射时的有效 SDC
已比较一致；差异仅是工具把三个 reset-assertion false path 合并成一个列表。

随后完成的共享约束收敛使用删除启动门控后的真实 RTL 重新综合。两组 mailbox
双向各约束 0.5ns，Genus 与 VC 读取同源生成的时钟 SDC；IP 约束不创建平台时钟。

| 当前约束后的检查 | 结果 |
| --- | --- |
| 8 组跨域路径及其中 4 组 payload | 预期 500ps 预算实际生效；最小 setup slack 288.8ps |
| 8 条本地同步级间路径 | 正常本地时序保留；最小 setup slack 820.6ps |
| 同步结构 | 16 个同步 FF 在映射后保留，每链仍为两级 |
| 整机 setup | 最差 0.0ps，仍无整体余量 |
| 非 ULVT 映射审计 | 25,719 个实例、263 种引用通过 |
| generic / map / opt | 约 824 / 344 / 234 秒 |
| 集成范围与门槛负例 | 同时钟两实例的 0.5/0.7ns 预算互不覆盖；缺失实例、0.001ns 过紧预算、遮蔽路径的 false-path 均被拒绝 |

完整输入与报告保存在本地忽略目录；这不是布局布线、hold/reset 或 MTBF 签核。
集成接口与复用边界见 [约束说明](../../docs/rtl/constraints.md)。

## 首轮 CDC/RDC 结果与解释

本节先保留清理前的基线和当时的待办；后续实际 RTL 的处置结果见本节末及
[CDC/RDC 结论](cdc-review.md)，不要把两份快照的结果混用。

VC SpyGlass 使用实际 DUT 与 common cells。端口位宽、方向、clock/reset 归属完整
校验。冷复位共同断言来自现有合同；各域仍分别同步释放。没有假定独立热复位安全，
没有导入其它设计的 waiver，也没有关闭整类规则。

| 结果 | 当前证据与处置 |
| --- | --- |
| CDC 原始 24 条 findings | 12 条控制重汇聚，以及 12 条 reset-to-data setup warning；仍以非零状态退出 |
| 其中 8 条组合重汇聚 | 对应的形式 Gray-encoding 检查通过；原始结构 findings 仍保留 |
| 其余 4 条重汇聚 | 涉及独立请求/回执握手、启动状态和接收方 phase；需要按协议语义审阅，不是一个多位数据字被逐位同步 |
| DUT 级形式检查 | 12 项 dataloss、3 项 datahold 通过；memory request 的一项 datahold 在时限内未证明，未记为 PASS |
| 实际 memory mailbox 单元证明 | 保持原 RTL 和 615/515-bit payload 类型、自由接口输入；两向 datahold、8 项 dataloss、4 项组合重汇聚性质通过。用于补充内部单元证据，不把 DUT 级 timeout 改写为通过 |
| RDC 可观察性检查 | 追踪穿过无复位存储后的消费者，剩余 12 条 setup warning；仍以非零状态退出 |

四个待审的控制重汇聚点分别涉及 memory request 的 source-ready、engine phase、
寄存器 owner 的 commit 和 memory endpoint phase。它们消费的是独立握手结果；
需要确认的合同是不要求这些控制位同时到达、接收方仅在相应 phase/credit 成立时消费。
这与要求一整个编码值原子跨域不同。

12 条 setup warning 的目的地是寄存器前端的 payload slices 和 memory request 的
`latched_data`。这些存储不复位，valid/所有权状态复位；无效 payload 不得被消费者
使用。RDC 已沿这些存储追踪到可观察端点，但 setup warning 本身仍需明确结案。
不应仅为消除这类报告而给所有 payload FF 添加复位。

建议接下来一起 review 上述协议不变量与定向处置范围。接受例外时应绑定具体路径、
源文件、工具和配置身份，保留 raw/waived/unresolved 区分；在这一步完成之前，当前
流程不输出 CDC/RDC clean。异步物理路径预算与布线约束仍是后续实现义务。

随后完成的 [CDC 归因与收敛](cdc-review.md) 将 12 条重汇聚进一步分开：9 条由 PDMA
额外的 `up/live` 门控引入；其余 3 条属于请求/回复 credit 与 engine phase 的应用
组合。生产 RTL 删除门控后，实际 mailbox 单元 CDC/formal clean；整机 CDC 的 15 条
和 RDC 的 12 条已按确切路径、源码/工具/分析身份审阅，原始报告保留，unresolved 均为 0。
新增启动测试先失败后通过；两个 native backend 的 common-cell 9 项与 DMA 13 项
均通过。完整 DUT 普通回归 38 项、独立同步延迟扰动 19 项通过；三组各 7 个 checker
负例正确拒绝。增强的整机形式检查仍有一项 datahold inconclusive，未计为 PASS。

## 流程质量与复用

- 设计 profile 集中定义边界；Genus/VC 只做各自的接口转换和分析。
- 共享运行目录、输入/输出身份、失败记录和进程组清理。长命令启动前完整解析入口；
  不依赖 Bash 在运行途中继续读取可能被修改的 launcher。
- 真工具负例覆盖无同步跨域、跨 reset 域错误、穿过无复位存储的错误，以及缺失端口。
- 库策略负例覆盖 ULVT 名称、单元/库的隐藏阈值属性、未批准引用和空映射。
- 运行器检查覆盖非零退出、超时、陈旧目录、运行中输入改变及顽固子进程清理。
- 初次综合虽然产生了报告，但 launcher 的收尾读取受到编辑影响；该次未作为通过
  证据。修正后已完成新运行并保存完整身份和退出记录。

重现入口见 [实现检查](../../impl/dma/README.md)；原始证据保存在忽略目录中，
不向公开仓库上传商业工具日志、库内容或部署信息。
