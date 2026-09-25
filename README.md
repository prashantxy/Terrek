# Terrek

**An AI layer for the shell you already use.** Terrek runs your zsh, bash, or fish inside any
terminal (iTerm, Ghostty, Alacritty, Terminal.app, tmux, SSH) and quietly remembers every
command: what you ran, where, how long it took, whether it failed, and what it printed. When
something breaks, press **Ctrl+T** and type `fix`.

```text
~/api ❯ cargo build
error[E0425]: cannot find value `pool` in this scope
  --> src/db.rs:42:9
↳ exit 101 · Ctrl+T then `fix` for help

terrek › fix
$ cargo build  (exit 101)

`pool` is declared inside the `if let` on line 38, so it's out of scope on line 42.
Move the declaration up, or return early:
    let pool = PgPool::connect(&url).await?;
```

- **Nothing to switch to.** Keep your terminal, prompt, plugins, and keybindings. Terrek is a
  6 MB binary that sits between your terminal and your shell.
- **It already knows the context.** Every question carries your OS, shell, directory, project
  type, git branch, and recent commands with exit codes. `fix` also sends the failed
  command's output.
- **Bring your own model.** Anthropic, OpenAI (or any OpenAI-compatible endpoint: OpenRouter,
  Groq, LM Studio, vLLM), Google Gemini, or **Ollama for fully local, offline use**.
- **Local-first.** History lives in SQLite on your machine (`chmod 600`). Nothing is sent
  anywhere until you ask a question.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/prashantxy/Terrek/main/install.sh | sh
# or
cargo install --git https://github.com/prashantxy/Terrek --locked
```

Then:

```bash
terrek setup     # pick a provider, paste a key (hidden), connection is tested
terrek           # start your shell with Terrek attached
```

Already exporting `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, or `GEMINI_API_KEY`? Skip `setup`;
Terrek picks it up.

To start Terrek automatically in every new terminal, add this to the **end** of `~/.zshrc` or
`~/.bashrc`. If Terrek ever fails to start, you land in your normal shell:

```bash
[ -z "$TERREK_SESSION" ] && command -v terrek >/dev/null && terrek && exit
```

## Use it

Inside a Terrek shell, **Ctrl+T** opens the palette. Anything that isn't a command goes to the AI.

| In the palette            | What it does                                            |
|---------------------------|---------------------------------------------------------|
| `fix`                     | Explain the last failed command and how to fix it       |
| `how do I undo my last commit but keep the changes?` | Ask anything, with context attached |
| `history` · `history --failed` · `search docker` | Browse what you've run          |
| `!make test`              | Run a command in your shell                              |
| `open gh rust-lang/rust` · `open yt lofi` · `open figma` | Open URLs, searches, apps |
| `help`                    | Everything else                                          |

Tab or → accepts the suggestion, ↑/↓ recall earlier entries, Esc closes the palette.

Every palette command is also a CLI command, so it works in scripts and pipes:

```bash
terrek ask "what does this regex do: ^(?=.*\d)(?=.*[a-z]).{8,}$"
kubectl logs api-7d9f | terrek ask "why is this pod crash-looping?"
terrek fix
terrek history -n 50 --failed
terrek doctor --ping
```

### Send messages from the terminal (optional)

```bash
terrek send slack --channel "#deploys" "v2.3 is live"
terrek send discord "build green ✅"
terrek send telegram --photo ./graph.png "latency today"
terrek send email ops@example.com --subject "Incident notes" --ai   # the AI drafts, you review
terrek send x "shipped a thing"
```

Credentials come from environment variables; see [docs/integrations.md](docs/integrations.md).
Build without them using `cargo install ... --no-default-features`.

## Configuration

`terrek config path` shows where the file lives (`~/.config/terrek/config.toml` on Linux,
`~/Library/Application Support/terrek/config.toml` on macOS). Every field is optional:

```toml
[ai]
provider = "anthropic"          # anthropic | openai | gemini | ollama
model = "claude-opus-5"         # defaults per provider
# api_key = "..."               # prefer the provider's env var
# base_url = "https://openrouter.ai/api/v1"

[shell]
# program = "/opt/homebrew/bin/fish"
palette_key = "ctrl-t"          # ctrl-g, ctrl-], ctrl-space, ...
failure_hints = true
integration = true              # the hooks that record commands

[history]
enabled = true
store_output = true             # keep output tails so `fix` works later
max_output_bytes = 8192
```

**Privacy:** start a command with a space to keep it out of history (as with
`HISTCONTROL=ignorespace`). Set `store_output = false` to record commands without their output.

## How it works

```text
 your terminal ─ keys ─▶ terrek ─ bytes ─▶ PTY ─▶ zsh / bash / fish
               ◀─ output ─ strips markers ◀─ output + hook markers
                                │
                     SQLite history ─▶ AI provider (only when you ask)
```

Terrek starts your shell in a pseudo-terminal with small hooks (`preexec`/`precmd` in zsh,
`PROMPT_COMMAND` in bash, events in fish) that emit invisible OSC escape sequences around each
command. Terrek removes them from the output stream and turns them into history records.
Keystrokes are forwarded byte for byte, so vim, fzf, tmux, and mouse input behave exactly as
before. Only the palette key is intercepted, and not while a full-screen app is open. Your rc
files are sourced normally, and nothing on disk is modified.

Source layout:

```text
src/
  main.rs, cli.rs      entry point and the command grammar (shared by CLI and palette)
  sessions/            interactive loop, palette line editor, marker parser + recorder
  pty/                 spawning the shell, zsh/bash/fish hook scripts
  ai/                  provider trait + Anthropic, OpenAI, Gemini, Ollama; prompts; setup
  commands/            ask, fix, history, open, doctor, integrations
  context/             project / git / environment detection
  db/                  SQLite history store and writer thread
  suggestion_engine/   palette completions, background jobs
bridges/whatsapp/      optional Node bridge for `send whatsapp`
experimental/          the tmux-style multiplexer prototype (not built)
```

## Status

| Platform | Shell wrapper | CLI (`ask`, `send`, …) |
|----------|---------------|-------------------------|
| macOS    | ✓ zsh, bash (3.2+), fish | ✓ |
| Linux    | ✓ zsh, bash, fish | ✓ |
| Windows  | not yet | ✓ |

Other shells run fine but their commands aren't recorded.

## Roadmap

- [x] Command recording with exit codes, duration, cwd, output
- [x] Palette with AI ask / fix, history, completions
- [x] Anthropic, OpenAI-compatible, Gemini, Ollama
- [ ] Streaming answers
- [ ] `terrek init zsh` hooks for recording without the wrapper
- [ ] Encrypted history sync across machines
- [ ] Team runbooks: share fixes for recurring errors
- [ ] Multiplexer (panes, sessions), see `experimental/`

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). `cargo test` runs unit tests plus a test that drives
real zsh and bash through a PTY.

## License

MIT © 2026 Prashant Dubey
