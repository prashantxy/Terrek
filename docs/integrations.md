# Integrations

`terrek send <target>` (also available in the palette) pushes messages from the terminal.
All credentials are read from environment variables, so keep them in your shell profile or a
secrets manager, never in the repo.

| Target | Variables | Notes |
|--------|-----------|-------|
| `slack` | `TERREK_SLACK_BOT_TOKEN` | Slack app with the `chat:write` scope; invite the bot to the channel. Default channel `#general`. |
| `discord` | `TERREK_DISCORD_TOKEN`, `TERREK_DISCORD_CHANNEL_ID` | Bot token; `--channel <id>` overrides the default. |
| `telegram` | `TERREK_TELEGRAM_TOKEN`, `TERREK_TELEGRAM_CHAT_ID` | Create a bot with @BotFather, message it once, then read the chat id from `https://api.telegram.org/bot<token>/getUpdates`. |
| `email` | `TERREK_EMAIL_FROM`, `TERREK_SMTP_USER`, `TERREK_SMTP_PASS`, optional `TERREK_SMTP_HOST` (default `smtp.gmail.com`), `TERREK_SMTP_PORT` (587) | Opens `$EDITOR` for the body. With `--ai`, you describe the email and review the AI draft before sending. Gmail needs an App Password. |
| `x` | none | Posts through the `xmaster` CLI if it is installed; otherwise opens the compose page. |
| `whatsapp` | optional `TERREK_WHATSAPP_BRIDGE_URL`, `TERREK_WA_COUNTRY_CODE` (default `91` for 10-digit numbers) | Requires the local bridge in `bridges/whatsapp`; read its README first. |
| `reddit` | none | Opens a pre-filled submit page. |
