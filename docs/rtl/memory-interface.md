# AXI5 内存事务设计草案

状态：编码前方案，尚未实现。服从 [copy-engine](../../model/modeling/docs/copy-engine.md)、
[context](../../model/modeling/docs/context-contract.md) 和
[page-fault](../../model/modeling/docs/page-fault-recovery.md) 合同。
下列局部队列/状态是硬件事务责任，不是要给 Rust step model 增加周期或 outstanding。

## 接口范围与所有权

首版候选为一个 AXI5 memory master，非一致性接口，不增加 ACE snoop 或 DMA 内部 MMU。
DATA_WIDTH、ADDR_WIDTH、ID_WIDTH 和 outstanding 容量是实例参数；架构地址仍为 u64，
不能因物理地址端口更窄就截断高位。不能表示的请求在发射前失败，映射到原有访问用途
对应的 error/fatal，不另造软件成功路径。地址端口具体支持范围待实现参数矩阵验证。

DMA 请求携带 channel、context selector、地址、长度、访问用途、操作及必要 ordering。
`hw-components` 在送出前查询并授权 Function/context，形成稳定的 address-space 属性。
平台 wrapper 再编码总线 sideband 和 opaque ch_tag。总线 ID 标识正在执行的事务，
不等于 channel、context、Function 或 ch_tag。直连模式不要求实例化 context 表。
PF control memory 仍用独立、enable 冻结的配置；若平台汇聚它与 DMA，仲裁和来源
区分在平台 wrapper，不让 DMA 排空逻辑误清 firmware 的内存请求。

建议物理接口只启用需要的 AXI5 特性：普通 INCR、原生 atomic 所需的 Compare WRAP，
以及显式配置的 request/response USER。地址/数据翻译、ATS/PRI 和 PCIe ID 编码不在
DMA 内实现。atomic 能力由端到端配置决定，不能见到 AXI5 就把 capabilities 置一。
本轮先设计 native atomic 和不支持 atomic 两种组合；exclusive 仍是系统允许的后续
实现选择，不伪称已经设计了第三套端点。

## 一次内部请求及其责任

概念上的内部连接如下，名字不是最终 Veryl 类型名：

| 值/资源 | 内容与职责 |
| --- | --- |
| MemoryRequest | owner channel、接受任务的身份、访问用途、context selector、地址/长度、操作、ordering |
| AuthorizedRequest | 上述信息及经授权的稳定 Function/地址空间属性；仍不拥有任务分配策略 |
| PayloadBuffer | 要发送或保留的数据、有效字节范围、唯一 owner；读写双方转移使用权 |
| TransactionSlot | AXI ID、owner、请求快照、offer/握手状态、预期返回、错误、buffer handle |
| MemoryReceipt | 成功/永久失败/PageFault/发射前取消、返回数据及内部请求身份；只终结这次访问 |

任务引用由 channel 保留；slot 只拥有一次外部访问的责任，不能用 slot 清空替代任务退休。
授权后的身份快照要保留至该次访问结束。绑定仍按既有 context 生命周期锁定；不是
缓存了旧 PASID 就可以让 firmware 提前撤销或重新绑定。

请求接受前必须预留 slot、足够的读/atomic 返回容量、必要的写数据和终结回执容量。
不能先让外部接受请求再等待一次可能失败的内部内存分配。所有容量有界，缺资源通过
READY 反压，不产生虚假的成功或丢失的错误回执。

## Slot、ID 和 AXI 通道推进

建议一个 ID 对应至多一个未完全收尾的 slot，ID 宽度满足可并发 slot 数；不做同 ID
多事务链。普通读、普通写和 atomic 共用唯一 ID 分配域，atomic 的 R/B 路由不会与普通
读混淆。slot 的 ID 在所需返回都已消费、结果已安全交接之后才能复用。
回执在跨域后靠内部请求身份关联 owner，不能重新查询一个已经复用的 AXI slot。
内部请求身份/credit 在 core 消费回执前不复用；bus ID 和这份 credit 的寿命不同。

```text
Reserved -> Offered -> AwaitingReturns -> ReceiptHeld -> Free
    \-------------> CancelledReceipt -------------------/
```

