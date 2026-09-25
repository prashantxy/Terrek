import express, { Request, Response } from "express";
import {
  initWhatsApp,        // now works
  sendWhatsAppMessage,
} from "./whatsapp.js";

const app = express();
app.use(express.json());

const PORT = Number(process.env.PORT ?? 3000);
// Loopback only: anyone who can reach this port can send messages as you.
const HOST = process.env.HOST ?? "127.0.0.1";

// Initialize WhatsApp
await initWhatsApp();

// Health check
app.get("/", (_req: Request, res: Response) => {
  res.send("🚀 Terrek WhatsApp Service Running");
});

// Send message endpoint
app.post("/send", async (req: Request, res: Response) => {
  try {
    const { to, message } = req.body as { to: string; message: string };

    if (!to || !message) {
      return res.status(400).json({ error: "Missing 'to' or 'message'" });
    }

    await sendWhatsAppMessage(to, { text: message });

    res.json({ status: "sent", to });
  } catch (err: any) {
    console.error(err);
    res.status(500).json({ error: err.message || "Failed to send message" });
  }
});

app.listen(PORT, HOST, () => {
  console.log(`🚀 Server running at http://${HOST}:${PORT}`);
});