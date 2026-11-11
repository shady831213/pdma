# 三域 reset、责任排空与通知设计草案

状态：完整多事务实现的设计方案；已实现的最小切片及其验证边界见
[最小 Copy 切片](minimal-copy.md)。软件可观察语义保持
[reset](../../model/modeling/docs/reset.md) 和
[context](../../model/modeling/docs/context-contract.md) 合同；本文给出多时钟、多事务
实现必须维持的不变量。与 [memory-interface](memory-interface.md) 配套。
本阶段仅实现 DMA；外部 CPU、Function/window/context 管理硬件和 firmware 不进 DUT。
涉及这些对象的描述是后续集成责任，当前 testbench 直接驱动 DMA 控制/平台接口。

## 三种完成不得混用

| 事件 | 证明的事实 | 不证明什么 |
| --- | --- | --- |
| reset MMIO 的 B=OKAY | core 接受 reset 请求 | 旧访问已经结束 |
| 单个 AXI R/B 收尾 | 某次访问外部责任结束 | 全部请求/内部数据已排空 |
| RESET.busy 清零 | 该 scope 旧任务、数据和所有外部责任均结束，复位状态已安装 | 被取消的 command 成功、外部写被撤销 |

三个域异步复位、各自同步释放。跨域通道用能证明安全的握手/异步 FIFO，不能逐位
同步地址、数据或任意二进制计数器。空/满和 pointer 同步用于通道内部协议，不是
对某个 channel 的全部清理证明。数据路径反压不能挡住 reset 请求和清理状态观察。

冷复位首先要求平台协调外部 AXI 对端及本端三个域：不能只抹掉本端 ID 表、随后把
对端的旧 R/B 解释成新事务。首版不承诺有 outstanding 时任意单域电气复位后的恢复。
复位释放时各域发布初始化完成，经同步后才能开放相应跨域通道；尚未工作的时钟域
不能靠超时被当作“已初始化”。这不要求三个域同时释放。

## 发射责任账本

建议 core 为每个 channel 维护 `memory_debt[c]`，不是直接同步 bus 域的 outstanding
寄存器。它覆盖的范围从请求离开本地阶段之前开始：

```text
本地准备 -> 已获接收/回执 credit -> 请求 CDC -> bus slot/AXI -> 回执 CDC -> core 消费
             debt +1                                                debt -1
```

每个已计入请求恰有一个终结回执，结果可以是成功、永久失败、PageFault 或发射前取消。
bus 不在 R/B 完成时直接修改 core 计数；core 消费对应回执才减一。接受和减一同拍需
合并 next-state，不能使用两个会互相覆盖的赋值。计数位宽由最大可接受请求数确定，
不允许溢出或饱和后继续接收。保持 slot/receipt identity 直到双方责任真正交接。

这个计数包含 CDC 中尚未对端可见的请求、held VALID、W 已出但 AW 未握手、R/B 未
收齐以及返回中的结果。因此它为零比“同步后的 FIFO empty 且 bus count=0”更强。
但它仍不包含尚未发射的本地阶段、accepted descriptor 或保留 scratch，必须另行清理。
写数据的内部跨域传输也要归属于请求：取消 header 不能留下无法识别的 payload。
若数据尚在通道中，先完成约定的内部传递/丢弃并回收其 credit，才能给终结回执。

## 单 channel reset 流程

1. **Close core admission。** 在 core 接受 reset 的点设置 resetting，禁止受影响
   channel 的新命令接受和新内存请求计入。未拥有资源的候选直接失效；已接受任务仍
   保留其引用与责任，不提前清 producer/consumer 来伪装完成。
2. **Close bus issue。** 独立有界控制通道发送 Close(channel)。bus 把这个 channel
   标为 closed，确认 CloseAck。随后抵达的旧 CDC 请求仍要消费并逐个回 Cancelled；
   不靠关闭一个共享 FIFO 来堵住 peer。CloseAck 只证明 issue gate 已关闭。
3. **Drain committed offers。** 在 bus 观察 Close 前已 offer 的事务继续完成 AXI
   义务。只有完全未 offer 的工作可取消。已发送 W 的 atomic 不能因收到 Close 就
   不再发送 AW；被背压的 VALID 也不能撤回。
4. **Release local owners。** 各阶段取消未发射工作，交还 buffer；收到的旧成功结果
   用于结清责任，但不继续发后继内存操作。error-event 尚未 offer 时可取消；已
   offer 时仍排空，不再为该结果发一个新的 error event。
5. **Install reset state。** 等 CloseAck、memory_debt=0、所有本地 owner 清理完成，
   再清受影响 channel 的任务/游标/配置/故障/本端 IRQ 状态和 accepted references。