Offered 从任何外部请求通道开始承担不可撤回义务：ARVALID，或者 AWVALID/WVALID
任何一个首次被驱动，都不能再当作可随意丢弃的内部请求。AW 和 W 独立握手；W 先
被接受时也必须最终补齐对应 AW。只有尚未 offer 的请求可因 reset 直接取消。
这比只统计 AR/AW 握手更保守，也覆盖 VALID 被背压的状态。

| 类型 | 外部义务 | 完成/释放条件 |
| --- | --- | --- |
| Read | AR 与全部预期 R beats | 按 ID 收完至正确 RLAST，错误分类完毕，数据或错误回执交接 |
| Write | AW、全部 W beats、B | AW/W 都完成且 B 已接收，结果交接；不以 WLAST 代替 B |
| AtomicStore | AW/W/B | 与写相同，但不能把普通写缓冲行为当成原子更新 |
| AtomicSwap/Compare | AW/W、规定数量 R、B | R/B 都收齐，旧值/错误合并，结果交接 |

写端保留明确的 AW/W 顺序队列，连续 W beats 属于同一选中事务，不交错不同写事务。
准备好事务后 AWVALID/WVALID 不等待对方 READY；可独立背压。RREADY 所需资源在
接受 atomic 前预留，不能等待 W 完成才允许接收 R，从而避免合法返回顺序导致死锁。
AXI 控制/数据在 VALID && !READY 时稳定。总线协议错误（未知 ID、错误 beat 数等）
不能改造成可重试 PageFault；须保留故障并进入设备 Internal fatal 的诊断/停止路径。

读/写/atomic 端点可同时推进，但它们不选择下一个软件 command。事务响应驱动各自
slot 和 channel 的责任账本，不反向调用 Fetch 或重启整个命令流水。

## Burst、访问边界与缓冲

每个 burst 不跨 4KiB，且受最大 beats、数据宽度、有效范围和已预留 buffer 限制。
源/目的 SG 边界独立；不能因为目标连续就越过源段，反之亦然。256 字节是当前 model
的 scratch 实现选择，不是 AXI burst 长度或 RTL 固定 buffer 规格。

不以对齐优化扩大合法访问范围：不能为读取少量 payload 而实际读取 descriptor 范围外
或另一页的额外字节。头尾采用合适的 narrow/unaligned 访问或更小的事务；WSTRB 精确
限定写入字节。读到的无效 lane 不参与 transform。对原子/word 访问保留自然对齐，
不以两笔普通写模拟 64 位原子操作或平台承诺的 non-tearing completion。
较窄总线承载 64 位 Wait/Fence 时，平台仍须保证完整 word 的非撕裂观察；一个
多 beat 普通事务不会自动得到这项保证。未满足的宽度/平台组合不得宣称完整支持。

读成功的数据在相应写安全完成前保留。缺页写重试直接使用保留数据，不能重新读取
源内存或重新执行 transform。所需 RAM 端口和容量由数据保留窗口推导，见
[datapath](datapath.md)。不在本文件偷定可以无限预读或只需一块共享 scratch。

## RESP + USER 原因码

建议分配 USER 响应部分的低 2 位为 `reason`，RUSER 数据附加位与此分开：

| reason | 含义 |
| --- | --- |
| 00 | 无扩展分类；RESP 决定成功或普通永久失败 |
| 01 | 明确的可恢复 not-present PageFault |
| 10/11 | 保留；不得进入 resume 路径 |

错误响应为 SLVERR/DECERR 时只有 reason=01 才是 PageFault；建议翻译端使用 SLVERR。
普通成功要求 reason=00。成功搭配非零 reason、atomic R/B 相互矛盾等属于集成协议
违约，不能猜测“应该重试”。错误搭配未知 reason 按永久失败处理并保留原始诊断信息。
没有 response USER 的平台仍可支持平凡 copy/普通错误，但不能声称支持此显式缺页返回。

平台对 reason=01 必须保证 **这一完整事务未产生需要撤销的效果**，特别是写和 atomic
没有修改目标。一个写 burst 中部分字节已经生效后，不允许仅回 PageFault 让 DMA
整体重发。平台将一个请求拆成多笔下游事务时仍要履行这项承诺；否则它必须返回
不可恢复的访问错误，或者集成协议需另行设计，不能仅靠保留一个 AXI ID 解决。
对 atomic，现有 PLAT-CE-2 更严格：任何报告为失败的操作都必须未修改目标。无法
判断 atomic 是否已生效时不能按普通原子失败重试；这是平台合同违约，需要停止诊断。

