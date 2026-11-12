# CDC 报告归因与电路核对

本文保留 2026-11-11 mailbox 检查点的分析与证据。core/bus 已在下一轮改用 FIFO；
当前结构、全部新报告及处置见 [2026-11-12 吞吐评审](throughput-review.md)。下文的
15/12 数量和单 slot 证明不能作为当前数据通路的覆盖。

日期：2026-11-11。本文解释 [最小闭环评审](minimal-loop-review.md) 的全部 24 条
CDC findings，区分源码事实、先行隔离实验和最终处置。**生产 RTL 已删除 `up/live`；
CDC 原始 15 条、RDC 原始 12 条均已逐项审阅，精确例外后的 unresolved 为 0。**
这仅适用于本文固定的源码/配置及冷复位合同，不是物理签核或完整整机形式证明。

## `up` 的实际含义

原 `DuplexMailbox` 的 `s_up/d_up` 在本域 reset 时清零，复位释放后的第一个时钟沿置一。
它们不等待 RAM 初始化、AXI 排空或其它 ready 条件，所以只是本域复位释放标志。
对端同步这个标志后，与本地 `up` 相与，门控两向握手的 valid/ready。
这层逻辑由 PDMA 自己添加；基础 Veryl 握手没有这套 `up/live` 门控。

当前合同要求冷复位共同断言、各域同步释放；不支持有 outstanding 时任意单域热复位。
在这个范围内，基础握手已经能在发送侧保存数据和 request toggle，等接收侧启动后
接收，再返回 ACK。**不需要额外的对端 `up` 来保证这一条消息不丢。**

去掉门控会改变启动期间的接收时机：先释放的一侧可以提前缓存一条消息；接收侧未
运行时不会交付，发送侧也不会覆盖未确认消息。原来则连提前缓存都禁止。真正的
初始化、外部事务排空和单域重启需求仍需要各自的协议，`up` 本身并未提供这些保证。
[reset 草案](reset-cdc.md)、架构与 common-cell 文档已统一这个区别：允许本地暂存
不等于对端已经运行或已经完成额外初始化。

## 全部报告的归属

下表按原始报告计数；去掉门控后，工具可能把同一个逻辑锥的汇聚端点定位到邻接寄存器。

| 类别 | 数量 | 来源及本次结论 | 处置 |
| --- | ---: | --- | --- |
| `CDC_COHERENCY_RECONV_COMB` | 8 | 两个 mailbox 中，额外同步的 `up` 与 request/ACK 分别同步后汇聚 | 已随冗余门控删除而消失 |
| `CDC_COHERENCY_ASYNCSRCS_RECONV_COMB` | 1 | memory request 的 source-ready 锥混入 bus `up`、ACK 和 reg 域启动状态 | 已消失，不是 Veryl 单比特同步器本身的问题 |
| `CDC_COHERENCY_RECONV_SEQ` | 2 | reg/core 的请求和回复 credit；core/bus endpoint 的请求和回执 credit | C1/C2：有意的独立握手组合，无同拍到达要求；精确例外 |
| `CDC_COHERENCY_ASYNCSRCS_RECONV_SEQ` | 1 | core engine 消费 reg 域配置/reset 动作与 bus 域回执/credit | C3：本地优先级、phase 和 drain 账本确定行为；精确例外 |
| `SETUP_RESET_DRIVING_NON_ASYNC_PIN` | 12 | 前端 AW/W/AR 的 6 个 payload 字段和 memory request 的 6 个字段 | R1/R2：冷复位使旧数据失效，新接收覆盖后才可见；精确例外 |

前 9 条是可消除的自有门控；中间 3 条属于 **应用组合协议**；最后 12 条属于
reset 到非异步 reset pin 的 setup 检查。三者不能混称为“同步器有问题”。

剩余 3 条的具体消费约束是：

- 寄存器 bank 只在 `request_valid && request_ready` 时 commit；回复没有存储空间时
  不接受新请求。请求与返回 credit 无需同时变化，valid 必须保持到真正接收。
- memory endpoint 只在 Idle 接受请求，收完 AXI 责任后进入 Receipt，直到回执被
  接收才释放 slot。request 和上一条回执 ACK 的同步延迟不能造成重复接收或提前复用。
