# DMA RTL 性能：稳态、任务延迟与内部切点

日期：2026-11-16。对象为当前 1..8-channel 共享 Copy/Fence 切片；硬件行为基线
`8365c8c1`，共享 cells/工具迁移后的生成结果已核对等价。这里是 RTL/VIP 仿真结果，
不是 Rust modeling 的运行速度，也不是已制造芯片的测量。
[2026-11-12 throughput review](throughput-review.md) 的 120/2160-cycle 数字属于较早
单 channel 实现；下面的 121/2161-cycle 数字才对应当前多 channel RTL。

2026-11-18 复核：控制面/执行/传输拆分及参数化后的默认 2、8-channel profile，
在相同的总线宽度、容量、时钟、seed=19 和 VIP 设置下重新运行上述场景，结果保持一致：
16KiB 为 2161 cycles、64B 为 121 cycles、四任务 batch 为 526 cycles，
两种 channel 数的全局 batch 分别为 4336/17386 cycles。读写 payload 均无空泡。
8-channel 内部观测的跨域延迟和两周期 grant→issue 也未变化。验证范围及新的
实现资格状态见 [本轮结果](../../verification/dma/results.md)；这些测量不能外推为
不同数据宽度、slot/credit/FIFO 容量下的性能保证。下面保留原测量定义与明细。
本轮八个种子中，8-channel 全局 batch 的观测范围为 17386..17398 cycles；
17386 是上述匹配种子的比较值，不是所有随机地址与相位下的固定延迟合同。

## 测量配置与边界

* 本轮新编译 2、8-channel 配置，seed=19；每个配置分别执行 long、short、batch、
  half-core ratio、multichannel throughput 和 observer-disabled 正常场景。
* 目标时钟：reg 500MHz，core/bus 各 1GHz；保持独立相位、jitter=0。ratio 场景仅将
  core 降为 500MHz。实现仍是三个异步域，不假定同频等于同步。
* memory VIP：持续 READY、最小合法响应延迟、32 项外部 outstanding 容量。
  “zero-delay”不等于 AXI 请求发出的同一组合周期必须返回响应。
* payload 为双方对齐的 Linear Copy；独立 source/destination buffer。每笔 Copy
  后有一个 Fence，任务 checker 检查 Fence 到来时内存和身份均正确。
* 普通吞吐场景要求 payload 区间读/写各 **零 bubble**，没有为此次测量调宽门槛。
* 本轮没有更改 DUT、重跑综合或声称新的 PPA signoff；只增加可关闭的被动时序观测。

数据由 [VIP performance callback](../../verification/dma/env/coverage/performance.svh)
和 [可选内部 observer](../../verification/dma/env/coverage/pipeline_timing.sv) 获得。
观察者不驱动 ready、寄存器、memory 或 completion，不改变 DUT 调度。没有
`+PDMA_INTERNAL_TIMING` 时，它的两个 initial 进程直接结束，没有逐周期采样成本。

## 1. 先定义指标

| 指标 | 起止点 | 含什么、不含什么 |
| --- | --- | --- |
| payload 吞吐 | 第一个到最后一个实际 R/W beat，跨度加 1 cycle | 区间内所有空泡；不含启动和收尾 |
| descriptor→Fence | Copy 首笔 descriptor AR 握手 → 后继 Fence 写 transaction 完成 | Copy/Fence fetch、CDC、命令/数据执行、收尾、可能的其它 channel 调度 |
| submission→Fence | 软件发布当前任务 → Fence transaction 完成 | 还包含 MMIO 及调度等待；不等于 host 轮询最终读到结果的时间 |
| batch 吞吐 | 第一条 Copy 首 descriptor → 最后一条 Fence 完成 | 包含命令边界开销；分子只算 Copy payload |
| same-burst R→W | 该 burst 最后 R beat → 对应第一 W beat | 数据返回、共享处理、再次跨域和写 endpoint；允许其它 burst 并行 |
| core request roundtrip | core 请求 FIFO 接收 → core 消费对应 receipt | 含队列等待、外部服务和返回，不是一个纯组合逻辑延迟 |

外部 64-bit data port 每方向峰值为 8 B/bus-cycle。1GHz 时对应 **8 GB/s Copy
payload**；同时读和写合计的总线数据流量可以是 16 GB/s，但不能称作 16 GB/s
有效 Copy 带宽。这里 GB 为十进制，KiB 为 1024B。

## 2. 单 channel 活跃：稳态很好，短任务仍有明显固定成本

在 2-channel 和 8-channel DUT 上只使用一个 channel，本轮测量相同：

