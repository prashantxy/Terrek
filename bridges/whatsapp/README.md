# WhatsApp bridge

`terrek send whatsapp <number> <message>` posts to this small local server, which
relays through a linked WhatsApp session using [Baileys](https://github.com/WhiskeySockets/Baileys).

```bash
cd bridges/whatsapp
npm install
npm start            # first run prints a QR code: WhatsApp → Linked devices → Link a device
```

- Listens on `127.0.0.1:3000` (`HOST` / `PORT` to change). Terrek finds it through
  `TERREK_WHATSAPP_BRIDGE_URL` (default `http://localhost:3000/send`).
- Session keys are stored in `./auth` (`WA_AUTH_DIR` to change). **Treat that folder like a
  password**: anyone with it can use your WhatsApp account. It is gitignored; never commit it.
- Baileys is an unofficial client. WhatsApp may restrict accounts that automate messaging;
  use it for personal notifications, not bulk sending. For anything commercial, use the
  official WhatsApp Business Cloud API instead.