- engine 的软件 reset 优先停止发新请求；已经接受的请求仍等待回执，再完成清理。
  reg/core 与 bus/core 没有全局到达次序；不能假设它们同时可见，也不能把软件 reset
  变成 mailbox 的电气 reset。原有软件提交/生命周期顺序要求继续适用。

因此 C1–C3 不是分别同步同一个原子编码的问题。迟到的 credit 只会延迟接收，不能
使已保持的 valid 消失。同步延迟改变 reset 与返回的可见先后时，engine 仍按本地
优先级停止新发射，并等已接受的回执结束后才 clear；不依赖全局同时到达。

R1 的 AW/W/AR source-valid 在 reset 期间关闭，slice 的 stored/valid 与 frontend
active 同时清零。无复位 data FF 即使保留旧值，也不会使 dispatch 成立；下一次
接受同时覆盖 data 并建立 valid。R2 的 request/ACK toggle 和目的端 valid 被冷复位
清零；新请求先锁存 payload、再由同步后的 toggle 触发目的端捕获。软件 reset 不
触碰这些电气 reset。RDC 已穿过无复位存储追踪到消费者，没有发现可观察 corruption。
**无须给所有 payload 添加 reset，也无须再同步这些数据的每一个 bit。**

上述结论限于协调冷复位、本域同步释放及现有单 slot 所有权协议。独立热复位、修改
credit/phase 或增加并发 slot 都须重新分析，不能沿用例外。

### C1：reg 域的新请求与旧回复 ACK，在 core 域汇聚

两条被工具追踪的源信号都产生于 reg_clk：

```text
寄存器请求 toggle ──同步到 core──> request mailbox 的 request_valid
寄存器回复 ACK    ──同步到 core──> reply mailbox 的 source-ready
```

这里 ACK 表示回复已被 reg 域的 frontend 接收，不是 AXI slave 的 B/R 握手。
后者仍由 frontend 自己管理。core 域 register bank 的实际接收条件是：

```text
request_ready = 本地没有待发送回复 || reply mailbox 能接收
commit        = request_valid && request_ready
```

若新请求先可见，本地回复槽空闲时仍可接收，并把新结果留在本地槽；本地槽和
reply mailbox 都忙时，请求保持在 request mailbox 中等待。若 ACK 先可见而尚无
请求，只是释放了回复 credit，不执行任何寄存器操作。因此不要求这两个信号表达
同一时刻的原子状态。工具报告点 `csr/crossing/requests/g_dst_side.valid/D` 是
request-valid 的更新点：新请求使它置位，bank 的接收条件使它清除，后一条件间接
依赖回复 credit，故两条同步路径在这里发生了时序汇聚。

### C2：core 域的新内存请求与旧回执 ACK，在 bus 域汇聚

```text
内存请求 toggle ──同步到 bus──> endpoint 的 port.valid
内存回执 ACK    ──同步到 bus──> receipt mailbox 的 source-ready，即 port.accepted
```

endpoint 的路径是 `Idle → Read/Write → Receipt → Idle`。它在 Idle 接受请求；
收齐外部 AXI 责任后，在 Receipt 保存结果，直到 receipt mailbox 接收该结果。
**交给 mailbox 不等于 core 已消费结果**：随后由 mailbox 持有结果，直到 core
消费并使 ACK 返回，mailbox 才重新有 credit。这与 endpoint 的本地 slot 是两份存储。

新请求可能已到 bus 域，而上一条回执的 ACK 还未到。endpoint 若已空闲可以处理
新请求；若执行结束时 receipt mailbox 仍忙，新的结果就停在 Receipt，既不覆盖
旧回执，也不接受第三条请求。反过来，ACK 先到只释放 mailbox credit，不触发新
内存操作。`endpoint/phase/D[0]` 同时受请求和回执接收条件影响，因而被报告；其
安全依据是各 slot 的所有权转移，而不是两个同步信号必须保持固定相对延迟。

### C3：寄存器控制与内存进度，在 core engine 的 phase 汇聚

这是 reg_clk 和 bus_clk 两个来源汇聚到 core_clk，工具追踪到四条同步 qualifier：
寄存器请求及回复 ACK、内存请求 ACK 及内存回执请求。寄存器那一组决定配置/action
何时在 core 接受；内存那一组分别决定能否提交请求、是否有回执可消费。

