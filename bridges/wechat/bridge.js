/**
 * WeChat Bridge - Wechaty-based WeChat integration
 *
 * Uses the NDJSON stdin/stdout protocol to communicate with the Rust backend.
 * Handles QR login, message receiving/sending, and contact management.
 */

const { WechatyBuilder } = require('wechaty');
const QRCode = require('qrcode');
const { Protocol } = require('./protocol');

class WeChatBridge {
  constructor(config) {
    this.config = config;
    this.protocol = new Protocol();
    this.bot = null;
    this._heartbeatInterval = null;
  }

  async start() {
    this.protocol.sendStatus('starting');

    try {
      this.bot = WechatyBuilder.build({
        name: this.config.botName || 'iagenthub-wechat',
        puppet: 'wechaty-puppet-wechat4u',
      });

      this._setupEventHandlers();
      this._setupCommandHandlers();
      this.protocol.startListening();

      await this.bot.start();
      this.protocol.sendStatus('waiting_for_login');

      // Start heartbeat
      this._heartbeatInterval = setInterval(() => {
        this.protocol.sendHeartbeat();
      }, 30000);
    } catch (error) {
      this.protocol.sendError(`Failed to start bot: ${error.message}`);
      process.exit(1);
    }
  }

  _setupEventHandlers() {
    // QR Code for login
    this.bot.on('scan', async (qrcode, status) => {
      if (status === 2) {
        // WechatyScanStatus.Waiting
        try {
          // Generate QR code as base64 data URL for direct display in the UI
          const dataUrl = await QRCode.toDataURL(qrcode, {
            width: 256,
            margin: 2,
            color: { dark: '#000000', light: '#ffffff' },
          });
          this.protocol.sendQrCode(qrcode, dataUrl);
        } catch (e) {
          // Fallback: send the raw text so the frontend can show something
          const fallbackUrl = `https://wechaty.js.org/qrcode/${encodeURIComponent(qrcode)}`;
          this.protocol.sendQrCode(fallbackUrl, '');
        }
      }
    });

    // Login success
    this.bot.on('login', (user) => {
      this.protocol.sendLogin(user.id, user.name());
    });

    // Logout
    this.bot.on('logout', (user, reason) => {
      this.protocol.sendLogout();
    });

    // Incoming message
    this.bot.on('message', async (msg) => {
      try {
        // Skip self messages
        if (msg.self()) return;

        const talker = msg.talker();
        if (!talker) return;

        const content = msg.text();
        if (!content || content.trim() === '') return;

        // Determine content type
        let contentType = 'text';
        const msgType = msg.type();
        if (msgType !== 7) {
          // 7 = Text in Wechaty
          contentType = 'unsupported';
          return; // Only handle text messages for now
        }

        this.protocol.sendMessage(
          msg.id || Date.now().toString(),
          talker.id,
          talker.name(),
          content,
          contentType,
        );
      } catch (error) {
        this.protocol.sendError(`Message handling error: ${error.message}`);
      }
    });

    // Error
    this.bot.on('error', (error) => {
      this.protocol.sendError(`Bot error: ${error.message}`);
    });

    // Ready (fully logged in and contacts loaded)
    this.bot.on('ready', async () => {
      this.protocol.sendStatus('running');

      // Fetch contacts
      try {
        const contacts = await this.bot.Contact.findAll();
        const contactList = contacts
          .filter((c) => c.type() === 1) // 1 = Personal
          .map((c) => ({
            id: c.id,
            name: c.name(),
            avatar_url: null,
            contact_type: 'personal',
          }));
        this.protocol.sendContacts(contactList);
      } catch (error) {
        this.protocol.sendError(`Failed to fetch contacts: ${error.message}`);
      }
    });
  }

  _setupCommandHandlers() {
    // Handle send_message command from Rust
    this.protocol.onCommand('send_message', async (cmd) => {
      try {
        const contact = await this.bot.Contact.find({ id: cmd.to_id });
        if (contact) {
          await contact.say(cmd.content);
        } else {
          this.protocol.sendError(`Contact not found: ${cmd.to_id}`);
        }
      } catch (error) {
        this.protocol.sendError(`Failed to send message: ${error.message}`);
      }
    });

    // Handle get_contacts command
    this.protocol.onCommand('get_contacts', async () => {
      try {
        const contacts = await this.bot.Contact.findAll();
        const contactList = contacts
          .filter((c) => c.type() === 1)
          .map((c) => ({
            id: c.id,
            name: c.name(),
            avatar_url: null,
            contact_type: 'personal',
          }));
        this.protocol.sendContacts(contactList);
      } catch (error) {
        this.protocol.sendError(`Failed to fetch contacts: ${error.message}`);
      }
    });

    // Handle ping command — reply with pong immediately
    this.protocol.onCommand('ping', (cmd) => {
      this.protocol.sendPong(cmd.ts);
    });

    // Handle stop command
    this.protocol.onCommand('stop', async () => {
      await this.stop();
    });

    // Handle logout command (switch account)
    this.protocol.onCommand('logout', async () => {
      try {
        if (this.bot) {
          await this.bot.logout();
          // The bot's 'logout' event handler will fire and send the logout event.
          // Then the bot will automatically trigger 'scan' with a new QR code.
        }
      } catch (error) {
        this.protocol.sendError(`Logout failed: ${error.message}`);
      }
    });
  }

  async stop() {
    if (this._heartbeatInterval) {
      clearInterval(this._heartbeatInterval);
      this._heartbeatInterval = null;
    }

    if (this.bot) {
      try {
        await this.bot.stop();
      } catch (error) {
        // Ignore stop errors
      }
    }

    this.protocol.close();
    process.exit(0);
  }
}

module.exports = { WeChatBridge };