6. **Reopen transport。** bus 收到 Open(channel) 并回 OpenAck 后，core 才清 busy。
   此时 channel 配置仍是 reset 值；允许软件重新配置不等于自动开始处理命令。

Close 可先于旧请求跨域到达：旧请求在入队前已计入 debt，因此即使 CloseAck 已返回，
只要这个请求的 Cancelled/完成回执尚未被 core 消费，busy 就不能清零。Open 在全部
旧债务为零后发送，避免新配置的访问撞上尚未解除的 bus gate。

同一 channel 同时只有一个 reset 交互，重复请求 coalesce；Close/Open 的状态和 ACK
在本次交互结束前不复用，不靠有限 generation 绕回来过滤“可能迟到”的 AXI 返回。
DMA-wide reset 包含所有 channel：可逐个关闭，也可用 scope mask，结果须汇总全部
责任而不是仅看当前 engine owner。不能把逐 channel 清理变成复制多套执行引擎。

物理上已经 offer、但对端永久不应答的事务可以阻止 busy 清零；这是保留真实责任，
不是用软件 deadline 把 reset 伪造成成功。peer 状态不能被清除，但共享 W 通道被
对端长期阻塞可能限制 peer 进度，不能声称无条件隔离总线故障。

## 缺页、Abort、Drain 与 reset 的交汇

| 场景 | 处理 |
| --- | --- |
| payload PageFault | 停止该任务新发射，收尾已承诺请求，保留成功读取的数据/已生效进度，park 后接受 resume |
| reset 遇到 parked | 无需修页或 resume，取消保留工作，按账本完成 cleanup |
| reset 遇到正在收尾的 fault | reset 接管清理目标，仍收齐外部责任，不发布一个新的可恢复任务 |
| Drain 遇到普通 false Wait/SEM Wait | 等待；恢复命令仍可被 firmware 接收，不伪造 Busy 完成 |
| Drain 遇到 faulted/parked | 按已有规则报告失败及 bitmap，绑定不撤销，责任不因失败清空 |
| Abort 遇到运行中的目标 command | 保持 command-atomic；若它随后 park，则取消该保留目标 |
| reset 打断维护 | 维护报告失败；reset 独立清理，不再依赖被放弃的 producer frontier |

维护记录 producer frontier 时，必须与对应 channel 的软件 producer 更新和 Fetch
接受点有确定先后。consumer 到达 frontier 只是 descriptor 遍历完毕；accepted work
和 memory_debt 仍需结束。不能把“维护命令失败”与“资源可以释放”合并。

## 同拍边界和操作顺序

建议每个 owner 的 next-state 计算显式表达下列优先级，不由源代码赋值先后偶然决定：

- reset/fatal 的关闭决定优先于同 scope 的新 admission/issue；此前已接受返回照常结账。
- reset 清理优先于 resume；未 faulted 的旧 resume 不预先清掉本拍新 fault。
- Trap ACK 与新 Trap 同拍，sticky cause 置位优先；ACK 不撤回已交付平台的通知。
- ring consumer/producer 和任务引用安装是同一个接受边界；维护不能观察到半个接受。
- 对外返回先留在本地/CDC 资源中，才算该阶段已释放，不能先释放 ID 再找空间存回执。

多个独立 MMIO 入口的请求先经公平仲裁串行化。软件须停止旧 MMIO 并等待其完成后
release/rebind；reset 不替软件取消还没到达 core 的配置写入。这项要求也覆盖旧 fault
handler 发出的 resume；没有 episode token 可替代软件生命周期同步。

## IRQ 和外部 timer

IRQ cause/mask/ACK 属于各 channel；产生通知是状态更新的一部分，不增加一条顶层
“IRQ pipeline”。建议 top 提供 core_clk 同步的 notification_pending/accept 接口，
承载来源和用途，不把单拍脉冲直接跨时钟传递。平台再实现中断线、消息或 MSI 路由。

DMA 本阶段输出 channel 通知和独立 DMA-wide fatal 通知，testbench 扮演接收方。
后续 managed 集成将 channel 通知（含 channel fatal）交给受权 owner，DMA-wide fatal
按现有合同交给 firmware；路由实现不移入 DMA。
身份路由必须在交付平台前稳定；已交付消息不得因重新绑定而变成新 owner 的通知。
可合并通知采用 pending 状态记录，背压不丢掉“需要检查”的事实，不要求为每次 Trap
存无界计数。后到的未屏蔽事件和 ACK 同拍置位优先。软件仍检查原因和队列。
这里的 notification pending 是已触发通知的锁存，不是 cause 与 mask 的组合电平。
只有新事件能设置它；unmask 不补发，平台交付 ACK 也不是软件的 Trap ACK 寄存器。