| 场景 | 每方向 payload beats/span | payload bubbles | descriptor→Fence | 有效 Copy 带宽，包含该区间开销 |
| --- | --- | --- | --- | --- |
| 16KiB，core/bus 1GHz | 2048 / 2048 cycles | 0 / 0 | 2161 bus cycles = 2161ns | 16384/2161 = **7.582 GB/s** |
| 64B，core/bus 1GHz | 8 / 8 cycles | 0 / 0 | 121 bus cycles = 121ns | 64/121 = **0.529 GB/s** |
| 16KiB，core 500MHz、bus 1GHz | 2048 / 2048 cycles | 0 / 0 | 2202 bus cycles = 2202ns | 16384/2202 = **7.441 GB/s** |
| 同 channel 预发布 4×64B Copy/Fence | 每个 payload 仍为 8/8 | 各 0 / 0 | 第一 descriptor→最后 Fence：526 cycles | 256/526 = **0.487 GB/s** |

16KiB 和 64B 两个同频单任务分别有 `2161-2048=113`、`121-8=113` cycles 的区间
开销。这是当前输入/时钟/平台下的测量关系，不是所有地址/负载下的常数合同。
其来源包含两条 96B descriptor 的 fetch、grant/address/decode 切点、跨域、payload
收尾和 Fence；不能把全部 113 cycles 都归给仲裁。

四任务 batch 刻意在 channel disabled 时预发布，再一次 enable，避免软件逐条
MMIO 供给不足污染命令边界数据。`526 - 4*121 = 42` cycles 是这些单任务区间之间
额外的总边界时间。单个 payload 没空泡，不代表连续小任务可以不付出启动/收尾成本。

## 3. 多 channel：共享的是同一份带宽，Fence 也参加仲裁

每个 channel 各发 16KiB Copy + Fence：2-channel 共 32KiB，8-channel 共 128KiB。
每个 Copy 自己的 payload 仍为 8 B/cycle、零空泡，但整个命令的 grant 不在 burst
之间轮换，Fence 是后继独立命令。因此 A 的 Copy 做完后可能先执行 B 的 Copy，
再轮到 A 的 Fence；不能把 Fence 延迟完全解释成 A 的数据仍未写完。

| 配置 | 各 channel 的 descriptor→Fence，bus cycles，按 channel 顺序 |
| --- | --- |
| 2 | 4283，2214 |
| 8 | 17015，14946，12877，10808，8739，6670，4601，2532 |

新加的 whole-batch 汇总直接取所有 metrics 的最早 descriptor 时间和最晚 Fence
时间，不以某个 channel 的窗口冒充全局窗口：

| 配置 | 总 Copy payload | 首 descriptor→末 Fence | aggregate 有效 Copy 带宽 |
| --- | --- | --- | --- |
| 2 channels | 32768B | 4336ns | **7.557 GB/s** |
| 8 channels | 131072B | 17386ns | **7.539 GB/s** |

以上不是每 channel 各有这份带宽；它是所有 channel 共用 master 的总有效速率。

后入选 channel 的起算点更晚，故这列不能直接作 channel 公平性比较；应结合共同
提交起点和 whole-batch 结束点。最早 channel 的 Fence 约 17µs 并不与 payload
8GB/s 矛盾：它在等待本轮其它命令及自己的后继 Fence 授权。

仲裁只有 command-boundary RR，没有抢占/time slice，也不承诺任意外部 stall 下
有界 latency。共享请求 FIFO 的 head-of-line、同 ID 响应顺序还会产生其它排队影响。

## 4. 内部 latency 的实际测量

内部 observer 按 `(channel,slot)` 匹配请求和回执，先处理 terminal receipt，再处理
同边沿 slot 复用，避免把 R→W 直通误认为同一请求尚未结束。它检查未拥有的回执、
重复返回、提前复用以及仿真结束时未归还的请求；统计本身不代替任务/协议 checker。

下表为同频 16KiB 单活跃 channel；2/8 配置结果一致。

