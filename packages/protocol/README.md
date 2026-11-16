# @sonasub/protocol

跨模块通信契约（桌面端 ↔ 引擎 ↔ 扩展）的归属位置。

当前阶段没有已确定的契约，因此本包不含源码，也不声明 `exports`。新增契约时：

1. 先确认调用双方与数据流，再定义类型；
2. 在本包 `package.json` 中按需声明 `exports`；
3. 消费方通过包名导入，不跨应用引用内部源码。

边界见 [docs/architecture.md](../../docs/architecture.md)。
