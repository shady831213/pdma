# Burst window、吞吐与跨域评审

日期：2026-11-12。范围仍是单 channel、32-bit AXI-Lite、64-bit 普通内存的
Linear Copy/Fence 切片。没有扩展 CPU、外围、firmware、多 channel、SG 或 atomic RTL。
结构见 [minimal-copy](minimal-copy.md)；本记录区分体系结构约定、实际测量和签核边界。
最终结构检查绑定 [`dma-copy-review.tcl`](../../impl/dma/reviews/copy.tcl)
中的生成 RTL、实际编译输入、约束、分析/审阅程序和工具身份。

## 所有权与交接

core/bus 的单项 mailbox 改为深度 8 的请求及回执异步 FIFO，低速寄存器桥保留
mailbox。Copy 用 8 个 burst-sized slot 保存源/目的位置、访问状态和需重试的数据。
读、写 endpoint 各自推进；一个 AXI 方向内统一使用 ID 0，依赖 AXI 的同 ID 返回
顺序。内部 slot 身份随回执返回，不把它当作软件 context 或额外的 AXI ID。

一次成功的读可以直接交给有 credit 的写请求，同时保存到保留 RAM。背压时回到
RAM 路径；同步 RAM 的空闲输出没有保留保证，因此有明确的 write-offer 暂存。
头部成功写回执在接收边沿退休，可同边沿安装下一份 slot 元数据，下一周期发请求。
不再经过无用的 Done→Free→Read 空转；乱序/暂停后留下的成功项仍通过 Done 按序退休。

所有已被 core 请求 FIFO 接收的工作都计债，包括还没到 AXI 的请求。软件 reset
停止新接收，继续消费全部回执后才 clear，不 reset FIFO。R 完成与同 slot 的新 W
可以同拍交接：旧读责任结束，新写责任仍存在，不能误认为已经 drained。

这轮也清掉父控制器的旧 Copy 请求格式分支。父层只保留 Fetch、Fence 和错误发布；
Copy 游标、数据及在途责任归 CopyWindow。没有增加另一个调度控制层。

## 错误进度的约定

用户确认 `bytes_completed` 表示**确认 stopping error 时已经确认写成功的连续逻辑
前缀**，不是成功总字节数，也不保证后缀未被修改。同一条 Copy 已发出的后续写可以
完成；后续 command 不越过停止的 command。只发出 W 数据不足以计入成功，必须有
成功回执。reset 仍须排空所有已承诺事务。