| 区间 | 实测 min / mean / max | 样本与解释 |
| --- | --- | --- |
| grant latch → 首 control issue | 2 / 2 / 2 ns，即 2 core cycles | Copy、Fence 各一次；包括 Idle 地址保存切点 |
| window 接收 start → 首 payload issue | 2 / 2 / 2 ns，即 2 core cycles | 1 次；初始化后 allocate，再 issue |
| core issue → bus endpoint 接收 | 4.180 / 4.180 / 4.180 ns | 各类请求；含 request FIFO 跨域及其输出，不含后续 AW/W 队列服务 |
| receipt 入返回 FIFO → core 消费 | 4.820 / 4.820 / 4.820 ns | 各类回执；不含前面的 reader/writer slice 和 merge slice |
| control read roundtrip | 15 / 17 / 19 ns | 4 次：两条 descriptor，各 64B+32B 两段 |
| control write roundtrip | 13 / 13 / 13 ns | Fence 一次 |
| payload read roundtrip | 19 / 43.016 / 68 ns | 256 次；burst 在有序 R lane 排队，故不是固定 19ns |
| payload write roundtrip | 20 / 20 / 20 ns | 256 次；本场景没有额外写排队 |
| last R → first W，同一个 burst | 12 / 12 / 12 bus cycles | VIP 独立观测全部 256 个 burst |

只有 64B 的单 burst 时，payload read roundtrip 为 19ns、write 为 20ns；不出现长任务
填充 window 时的 68ns 读排队。这个差异正说明“通路固定延迟”和“请求响应时间”
不能混为一谈。roundtrip 也不能简单相加成为任务时间，因为许多请求重叠执行。

core 降为 500MHz 时，同一 observer 的结果：

| 区间 | min / mean / max，ns |
| --- | --- |
| grant → first issue；window start → first issue | 两者均 4 / 4 / 4，即仍为 2 core cycles |
| core issue → endpoint | 5.019 / 5.019 / 5.019 |
| 返回 FIFO → core，control 与 payload read | 8.981 / 8.981 / 8.981 |
| 返回 FIFO → core，payload write | 7.981 / 9.958 / 9.981 |
| control read roundtrip | 20 / 22 / 24 |
| control write roundtrip | 18 / 18 / 18 |
| payload read roundtrip | 24 / 36.281 / 66 |
| payload write roundtrip | 24 / 25.977 / 26 |

这些小数包含指定异步相位与接收边沿取整。两级同步器不等于完整 FIFO 通路只有
两个周期：指针可见性、输出寄存器、valid/ready 接收也在路径上。其它时钟比、
jitter、FIFO 占用和外部背压必须重新测量，不能外推这里的 min/max 为体系结构界限。

## 5. 对性能的判断与尚未覆盖范围

当前长且对齐的 Copy 已能持续填满普通 64-bit 总线；在这类 profile 下，继续减少
payload 内部逻辑延迟不一定增加稳态带宽。更明显的成本在：

1. 小任务固定成本：每个 Copy/Fence 都要抓取完整 96B descriptor，且控制访问串行。
2. 命令级授权：长命令保持 engine，增加 peer 与后继 Fence 的等待。
3. 对齐不匹配的 byte path：当前没有通用的数据重排/拼接路径。
4. 有限窗口及共享队头：外部长延迟、方向背压可能使 8 行保留窗口或 FIFO 不足以持续供给。

这次没有测得随机/最坏平台延迟下的完整性能曲面、unmatched alignment 的性能基线，
也没有延迟 SLA。每个 channel 的 row 配额不代表外部独立带宽。错误/park/reset 场景
的正确性已另外验证，不能把上述 zero-delay latency 用作它们的超时界限。

既有控制预算综合在目标时钟达到最差 setup slack 0ps，即无额外整体余量；该结果
不包含真实 SRAM intrinsic timing、布线/时钟树、MTBF 或实际 workload 功耗。RTL
吞吐无 bubble 与物理时序签核是两件事。

## 6. 复现

工具配置使用受信任的私有环境文件；部署路径和原始商业工具报告不进入公共仓库。
例如生成 8 通道 immutable snapshot 后：

```sh
PDMA_CHANNEL_COUNT=8 bash rtl/flow/snapshot.sh "$snapshot"
bash verification/dma/flow/compile.sh "$snapshot" "$build"
bash verification/dma/flow/run.sh "$build" "$run" pdma_zero_delay_long_test 19 +PDMA_INTERNAL_TIMING
```

short、batch、ratio 分别使用 `pdma_zero_delay_short_test`、
`pdma_zero_delay_batch_test`、`pdma_zero_delay_ratio_test`；多 channel 使用
`pdma_multichannel_throughput_test`。输出目录必须新建；各 case 可独立并行。

VIP 输出 `COPY_THROUGHPUT`、`COPY_END_TO_END`、`COPY_HANDOFF`、`COPY_BATCH`，
多 channel 汇总输出 `COPY_MULTI_BATCH`；内部 observer 输出 `PDMA_INTERNAL_TIMING`。
普通运行省略 plusarg，保持原有任务检查而不启用内部采样。
