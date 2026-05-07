import makeWASocket, {
  useMultiFileAuthState,
  DisconnectReason,
  fetchLatestBaileysVersion,
} from "@whiskeysockets/baileys";
import qrcode from "qrcode-terminal";

let sock: any = null;
let isInitializing = false;
let connectionState: "idle" | "connecting" | "open" | "closed" = "idle";

async () =>{

}
async function destroySocket() {
  try {
    if (sock) {
      sock.ev.removeAllListeners();
      sock.ws?.close();
    }
  } catch {}
  sock = null;
  connectionState = "closed";
}


async function autoInit() {
  if (sock || isInitializing) return;

  isInitializing = true;
  connectionState = "connecting";
  console.log("🔄 Auto-initializing WhatsApp...");

  try {
    const { state, saveCreds } = await useMultiFileAuthState("auth");
    const { version } = await fetchLatestBaileysVersion();

    sock = makeWASocket({
      auth: state,
      version,
      browser: ["MacOS", "Chrome", "120.0.0"], 
      syncFullHistory: false,
    });

    sock.ev.on("creds.update", saveCreds);

   
    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error("Connection timeout"));
      }, 20000);

      sock.ev.on("connection.update", async (update: any) => {
        const { connection, lastDisconnect, qr } = update;

        if (qr) {
          console.log("📱 Scan this QR code:");
          qrcode.generate(qr, { small: true });
        }

        
        if (connection === "open") {
          console.log("✅ WhatsApp Connected successfully!");
          connectionState = "open";
          clearTimeout(timeout);
          resolve();
        }

        if (connection === "close") {
          const statusCode = lastDisconnect?.error?.output?.statusCode;
          console.log(`❌ Connection closed: ${statusCode}`);

          
          if (statusCode === 515) {
            console.log("⚠️ Pairing complete. Restarting clean session...");

            clearTimeout(timeout);
            resolve(); 
            
            setTimeout(async () => {
              await destroySocket();
              autoInit();
            }, 6000);

            return;
          }

          
          clearTimeout(timeout);
          reject(new Error(`Connection closed: ${statusCode}`));
        }
      });
    });

   
    if (connectionState === "connecting") {
      sock.ev.on("messages.upsert", (m: any) => {
        for (const msg of m.messages || []) {
          if (msg.message?.protocolMessage) continue;
          if (msg.key.fromMe) continue;

          const from = msg.key.remoteJid;
          const text =
            msg.message?.conversation ||
            msg.message?.extendedTextMessage?.text ||
            "[Non-text message]";

          console.log(` ${from}: ${text}`);
        }
      });

      console.log(" WhatsApp socket is ready.");
    }
  } catch (err: any) {
    console.error(" Auto-init failed:", err.message);
    await destroySocket();
  } finally {
    isInitializing = false;
  }
}

// 🚀 SEND FUNCTION
async function sendWhatsAppMessage(jid: string, content: any) {
  if (!sock || connectionState !== "open") {
    console.log("⚡ WhatsApp not ready. Initializing...");
    await autoInit();
  }

  let attempts = 0;
  const maxAttempts = 10;

  while (attempts < maxAttempts) {
    try {
      const result = await sock.sendMessage(jid, content);
      console.log(` Message sent to ${jid}`);
      return result;
    } catch (err: any) {
      const isRetryable =
        err?.output?.statusCode === 428 ||
        err?.message?.includes("Connection Closed") ||
        err?.message?.includes("closed");

      if (isRetryable) {
        console.log(
          ` Waiting for connection (${attempts + 1}/${maxAttempts})...`
        );
        await new Promise((r) => setTimeout(r, 1500));
        attempts++;
        continue;
      }

      throw err;
    }
  }

  throw new Error(" Failed to send: connection never stabilized.");
}

export { sendWhatsAppMessage, autoInit as initWhatsApp };


if (typeof module !== "undefined" && module.exports) {
  module.exports = {
    sendWhatsAppMessage,
    initWhatsApp: autoInit,
  };
}