RTL 在错误确认时保存成功集合、退休头和已有前缀，随后只从这份固定状态计算 event
中的前缀；晚到回执不参与。模型已经在 write 成功后才 commit，并在错误时构造固定
事件，所以不改模型算法，只补全
[正式规范](../../model/modeling/docs/copy-engine.md#error-records-reporting-and-discovery)。

预读错误可以先于前面 chunk 的写回执被观察到。定向 read-error 场景的首段只有一个
完整 bus beat，因此只接受零或该段全部字节，不接受中间数值；逐字节验证所报前缀。
有序 write error 保留精确值检查和“少报前缀”负例，另有“部分 beat 前缀”负例。
原生窗口测试直接
控制回执交错，检查错误时刻的准确快照。没有把模型的串行访问顺序强加给 RTL。

## 性能测量

采用实际 AXI VIP 回调，基于任务的独立 buffer 范围区分 descriptor、payload 和
Fence。zero-delay 配置有持续 READY、最小合法响应延迟及足够的 VIP outstanding
容量。同时记录 payload 拍跨度、间隔位置、R 完成到 W 开始的交接区间和完整
首 descriptor→Fence 延迟；提交到 Fence 的软件可见时间也保留。

| 配置/场景 | mailbox 基线 | 最终窗口/FIFO |
| --- | --- | --- |
| 16KiB aligned，payload 读/写 | 2048 拍 / 9188 cycles，各 7140 空泡，1.7832 B/cycle | 2048 拍 / 2048 cycles，各 0 空泡，8 B/cycle |
| 同任务，首 descriptor→Fence | 9284 bus cycles | 2160 bus cycles |
| 64B，首 descriptor→Fence | 104 bus cycles | 120 bus cycles |
| 4 个预先发布的 64B Copy/Fence | — | 首 descriptor→末 Fence 共 519 bus cycles |
| core 频率为 bus 的一半，16KiB | — | 读写各 0 空泡；首 descriptor→Fence 共 2200 bus cycles |

外部峰值是每方向 8B/bus-cycle，不使用 DUT 自己的慢速结果作为目标。普通
zero-delay 测试要求从首到末 payload 拍没有空泡；短任务和命令边界成本另行展示。
未对不匹配对齐方式的逐字节路径宣称吞吐闭合。
短任务的额外 16 cycles 是保留窗口、异步队列和返回路径的固定成本；没有将长任务
的收益推广成所有任务都更快。完整任务还包含 descriptor fetch、收尾和 Fence，
因此整体有效带宽不等于 payload 阶段的 8 B/cycle。

同步延迟测试是另一个明确标记的、仅用于仿真的快照。每条同步链独立增加 0/1 个
接收周期的采样延迟；Gray vector 保持整体采样，不捏造任意 bit 间的整周期偏移。
这验证协议对延迟变化的容忍，不模拟模拟电路亚稳态或 MTBF。实现流程拒绝此快照。

早期窗口版本在 half-core 加扰动时有 89/90 个周期性空泡；slot 交接和读写直通
修正后，仅观察到写方向第 5 个 burst 前的 1 个周期瞬态。全部 256 个 burst 的
R 完成→W 开始时间为 19..20 bus cycles，读方向连续。没有通过额外预填延后首拍来
掩盖这个瞬态。扰动 profile 仍要求读方向零空泡；写方向的**整条任务**预算由一次
公平回执仲裁、跨接收时钟的取整，以及两条链各至多一次额外采样周期确定：等频
3 cycles，half-core 5 cycles。预算不随 burst 数增长，不由观测到的 DUT 速度倒推。
普通未扰动构建的预算仍严格为零。

## CDC / RDC 归因与物理约束

原始报告、审阅后的报告和未处置项分别保留。最终快照 CDC 为 40 项：40 项逐路径
审阅、0 项未处置；RDC 为 1363 项：1363 项逐路径审阅、0 项未处置。其中 RDC 的
1330 条路径终止于下面描述的精确 unconnected/no-clock 端点。

| 类别 | 来源与结论 |
| --- | --- |
| 4 个 Gray decode 汇聚 | 源端注册的 Gray pointer 逐步推进，经过两级同步后解码；必须配合传播/skew 预算，不能只看 RTL 拍数 |
| 3 个协议汇聚 | 独立请求、回执和寄存器控制 credit 在本域相遇；晚到只会延迟接收，不要求同时变化；reset Close 与 per-slot 计债保留责任 |
| 32 个 reset-to-data setup 提示 | 前端 payload、窗口元数据、RAM 读数据和 FIFO 数据存储不做内容复位；有效性/占用复位，建立有效性之前有新数据 |
| RAM read data 的 reset mixed-use | reset 控制的 enable 影响无效期数据，不是状态寄存器的异步释放绕过同步；消费者只保存合格读结果 |
| RDC 指向 unconnected/no-clock 的路径 | 精确落在关闭功能的 FIFO controller `i_data` 和 RAM `collision`，没有状态/输出消费者，见下文 |

三个协议汇聚点分别是：

| 汇聚点 / 接收域 | 相遇的信息 | 延迟不一致时的行为 |
| --- | --- | --- |
| `csr/crossing/requests/g_dst_side.valid` / core | reg→core 的新请求通知，与 reg 对上一份响应的接收确认 | 请求数据由请求握手保持；响应确认只是释放返回槽。后者晚到会阻塞接收，不能把另一份响应或请求数据配进来 |
| `endpoint/reader/pending/u_controller/update_state` / bus | 请求 FIFO 同步写指针提供新读请求；回执 FIFO 同步读指针提供返回 credit | 前者控制 AR 接收、tracker push；后者经返回背压控制 R 完成、tracker pop。二者是独立入队/出队条件，延迟只改变 occupancy/停顿，不要求同拍到达 |
| `request_crossing/queue/word_count` / core | 已同步的寄存器请求/返回 credit、请求 FIFO 读指针、回执 FIFO 写指针 | 寄存器动作决定是否继续发新工作；请求 credit 决定是否接收；已到回执释放旧责任。只有本域实际握手修改计数，reset 关闭新工作后仍收齐旧回执，不拿异步信号的同时性当作排空条件 |

这里没有将一份 transaction 的 valid 和 data 分别送进两条独立同步链。FIFO 数据行
在通知消费者前已写好，消费者归还 credit 前不能覆盖；mailbox 的 payload 同样受
完整握手保护。协议汇聚的安全性来自这些所有权条件，不来自假设各同步链延迟相同。

最后一类不是可观察的寄存器损坏：标准 FIFO 只在 `MATCH_COUNT_WIDTH>0` 的
`g_data_match` 中使用 controller 的 `i_data`，本实例参数为 0；实际 payload RAM
在其它路径。`Ram1R1W` 的 `collision` 只被可选 forwarding 分支使用，本实例关闭
该分支，直接使用 raw read 数据。其它 resetless RAM 或实际输出不能套用这个理由。

审阅规则为这些**明确端点**检查 unconnected/no-clock 类型、源 reset 和精确匹配
数量，并绑定全部 source/tool/config/analysis 身份。数量只是覆盖检查，不是安全
证明；不以 tag 整类屏蔽。其它报告继续一项对应一个 finding。原生负例验证过期
身份、错误端点、漏项、连接到寄存器的分组、缺少端点和错误数量都会失败。

Genus 与 VC 使用同源 clock SDC。实例约束对 mailbox 和 FIFO 的双向跨域路径设置
0.5ns 最大延迟，分别核对数据银行路径；同域同步级间及汇聚逻辑保持普通本域时序。
Gray pointer 的预算低于最快源时钟周期，也限制 bit 间 skew。没有猜测性的
`set_multicycle_path`，也没有把整个异步时钟组 false-path 掉。

common-cell 约束拥有私有同步链路径，应用层仅遍历设计 profile。真实映射的双 IP
集成测试分别使用 0.5ns/0.7ns 预算，验证实例隔离、FIFO 深度、缺失路径、两种传输
的不可能预算及覆盖预算的 false-path。源/目的时钟与 reset 策略仍由集成方负责。

## 资格与资源代价

| 检查 | 结果与范围 |
| --- | --- |
| DMA 原生测试 | 17/17，CC 与 Cranelift 均通过 |
| common-cell 原生测试 | 10/10，两个 backend 均通过 |
| 实际 VIP 系统回归 | 目标时钟、随机异步时钟、同步延迟扰动各 48 次通过；共 294 个场景、318 次终态检查 |
| 任务 checker 负例 | 三组各 8 个均以预期诊断拒绝，包括少报前缀和不可能的部分 beat 前缀 |
| 延迟扰动审计 | 8 条链种子各异；两组长任务都实际覆盖每条链的 0/1 周期延迟 |
| 静态审阅门槛 | 正常策略通过；过期身份、错误路径、漏项、连接到真实寄存器的分组、错误数量、缺失端点全部拒绝 |
| 约束集成探针 | 双实例 0.5ns/0.7ns 约束通过；错误 scope/depth、缺失路径、不可能预算及掩盖路径的 false-path 均拒绝 |
| 寄存器 mailbox 形式检查 | 当前 106/66-bit payload、reg/core 时钟边界通过；没有推导成整机证明 |
| 生产 RTL 覆盖率 | 只合并 48 次普通随机时钟正例；场景 64/64 bins，代码未覆盖项保留，详见验证结果 |
| 最终技术映射 | Genus 完整流程退出 0；core/bus 1GHz、reg 500MHz 的 setup 门槛通过，最差 slack 0ps |

回归入口曾因执行期间修改脚本而中断汇总；该批单独保留，没有算作完整通过。
入口调整为执行前完整解析，以上三批均使用稳定脚本重新完成，结果和独立制品
哈希已核对。没有用进程退出码替代任务完成或预期错误诊断。

最终映射使用 74,120 个单元、302 种引用；实际引用审计通过，未使用 ULVT。
Cell area 为 32,267.907，按所选库的面积单位统计，不是芯片面积。clock/input
时序完整性检查的问题数为零。40 个同步 FF 保留；8 组跨域路径、4 组独立数据银行检查和
8 条本域同步链检查均通过。0.5ns 跨域预算保持有效，跨域最小 setup slack 为
211.3ps，本域同步级间最小 slack 为 820.6ps。完整设计的最差 setup 为 0ps，
**没有整体时序裕量**；不能用某一条跨域路径的正裕量替代这个结论。

以上来自本次最终快照的新映射，不复用中间窗口版本的面积或时序数字。输入和
报告制品哈希已复核，工具流程没有未结束的进程。功耗报告只采用无活动向量的
探索性假设，不作为真实 workload 功耗结果。

当前 generic provider 和标准 FIFO 数组没有承诺 SRAM 实现。须如实报告映射后的
存储/逻辑成本；未完成真实宏替换、布局布线、时钟树、hold/reset recovery/removal、
模拟 MTBF 或基于真实任务活动的功耗签核。结构审阅与映射 setup 达标不代替这些工作。
whole-DMA formal 不在已通过声明内；旧快照的 inconclusive 结果不会转写成新快照通过。

公共源码保留可复用 flow、约束及精确例外。商业工具部署、技术库输入和原始报告
留在受忽略的本地证据目录，不进入 Git。快照只指纹实际编译输入并复核文件清单，
文档改动不会造成伪装成硬件变化的重新资格验证。
