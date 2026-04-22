// src/commands/whatsapp.ts
import makeWASocket, {
  useMultiFileAuthState,
  DisconnectReason,
  fetchLatestBaileysVersion,
} from "@whiskeysockets/baileys";
import qrcode from "qrcode-terminal";

let sock: any = null;
let isInitializing = false;

async function autoInit() {
  if (sock || isInitializing) return;

  isInitializing = true;
  console.log("🔄 Auto-initializing WhatsApp...");

  try {
    const { state, saveCreds } = await useMultiFileAuthState("auth");
    const { version } = await fetchLatestBaileysVersion();

    sock = makeWASocket({
      auth: state,
      version,
      browser: ["Ubuntu", "Chrome", "20.0.04"],
    });

    sock.ev.on("creds.update", saveCreds);

    sock.ev.on("connection.update", (update: any) => {
      const { connection, lastDisconnect, qr } = update;

      if (qr) {
        console.log("📱 Scan this QR code:");
        qrcode.generate(qr, { small: true });
      }

      if (connection === "open") {
        console.log("✅ WhatsApp Connected successfully!");
      }

      if (connection === "close") {
        const statusCode = lastDisconnect?.error?.output?.statusCode;
        console.log(`❌ Connection closed: ${statusCode}`);

        if (statusCode !== DisconnectReason.loggedOut) {
          console.log("🔄 Reconnecting in 5s...");
          setTimeout(autoInit, 5000);
        } else {
          console.log("🚪 Logged out. Delete 'auth' folder.");
        }
      }
    });

    // Clean message handler
    sock.ev.on("messages.upsert", (m: any) => {
      for (const msg of m.messages || []) {
        if (msg.message?.protocolMessage) continue;
        if (msg.key.fromMe) continue;

        const from = msg.key.remoteJid;
        const text = msg.message?.conversation ||
                     msg.message?.extendedTextMessage?.text ||
                     "[Non-text message]";
        console.log(`📨 New message from ${from}: ${text}`);
      }
    });

    // Wait for initial sync to complete
    await new Promise(resolve => setTimeout(resolve, 4000));
    console.log("✅ WhatsApp socket is ready.");

  } catch (err: any) {
    console.error("❌ Auto-init failed:", err.message);
  } finally {
    isInitializing = false;
  }
}

// Improved send function with retry logic
async function sendWhatsAppMessage(jid: string, content: any) {
  if (!sock) {
    console.log("⚡ WhatsApp not ready. Auto-initializing...");
    await autoInit();
    if (!sock) throw new Error("Failed to initialize WhatsApp");
  }

  // Retry logic for "Connection Closed" / 428 error
  let attempts = 0;
  const maxAttempts = 12;

  while (attempts < maxAttempts) {
    try {
      const result = await sock.sendMessage(jid, content);
      console.log(`✅ Message sent successfully to ${jid}`);
      return result;
    } catch (err: any) {
      if (err?.output?.statusCode === 428 || 
          err?.message?.includes("Connection Closed") || 
          err?.message?.includes("closed")) {
        
        console.log(`⏳ Connection not fully ready (attempt ${attempts + 1}/${maxAttempts})...`);
        await new Promise(r => setTimeout(r, 1200));
        attempts++;
        continue;
      }
      throw err;
    }
  }

  throw new Error("Connection not ready after waiting. Try again in 10 seconds.");
}

// ===================== EXPORTS =====================
export { sendWhatsAppMessage, autoInit as initWhatsApp };

// CommonJS support for Rust bridge
if (typeof module !== "undefined" && module.exports) {
  module.exports = {
    sendWhatsAppMessage,
    initWhatsApp: autoInit,
  };
}