engine 在发射 phase 检查 `memory.ready`，在等待 phase 检查 `memory.returned`。
回执先到、请求 credit 后到时，可以消费旧回执，但下一次发射仍等 credit。两者
不是需要原子采样的编码字段。软件 reset 的交错另由 core 本地优先级定义：

| core 观察到的顺序 | 行为 |
| --- | --- |
| reset 先到 | `shutdown` 阻止新发射；已有请求仍等回执，之后才 clear |
| 回执先到 | 可以正常推进；reset 随后接受时，再排空当时实际已接受的请求 |
| reset 与回执同拍可见 | reset 优先，旧回执仍用于排空，不因这条回执再发后继访问 |

这里的 reset 是通过 MMIO 送来的软件操作，不是电气复位穿过 CDC。合同也不保证
软件发出 MMIO 的瞬间就停止 engine；停止边界是 core 接受该操作。这个确定的
本地行为解释了 `engine/phase/D[0]` 的混合来源汇聚为何可以接受。

## 实际 RTL 的收敛验证与门槛

- 新增启动测试在旧 RTL 上因无法提前暂存而失败；修改后 common-cell 9 项、DMA
  13 项原生测试通过，两个 native backend 均覆盖。
- 目标频率与随机异步时钟各 19 个完整 DUT 场景通过。再在 8 条同步链分别注入
  0/1 个接收周期延迟，19 个场景通过；三组各 7 个任务 checker 负例正确拒绝。
- 当时的 `tb/cdc_assertions.sv` 绑定两组实际 mailbox，检查接受到交付之间的数据快照、
  单 slot 所有权、重复交付和背压保持；原有生命周期断言检查 reset 期间不新发射、
  无主回执及排空前不得 clear。这些检查观察真实端口，不推动 DUT，也不复制其 phase。
- 断言资格测试的正常传输通过，数据损坏、重复交付、提前覆盖和破坏背压保持四个
  负例被正确检测。审阅门槛拒绝过期身份、错误目的地以及漏审一条报告；原有未同步
  CDC、独立 reset 损坏、穿过无复位存储的 RDC 损坏与错误端口配置负例仍被拒绝。
- 实际 615/515-bit mailbox 的 2 项 datahold、6 项 dataloss 形式性质通过。整机
  增强形式检查的 3 项 datahold、9 项 dataloss 通过，仍有 1 项 datahold 超时；
  **此项仍为 inconclusive，未加入例外，也未把整机 formal 模式改成通过。**

[`dma-copy-review.tcl`](../../impl/dma/reviews/copy.tcl) 明列 C1–C3
和 R1/R2 的具体目的地、reset 或完整同步 qualifier 集合。例外绑定生成 RTL、源码
manifest、设计 profile、约束、分析配置及工具身份。逐项必须恰好匹配一个 finding；
任何身份变更、缺项或路径变化都要求重新审核。未匹配的额外报告仍使检查失败。
原始报告在应用例外之前保存；同时输出 reviewed 与 unresolved，禁止整类屏蔽。

当前结构审阅结果为 **CDC：raw 15 / reviewed 15 / unresolved 0；RDC：raw 12 /
reviewed 12 / unresolved 0**。未选择该明确审阅策略时，原始流程仍按 findings 失败。
这给出了当前全部结构报告的结论，不免除文末的物理实现义务。

## 先行隔离实验

实验从同一份实际 DUT 生成 RTL 快照出发，仅在忽略的研究目录删除 `DuplexMailbox`
的 `up/live` 状态、同步链及门控，保留两向 Veryl handshake、payload 类型及其 reset
配置。没有改 DUT 的事务、phase、credit 或 AXI 行为。

| 检查 | 观察 |
| --- | --- |
| 完整 DUT 静态 CDC | 24 → 15；恰好少 8 条 COMB 和 1 条 mixed COMB，剩余 3 条 SEQ 与 12 条 setup warning |
| 实际 615/515-bit mailbox 单元 | CDC/formal clean；2 项 datahold、6 项 dataloss 证明通过，未抽象掉内部状态 |
| 原实现与实验版的冷复位释放测试 | 各完成 4 次 request、4 次 response；两种释放顺序、提前提交、接受后改变外部 payload，均无丢失或重复 |
| 启动行为差别 | 原实现提前接收 0 次；实验版提前接收 4 次，均在对端启动后正确交付 |
| 独立同步延迟扰动，随机时钟 | 19 个 DUT tests × 4 seeds，共 76 PASS；7 个 checker 负例被正确拒绝 |
| 独立同步延迟扰动，目标频率 | 19 个 DUT tests × 1 seed，共 19 PASS；7 个 checker 负例被正确拒绝 |

