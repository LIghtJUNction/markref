# markref

从代码注释中维护可共享、可校验的定位引用。使用 Rust 编写，第一版只读。

[English](README.md) · [AUL 许可证](LICENSE.txt)

## 标记格式

~~~rust
// 退款业务入口
// @mark payments.refund role=authority
fn refund() {}

// @mark payments.refund role=caller contract=rpc:Commerce.Refund
fn request_refund() {}
~~~

同一标签可以在多个文件、语言和组件中重复出现，不算重复错误。
标签表示功能，不需要包含语言或目录名。最简单的形式是
`// @mark payments.refund`，`role` 和 `contract` 都是可选字段。

## 安装与使用

~~~sh
cargo install --git https://github.com/LIghtJUNction/markref
markref find "退款"                  # 搜索标签、相邻注释、符号等
markref show payments.refund         # 聚合同标签的全部位置
markref check                        # 检查格式与无法关联代码的位置
markref context payments.refund      # 精简上下文
markref context payments.refund --expand
markref context payments.refund --max-bytes 8000
markref show payments.refund --json
~~~

默认仅扫描当前 Git 项目中已跟踪的源码。使用
`--include-untracked` 才加入未跟踪文件，但仍遵守 Git 忽略规则
（包括 `.gitignore`）。不进入子模块，不跟随符号链接。

快速扫描 `@mark` 字节串，只有候选文件才解析注释；不同文件并行处理。
第一版不保存持久索引，行号来自本次扫描。相邻注释和符号关联
采用轻量规则，不是完整语法分析，也不能证明 RPC 已经接通。

运行 `cargo test --all-targets` 执行测试。
[性能实验设计](docs/benchmark.md) 说明后续需要验证的指标。
当前没有声称已测得的速度或 Token 收益。

## 许可证

采用 [AUL-1.0-draft.1](LICENSE.txt)，不是 MIT 许可证。
这是自定义草案，尚未获得 OSI 批准。
本项目上游指向自身，见 [UPSTREAM.toml](UPSTREAM.toml)。
