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

## 折叠区域导航

已有的折叠注释可以直接使用，不必再添加 `@mark`：

~~~rust
// @context-fold begin cache.refresh
fn refresh() {}
// @context-fold end cache.refresh
~~~

用 `markref show cache.refresh`、`find`、`list` 或 `context` 导航。
名称允许任意非空且不含空白的字符，包括中文和 `/`（如 `中文/区域`），
不另设长度限制，仍受扫描文件大小限制。ASCII 标签格式和 128 字符上限只用于 `@mark`。
注释正文须以 `@context-fold begin NAME` 或 `@context-fold end NAME` 开头，
不接受额外字段。每个边界是**独立的导航位置**，`target_line` 指向注释自身，
文本和 JSON 中用 `fold_boundary=begin/end` 区分边界类型。
同名位置会聚合，也能与同名 `@mark` 共存；同名不代表已验证 begin/end 配对、
嵌套关系或完整代码范围。允许孤立或重复边界，`check` 仅检查其格式，不检查配对，
也不要求它们关联附近代码。`context --expand` 仍只输出边界附近的有限代码片段，
不会展开整个折叠区域。

## 安装与使用

~~~sh
cargo install markref --locked
# 安装当前源码：
cargo install --path . --locked
markref list                         # 先发现标签、位置数和职责
markref list --json
markref find "退款"                  # 搜索标签、相邻注释、符号等
markref show payments.refund         # 聚合同标签的全部位置
markref check                        # 检查格式与无法关联代码的位置
markref context payments.refund      # 精简上下文
markref context payments.refund --expand
markref context payments.refund --max-bytes 8000
markref show payments.refund --json
~~~


## Agent 技能

仓库维护 [markref 技能](skills/markref/SKILL.md)，包括有限长度上下文、跨语言
定位、扫描范围及找不到标记时回退到 `rg`。安装 CLI 不会自动安装技能。

首次安装 Codex 技能时，在本仓库执行以下命令；已有同名技能先检查差异再替换：

~~~sh
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills"
cp -R skills/markref "${CODEX_HOME:-$HOME/.codex}/skills/"
cp LICENSE.txt UPSTREAM.toml "${CODEX_HOME:-$HOME/.codex}/skills/markref/"
~~~

重新加载 agent 会话后，可用 `$markref` 调用。其他 agent 按其技能目录约定加载
同一份 `SKILL.md`。技能本身不会擅自给源码添加标记。

## 扫描范围

`list` 聚合同标签的位置数与不同职责，按标签和职责排序。摘要按源码顺序合并
相邻注释，最多读取 8 行、输出 180 个字符，空行分隔不同注释块。
搜索和定位发现扫描诊断时会在 stderr 提醒，JSON stdout 保持可解析；
用 `check --json` 查看完整错误和警告。`--max-bytes` 约束 stdout，不含 stderr。

默认仅扫描当前 Git 项目中已跟踪的源码。使用
`--include-untracked` 才加入未跟踪文件，但仍遵守 Git 忽略规则
（包括 `.gitignore`）。不进入子模块，不跟随符号链接。

快速扫描 `@mark` 或 `@context-fold` 字节串，只有候选文件才解析注释；不同文件并行处理。
第一版不保存持久索引，行号来自本次扫描。相邻注释和符号关联
采用轻量规则，不是完整语法分析，也不能证明 RPC 已经接通。
Rust 原始字符串（含字节、C 字符串前缀）按引号及井号结束符匹配，
避免内部引号、反斜杠造成假标签或漏掉后续真标签。

运行 `cargo test --all-targets` 执行测试。
[性能实验设计](docs/benchmark.md) 说明后续需要验证的指标。
当前没有声称已测得的速度或 Token 收益。

## 许可证

采用 [AUL-1.0-draft.1](LICENSE.txt)，不是 MIT 许可证。
这是自定义草案，尚未获得 OSI 批准。
本项目上游指向自身，见 [UPSTREAM.toml](UPSTREAM.toml)。