reset 清除尚由设备拥有的 pending；外部已握手接收的消息不撤回。accept 表示本拍
接收能力，pending && accept 的 core 时钟边沿完成交付，不是稍后返回的旧事件 ACK。
此端口允许 reset 撤销未交付 pending，不能误当成 VALID 必须保持的 AXI 通道。
异步或消息型平台适配器在接收边沿锁存身份，随后独立完成交付；不能把迟到 ACK
接回来清除另一次新 pending。不为尚未交付的通知阻塞 channel reset。

Timestamp 消费一次一致的 u64 外部时间样本，单位由平台决定。首版 DMA timer 输入
要求相对 core_clk 稳定/同步；异步 timer 的原子快照由外部适配器用握手提供，不能
逐位同步 free-running 二进制计数器。DMA reset 不清平台时钟，fault 不重新采样。

## 验证义务

| 故意构造的交错 | 必须成立的不变量 |
| --- | --- |
| 请求刚进入 CDC，Close 先到 bus | 旧请求返回一次取消/完成，debt 清零前 busy 不清 |
| AW 或 W 单独被接受，reset 到来 | 另一半以及全部响应仍完成；无 ID 提前复用 |
| Atomic R/B 分别延迟，reset/Abort 交错 | 不漏任一通道责任，不重做成功更新 |
| 回执已回 core，但 payload owner 还没释放 | 仅 debt=0 不足以结束 reset |
| 同拍 credit 分配/回收或 reset/返回 | 计数不丢更新，关闭边界后无新 issue |
| peer 数据访问、目标 channel 长时间清理 | peer 状态保持，资源可用时能推进 |
| 时钟不同频率/相位、复位释放顺序不同 | 无伪 request/ack，通道初始化完整后才接收 |
| 平台已接收 IRQ 后 reset/rebind | 旧消息仍属于原接收方，新 cause 不被旧 ACK 吞掉 |

这些是针对真实端口的验收义务，不以原生仿真替代 CDC/RDC 或物理 reset 签核。

## VC SpyGlass CDC/RDC 检查门槛

2026-11-11 确认：DMA 必须使用 **VC SpyGlass** 分别运行 CDC 和 RDC。首个检查
对象是实际生成的 `DmaCopy` 及其引用的 common cells；随后随 DMA 实现扩展覆盖。
当前尚未建立 PDMA 的 VC SpyGlass 检查通过证据。

- 明确 reg/core/bus 三域及每个接口的 clock/reset 归属。core 与 bus 即使都为
  1GHz，也没有同步关系；reg 目标为 500MHz。约束必须匹配实际端口和位宽，缺失
  或未分类端口使检查失败，不能用自动猜测的归属得到干净报告。
- CDC 检查 reg/core 请求与回执、core/bus 请求/数据与回执、初始化和清理握手。
  检查同步器、脉冲/事件传递、多位数据稳定性及控制重汇聚；握手 payload 不能
  简单视为可逐位同步的总线。未来 FIFO/Gray-pointer 通路也需按实际结构检查。
- RDC 独立检查异步置位、本域同步释放、不同释放顺序、复位与非复位状态之间
  的传播和跨域重汇聚，包含异步时钟路径。不能仅凭各域有复位同步器就判定通过。
  本地同步释放由内部电路还是集成边界保证，必须据实际实现明确，不假定存在。
- 软件 channel/DMA reset 是停止与排空协议，不是电气 reset。请求、确认和初始化
  信号的跨域结构由静态检查覆盖；不丢失 outstanding、无伪回执、busy 不提前清零
  等时序性质仍由 DUT assertions 和随机异步仿真检查。不得放宽系统 reset 合同
  来隐藏发现；首版未承诺的任意单域热复位恢复也不能冒称已获验证。
- 流程资格验证应包含漏同步的 CDC 负例、reset-domain 不安全传播的 RDC 负例及
  错误端口/约束负例。工具完成、零路径或约束错误均不等于设计通过。
- 保留 raw 与 unresolved 报告；逐项修复或说明受精确路径、源文件、配置和工具
  身份约束的例外。不得照搬其它设计的 waiver 或整类关闭规则。相关源文件或
  拓扑改变后重新审查。

VC SpyGlass 的结构检查不替代同步器 MTBF、reset recovery/removal、握手 payload
最大延迟/偏斜及布线约束。STA 的异步 clock group 或 reset false path 不能抹去这些
物理义务。通用 launcher、设计约束和报告验收代码应进入仓库；部署配置、原始报告
及工艺库内容保持私有。检查工具与流程参考
[pcie_modeling](https://github.com/shady831213/pcie_modeling)，验证结论必须重新绑定 PDMA。
