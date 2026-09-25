# Contributing to Terrek

First off — thanks for your interest in contributing to **Terrek** 
This project is built to push the boundaries of terminal-based systems, and contributions are what make it evolve.

---

##  What is Terrek?

Terrek is a **terminal-first workflow engine** with:

* TUI-based interface
* AI integration (multi-provider support)
* Modular, extensible architecture

We aim to build something closer to a **terminal-native OS layer**, not just a CLI tool.

---

##  Ways to Contribute

You can contribute in multiple ways:

###  Bug Reports

* Found a bug? Open an issue.
* Include:

  * OS (Linux/macOS)
  * Steps to reproduce
  * Expected vs actual behavior

---

###  Feature Requests

* Suggest improvements or new ideas
* Explain:

  * the problem
  * why it matters
  * possible approach (optional)

---

###  Code Contributions

* Fix bugs
* Improve performance
* Add new AI providers (OpenAI, Claude, Ollama, etc.)
* Enhance TUI components

---

## 🏗 Project Structure

The layout and data flow are described in the README's "How it works" section. In short:
`sessions/` runs the interactive loop, `pty/` owns the shell and its hooks, `ai/` holds one
file per provider behind the `ChatModel` trait, and `commands/` implements everything the
CLI and palette can do (both use the grammar in `cli.rs`).

---

## ✅ Before opening a PR

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --no-default-features -- -D warnings
cargo test          # includes a test that drives real zsh and bash through a PTY
```

---

## 🔧 Development Guidelines

### Code Style

* Follow Rust idioms
* Prefer small, modular files
* Avoid large monolithic functions

---

### Architecture Rules

* Do NOT mix UI and business logic
* AI providers implement `ai::ChatModel`; add the kind to `ai/provider.rs` and wire it in `ai/auto.rs`
* Keep provider-specific logic isolated

---

### Commits

Use clear commit messages:

```bash
feat: add OpenAI provider
fix: handle invalid API key
refactor: split AI factory logic
```

---

##  Testing

* Add tests for new logic when possible
* Ensure existing functionality is not broken

---

##  Security

* Never commit API keys or secrets
* Use environment variables or config files

---

##  Pull Request Process

1. Fork the repo
2. Create a branch:

```bash
git checkout -b feature/your-feature
```

3. Make changes and commit
4. Push and open a PR

---

##  Ideas for Contributions

* Add new AI providers (Claude, local LLMs)
* Improve terminal UI animations
* Build plugin system
* Enhance error handling
* Add streaming AI responses

---

##  Philosophy

Terrek is built around a simple idea:

> The terminal is not limited — most tools are.

We aim to build something **fast, composable, and deeply integrated into developer workflows**.

---

##  Final Note

Even small contributions matter.

If you’re unsure where to start, open an issue — we’ll guide you.

---
