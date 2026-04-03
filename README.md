#  Terrek

> A terminal-native workflow engine with built-in AI. 

If you wanna discuss architecture then reach here :- 

Terrek transforms your terminal into a **programmable, intelligent environment** — combining TUI, automation, and multi-provider AI into a single system.

---

## ⚡ Features

* 🧠 **AI Integration (BYOK)**

  * Supports OpenAI, Gemini, Claude, Ollama
  * Bring your own API key — auto-detected

* 🖥 **Terminal UI (TUI)**

  * Interactive interface built with Rust
  * Multi-pane, system-style experience

* ⚙️ **Command System**

  * Run workflows, scripts, and AI tasks
  * Extensible command architecture

* 🔌 **Modular Architecture**

  * Pluggable AI providers
  * Clean separation of UI, logic, and integrations

---

## 🎮 Demo

```bash
You are in SHELL session. Press Ctrl+T for TERREK mode.

-- TERREK MODE --
[Terrek] > terrek ai analyze my project
[Terrek AI] 🔍 Detected provider: OpenAI
[Terrek AI] ...
```

---

## 🚀 Installation

### One-line install

```bash
curl -fsSL https://terrek.dev/install.sh | bash
```

### Manual

```bash
git clone https://github.com/prashantxy/Terrek.git
cd Terrek
cargo build --release
```

---

## 🧠 AI Setup

Run:

```bash
terrek setup
```

Then paste your API key:

```bash
> sk-xxxx
```

Terrek will:

* auto-detect provider
* configure AI
* store your key locally

---

## ⚙️ Usage

### Enter Terrek mode

```bash
Ctrl + T
```

---

### Run AI commands

```bash
terrek ai "how's my project structure?"
```

---

### Example workflows

```bash
terrek run pipeline
terrek logs
terrek status
```

---

## 🏗 Architecture

```bash
src/
  ai/           # AI providers + abstraction
  commands/     # CLI commands
  ui/           # terminal UI
  config/       # configuration
  app/          # core engine
```

---

## 🔥 Vision

Terrek is not just a CLI tool.

> It’s an attempt to build a **terminal-native system layer** —
> where workflows, AI, and developer tools live together.

---

## 🧩 Roadmap

* [ ] Streaming AI responses
* [ ] Plugin system
* [ ] Workflow engine
* [ ] Remote dashboard (optional web UI)
* [ ] Multi-session support (tmux-like)

---

## 🤝 Contributing

Contributions are welcome.

See `CONTRIBUTING.md` for details.

---

## 🔐 Security

* API keys are stored locally
* Never commit secrets

---

## 📄 License

MIT License © 2026 Prashant Dubey

---

## 💡 Inspiration

Built for developers who believe:

> The terminal is not limited — most tools are.
