/**
 * NDJSON stdin/stdout protocol wrapper for bridge communication.
 * All messages are JSON objects delimited by newlines.
 */

const readline = require('readline');

class Protocol {
  constructor() {
    this._handlers = new Map();
    this._rl = null;
  }

  /** Send an event to the Rust backend via stdout */
  send(event) {
    const json = JSON.stringify(event);
    process.stdout.write(json + '\n');
  }

  /** Start listening for commands from Rust backend via stdin */
  startListening() {
    this._rl = readline.createInterface({
      input: process.stdin,
      terminal: false,
    });

    this._rl.on('line', (line) => {
      const trimmed = line.trim();
      if (!trimmed) return;

      try {
        const command = JSON.parse(trimmed);
        const handler = this._handlers.get(command.type);
        if (handler) {
          handler(command);
        } else {
          this.sendError(`Unknown command type: ${command.type}`);
        }
      } catch (e) {
        this.sendError(`Failed to parse command: ${e.message}`);
      }
    });

    this._rl.on('close', () => {
      process.exit(0);
    });
  }

  /** Register a handler for a specific command type */
  onCommand(type, handler) {
    this._handlers.set(type, handler);
  }

  // Convenience methods for sending specific event types

  sendStatus(status) {
    this.send({ type: 'status', status });
  }

  sendQrCode(url, imageBase64) {
    this.send({ type: 'qrcode', url, image_base64: imageBase64 || '' });
  }

  sendLogin(userId, userName) {
    this.send({ type: 'login', user_id: userId, user_name: userName });
  }

  sendLogout() {
    this.send({ type: 'logout' });
  }

  sendMessage(messageId, senderId, senderName, content, contentType = 'text') {
    this.send({
      type: 'message',
      message_id: messageId,
      sender_id: senderId,
      sender_name: senderName,
      content,
      content_type: contentType,
    });
  }

  sendContacts(contacts) {
    this.send({ type: 'contacts', contacts });
  }

  sendError(error) {
    this.send({ type: 'error', error });
  }

  sendHeartbeat() {
    this.send({ type: 'heartbeat' });
  }

  sendPong(ts) {
    this.send({ type: 'pong', ts });
  }

  /** Stop listening and close the readline interface */
  close() {
    if (this._rl) {
      this._rl.close();
      this._rl = null;
    }
  }
}

module.exports = { Protocol };
