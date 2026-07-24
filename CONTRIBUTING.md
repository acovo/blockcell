# Contributing to BlockCell / 参与 BlockCell 贡献

感谢你帮助改进 BlockCell。本文说明提交问题、修改代码和准备 Pull Request 时应遵循的基本约定。

Thank you for helping improve BlockCell. This guide describes the basic expectations for issues, code changes, and pull requests.

## 开始之前 / Before You Start

- 对缺陷修复，请描述复现步骤、预期行为、实际行为以及运行环境。
- 对较大的功能或架构变更，建议先创建 issue，确认范围和兼容性要求。
- 不要在公开 issue、日志、测试数据或提交中包含 API Key、Token、密码及个人数据。

- For bug fixes, include reproduction steps, expected behavior, actual behavior, and environment details.
- For substantial features or architectural changes, open an issue first to align on scope and compatibility.
- Never include API keys, tokens, passwords, or personal data in public issues, logs, fixtures, or commits.

## 开发环境 / Development Environment

BlockCell 是 Rust 2021 workspace，最低支持 Rust 1.85。克隆仓库后可运行：

BlockCell is a Rust 2021 workspace with a minimum supported Rust version of 1.85. After cloning the repository, run:

```bash
cargo build --workspace
cargo test --workspace
```

修改单个 crate 时，优先运行对应测试以缩短反馈时间，例如：

When changing one crate, prefer its focused tests for faster feedback, for example:

```bash
cargo test -p blockcell-core
cargo test -p blockcell-skills
```

## 修改原则 / Change Guidelines

- 每个提交只处理一个清晰问题，避免混入无关格式化、生成文件或本地系统文件。
- 缺陷修复应先增加能复现问题的测试，再进行最小实现修复。
- 保持公开配置、CLI 参数、文档和实际运行行为一致。
- Skill 修改必须遵循 [`rules/README.md`](rules/README.md) 中的当前执行规范。
- 不要提交 `.DS_Store`、密钥、本地配置、构建产物或无关的工作区改动。

- Keep each commit focused on one clear concern; avoid unrelated formatting, generated files, or local system files.
- For bug fixes, add a test that reproduces the problem before implementing the minimal fix.
- Keep public configuration, CLI options, documentation, and runtime behavior consistent.
- Skill changes must follow the current execution contract in [`rules/README.md`](rules/README.md).
- Do not commit `.DS_Store`, secrets, local configuration, build artifacts, or unrelated workspace changes.

## 提交前检查 / Pre-Submission Checks

至少运行与改动直接相关的测试，并执行格式检查：

Run at least the tests directly related to the change and check formatting:

```bash
cargo fmt --all -- --check
cargo test -p <affected-package>
```

如果改动跨越多个 crate、共享配置或公共接口，请运行完整 workspace 测试：

For changes spanning multiple crates, shared configuration, or public interfaces, run the full workspace tests:

```bash
cargo test --workspace
```

文档修改还应确认相对链接存在，命令示例与当前 CLI 帮助一致。

For documentation changes, also verify that relative links resolve and command examples match the current CLI help.

## Commit 与 Pull Request

- 使用简洁、可读的提交说明，明确修改目的。
- Pull Request 描述应包括：问题背景、解决方案、验证方式和可能的兼容性影响。
- 保持提交历史可审查；根据维护者反馈补充修复时，避免顺手修改无关文件。

- Use concise, readable commit messages that state the purpose of the change.
- Pull request descriptions should include context, solution, verification, and possible compatibility impact.
- Keep history reviewable and avoid modifying unrelated files while addressing review feedback.

## License

提交贡献即表示你同意所提交内容按照本仓库的 [MIT License](LICENSE) 发布。

By contributing, you agree that your contribution is licensed under this repository's [MIT License](LICENSE).