以上不验证独立单域热复位；也不把整机原有 datahold proof 的超时改写成 PASS。
扰动实验在 8 条实际同步链上分别为输入转换注入 0/1 个接收时钟的额外首级捕获延迟，
保持后续级正常移位。每个实例使用独立的层次身份与 run-seed 派生序列；检查了各链
seed 唯一、注入计数非零及不同的延迟序列。没有修改 payload 或 DUT 事务状态。
早期依赖模拟器隐式 RNG 的试验存在序列相关性疑点，已排除出上述证据。
这提供有界数字延迟下的协议证据，不是全部延迟交错的证明或模拟电路级 MTBF 签核。

原始快照、配置、源码哈希、逐场景结果和注入计数留在忽略的本地证据目录，商业工具
原始日志不进入 Git；运行完成时检查并清理本轮拥有的进程。

## 与成熟电路/方法的对照

[Cummings 的 CDC 论文](https://www.researchgate.net/profile/Clifford-Cummings/publication/260384058_Clock_Domain_Crossing_CDC_Design_Verification_Techniques_Using_SystemVerilog/links/54bfe9710cf28a6324a007de/Clock-Domain-Crossing-CDC-Design-Verification-Techniques-Using-SystemVerilog.pdf)
在 §5.3.1 讨论源域合并相关控制，§5.6 讨论保持数据与同步控制、反馈 ACK 的多周期
传输。请求/确认握手是成熟方案，不需要把任意多位 payload 逐位送入同步器。
这里的多周期传输方法不等于对全部跨域路径设置 STA multicycle exception。

[PULP `cdc_2phase` v1.37.0](https://github.com/pulp-platform/common_cells/blob/v1.37.0/src/cdc_2phase.sv)
使用数据保持和 request/ACK 两相握手，没有额外的双方 `up` 互等。它明确要求冷复位
共同断言、分别同步释放，并明确排除普通单域 warm reset；这与我们的当前适用范围
吻合。它同时要求异步数据/控制路径的最大延迟约束，说明握手并不免除物理约束。

[当前固定版本 Veryl handshake](https://github.com/veryl-lang/veryl/blob/8521c8c21e495916ca33648ddab746320953e11d/crates/std/veryl/src/async_handshake/async_handshake.veryl)
在发送接受时锁存 payload、翻转 request，在接收握手后翻转 ACK；接收端捕获期间
发送端持有数据。上述单元证明检验的是它在本设计实际类型/配置下的实例。

对于“在 d 域先合并再同步”，当两个控制共同表达一个条件时，这是合理方向，源域
合并结果应寄存以避免组合毛刺。但 `up` 是复位释放电平，ACK 是事件 toggle，直接
相与不能同时保留这两个含义。此处无需设计新的合并协议，删掉冗余门控更直接。

无复位数据寄存器本身也不罕见，例如
[PULP clearable 版本](https://github.com/pulp-platform/common_cells/blob/v1.37.0/src/cdc_2phase_clearable.sv)
使用不带 reset 的数据 FF，并另外处理控制/隔离；不能据此把它的独立 clear 能力
套到我们的 cell。[Synopsys 的 RDC 说明](https://www.synopsys.com/blogs/chip-design/what-is-reset-domain-crossing.html)
强调继续穿过无复位存储检查下游可观察性。当前 RDC 已这样检查，未发现 corruption
error；原始 12 条 setup warnings 保留，并按 R1/R2 的确切路径批准例外。

### 与 pcie_modeling 已有记录的关系

已核对 [pcie_modeling 的工具链记录](https://github.com/shady831213/pcie_modeling/blob/master/docs/evidence/rtl-flow-toolchain-2026-09-21.md)
中 2026-09-22 的 `RDC-STD-001` 决策及其后 VC SpyGlass 绑定：它明确针对 Veryl
`async_handshake` 源端无复位 `latched_data` 接受定向工程例外，并保留单侧 reset
导致额外输出的独立反例。该记录不代表整库免审或任意复位合同安全。

PDMA 的 12 条应按实现来源分开：

| 本项目报告 | 实现来源 | 与已有记录的关系 |
| --- | --- | --- |
| R2 的 6 个 request payload 字段 | Veryl std `async_handshake` 的源端 `latched_data` | 与 `RDC-STD-001` 是同类无复位存储结构；`INITIALIZE_DATA_OUT` 仅控制目的端数据 reset |
| R1 的 6 个 AW/W/AR 字段 | 从 pcie_modeling 导入的 `common_cell::ReadyValidSlice::Forward` | Forward 自己实现无复位 data FF，不走 `$std::slicer`；不属于 Veryl std 的那份例外 |

共同触发条件是 reset 影响控制 FF，控制 FF 又影响无复位 data FF 的 D/enable。
这不是编译器漏生成 reset；源码就明确选择了“复位有效性、不复位 payload”。
旧项目实例的版本、位宽、复位约束及报告类别与当前 PDMA 不同，不能照搬报告数量
或 waiver。本文保留其设计依据，同时用 PDMA 的实际源码、消费者可观察性和合同
分别审查 R1、R2；这里没有引入参考项目的本地路径或工具部署信息。

2026-11-11 用户确认 `async_handshake` 按 pcie_modeling 的方式处理：保留标准库
实现，对无复位源端 payload 使用定向工程例外，保留原始报告和独立单侧 reset 的
限制。R2 已明确标注这个来源，并维持 PDMA 自己的版本、实例与路径绑定；R1 不混入
该标准库例外。后续替换 cell、改变参数/拓扑或复位合同时重新核对。

## 传输选择与 payload reset的边界

结构检查通过并不代表 mailbox 适合所有流量。当前最小切片采用每方向一个条目的
握手存储，下一条条目需等 ACK 返回后才能接收；大 payload 可以摊薄控制开销，但
不能提供连续条目入队能力。它只证明最小功能闭环，不作为高速 DMA 通路的最终选择。

| 通路 | 选择原则 |
| --- | --- |
| reg_clk ↔ core_clk 的配置/状态访问 | 保留完整请求—响应桥，前端处理 AXI-Lite，寄存器 owner 在 core 执行副作用；两向单项握手是合理的低请求率方案 |
| core_clk ↔ bus_clk 的持续 DMA 请求、结果和 payload | 采用有界异步 FIFO/流式缓冲，结合在途事务窗口、返回容量和 credit 预算隐藏往返延迟；不能继续逐 burst 等待空槽 |
| 单比特状态或通知 | 根据稳定电平、事件计数或握手语义选用适合的同步方式，不统一装进 payload mailbox |

[PULP reg_cdc](https://github.com/pulp-platform/register_interface/blob/master/src/reg_cdc.sv)
用分别握手的请求和响应实现寄存器桥；
[PULP axi_cdc](https://github.com/pulp-platform/axi/blob/master/src/axi_cdc.sv)
则对五个 AXI 通道使用异步 FIFO。它们说明控制事务和持续流量可以采用不同结构，
不意味着直接移植其协议、参数或签核结果。DMA 的具体 FIFO 组织还须与事务身份、
payload 存储和 reset 排空责任一起确定，且读写串行 engine 本身也需解除吞吐瓶颈。

`ReadyValidSlice::Forward` 是同一时钟域内的 slice，不是 CDC cell。它已经复位
valid/占用位；无复位的是 payload。只要复位协调、消费者按 valid 消费，且建立
valid 时已经写入新数据，就不需要靠 payload reset 保证功能。pcie_modeling 的
[common-cell 合同](https://github.com/shady831213/pcie_modeling/blob/master/rtl/common_cell/README.md)
也明确如此。类似地，
[OpenTitan 单项 FIFO](https://github.com/lowRISC/opentitan/blob/master/hw/ip/prim/rtl/prim_fifo_sync.sv)
将 full 状态复位、数据存储不复位。

因此当前不增加仅为消告警的 `RESET_DATA` 参数。该选项可以有合理用途，但应由
具体调用方的复位值需求驱动；增加它意味着额外的 reset 负载及配置验证。确定的
无效期输出值和存储本身复位也是两个不同要求，不能混用。它同样不能修复跨域
独立 reset 的握手生命周期问题。

## 不能用逻辑验证替代的实现义务

最初整域异步例外屏蔽了四个跨域方向的 STA 路径，原映射库中实际查到的路径数均为 0。
本轮已改为实例限定的寄存器间约束：当前两个 mailbox 的双向预算各为 **0.5ns**，
相当于最快 1ns 时钟周期的一半；固定两级同步后的合格捕获为 payload 留出余量。
该数值属于当前配置，不能直接照搬到不同频率、同步级数或握手电路。

Genus 的重新综合验证了 8 组跨域路径、4 组具体 payload 路径和 8 条同步链的本地
级间路径。原生报告显示预期的 500ps Path Delay 预算；最小跨域 setup slack 为
288.8ps，本地同步级间最小 setup slack 为 820.6ps。16 个同步 FF 在映射后保留，
整机最差 setup slack 为 0ps。它们是未布线估计，不是 routed delay 或 MTBF 证明。

共享 profile 与生成的 `clocks.sdc` 被 Genus 和 VC 共同使用；物理路径约束限定于
调用方给出的 IP 实例，独立顶层才创建时钟及全局关系。双实例测试使用同一组时钟、
不同预算，确认没有互相覆盖，并拒绝缺失实例、过紧预算和遮蔽路径的 false-path。
接口、预算推导和集成义务见 [可复用约束说明](../../docs/rtl/constraints.md)。

还应检查同步链的技术映射、保持/布局约束与目标频率下的 MTBF。Veryl 的泛型
synchronizer 参数提供替换同步实现的接口；`STAGES=2` 本身不是 1GHz 的可靠性证明。

### SDC 与结构例外各自负责什么

低速寄存器桥的协议重汇聚 C1，以及 Forward slice 的无复位 payload R1，可以按
前述具体路径接受结构例外；这不删除它们的时序实现义务。当前结构 reviewed gate
与物理 STA 签核分别报告，不能将前者的 unresolved=0 写成后者通过。

VC SpyGlass 支持 `read_sdc`；当前安装版本的命令帮助已确认该入口及
`create_clock`、`set_clock_groups`、`set_max_delay` 等命令。官方
[RDC 说明](https://www.synopsys.com/blogs/chip-design/what-is-reset-domain-crossing.html)
也区分 SDC 的 clock/IO intent 与额外的 reset/RDC Tcl 约束。当前 VC adapter 已使用
SDC 风格的时钟命令及原生 reset/端口归属属性；不是没有时钟域约束。

| 项目 | 约束/检查位置 | 不能据此声称什么 |
| --- | --- | --- |
| 时钟、异步关系及 IO 时序意图 | 共用设计 profile，翻译为各工具支持的 SDC | 声明异步不证明数据安全跨域 |
| reset 极性、协调断言及接口 reset 归属 | VC 原生 reset/RDC Tcl，配合真实集成合同 | 不能把无复位 data FF 虚构为带 reset |
| 握手 payload 最大传播延迟、同步链级间时序及实现保持 | 综合/STA/物理实现约束与实际路径报告 | VC 接受 SDC 不等于已检查布局布线延迟或 MTBF |
| 已审阅的具体结构报告 | 精确字段和源码绑定的 waiver | waiver 不能替代或关闭正常数据路径时序检查 |

C1 不要求两条独立握手的控制信号等延迟，但每一条握手的 bundled payload 必须在
目的端合格捕获前稳定。当前已约束并验证实际 `latched_data` 到目的端捕获寄存器的
最大延迟预算，保留同步器级间的本地时序检查，没有把同步链全部 false-path。
VC 的异步 clock-group 用于结构域关系；STA 中的 clock-group/false-path 例外则必须
检查优先级，确保没有屏蔽本来要检查的 bundled-data 路径。

R1 是同域 Forward slice，不需要为它添加跨域 max-delay。它的正常同步数据/控制
路径仍须满足 setup/hold；有效性复位、同步释放和下游消费条件仍按现有合同检查。
接受这 6 条 reset-to-data 警告不意味着给整条 data/enable 路径设置 false-path。

冗余 `up/live`、结构报告处置及这轮综合路径约束已完成；布局后延迟/偏斜、reset
recovery/removal、同步器技术实现与 MTBF 仍需物理阶段验证，不能靠结构 waiver 解决。
异步 FIFO 是否替换 mailbox 由吞吐/容量需求
决定，不应只为清除报告而更换电路，也不能将整个 common cell blackbox 掉。