建议一次缺页读 burst 的所有 beats 都带相同 reason 和错误分类，仍返回规定 beat 数，
数据被丢弃；非缺页普通错误可以逐 beat 收集，整次访问失败。AXI base 协议并没有
提供事务回滚保证，以上 reason 和“无效果”规则均为 PDMA/平台的附加合同。

同样的底层 PageFault 在不同访问用途上处理不同：

| 用途 | DMA 行为 |
| --- | --- |
| command fetch | CommandRead channel fatal；不消费未解码 descriptor |
| SG metadata | ListReadError，停止并尝试发布 error event |
| payload、Wait、Atomic、SEM、Fence、Timestamp target | 保留任务及所需数据，完成收尾后进入可 resume 的 parked 状态 |
| error-event write | EventWrite channel fatal；不推进 event producer，不隐式重试 |
| firmware control memory | 不由 DMA 分类，由该软件/平台通路处理 |

MMU 向软件报告地址、访问类型、空间和 ch_tag 的路径仍由平台提供。USER 原因码只
告诉 DMA 此次访问的结果，不替代 fault report，不携带软件 handler，也不是 fault
episode token。软件仍须处理完旧报告后才能安全 release/rebind。

## Atomic、Wait、Fence 的落实

建议 Add/Subtract/And/Or/Xor 用 AtomicStore，无软件 old-value 目的地；Subtract 的
操作数取选定位宽内的负数，And 用 CLR 配合位宽内按位取反。Exchange 用 AtomicSwap，
CompareExchange 用 AtomicCompare。CAS 不匹配是成功且不修改，不生成 error event。

Compare 的 compare/swap 顺序和 INCR/WRAP 取决于目标处于双操作数区域的哪一半，
不能固定写成“先 compare 再 swap”。32/64 位操作数和各 DATA_WIDTH 必须交叉验证；
当总线小于操作数时仍由一个合法 atomic transaction 实现，不拆成独立 RMW。

SEM Signal/Wait 用读值、条件判断、CAS。Signal 发现最大值返回 ArithmeticOverflow；
Wait 发现零不写。CAS 竞争失败释放 grant，后续重新读值和仲裁；不同于缺页，不设置
faulted。成功的原子更新必须先记入任务状态，再允许后续退出/通知，避免恢复时再消费
同一 permit。普通 Wait 也释放 grant 重试，Drain 不为它凭空生成成功或 Busy。

Release 操作须等待同 channel 前驱访问达到平台可见性边界后再发射；Acquire 成功后
才允许该 channel 后续访问。Fence 发布前跨 context 等待本 channel 的全部前驱，
不排空其它 channel。等待 B 只有在选定的内存属性和平台合同下才构成所需可见性证明。
首版候选采用 Non-bufferable 请求，并要求平台在返回完成时满足同步域可见性；不把
某个 USER order 位当作已实现缓存一致性，也不默认普通 AXI 就提供 CPU cache snoop。
具体 AxCACHE/AxPROT 组合由端口 profile 表和平台映射冻结，不能由软件任意放宽权限。

Timestamp 的采样及保留在 core，bus 只写给定值；写故障或 bus reset 清理不重新采样。
Trap 在本地 channel 请求通知，不伪造一次 memory transaction，也不是 Fence。

## 检查矩阵与来源边界

- 每种事务覆盖 AW/W 独立背压、R/B 返回先后、返回 backpressure、错误和 reset。
- ID 最小值、非二次幂 slot 数、每个合法 DATA_WIDTH、32/64 原子宽度、Compare 两半地址。
- 不同 context 的同步域、没有原子能力的实例、缺少/损坏 USER 信息。
- 读 fault、保留写 fault、重复 fault、atomic 更新后 reset、SEM CAS 竞争失败。
- 检查 invalid response 不导致“重放一次已经成功的 atomic”。

AXI 协议依据：[Arm IHI0022H](https://developer.arm.com/-/media/Arm%20Developer%20Community/PDF/IHI0022H_amba_axi_protocol_spec.pdf)，
A3/A5 事务规则、A8.3 USER 和 E1.1 atomic。本文的资源组织、reason 编码、完成后的
责任交接和平台可见性要求是 PDMA 设计建议，不是声称这些策略由 AXI 标准规定。
