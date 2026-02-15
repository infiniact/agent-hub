#!/usr/bin/env node

/**
 * WeChat Bridge Entry Point
 *
 * Reads configuration from CHAT_TOOL_CONFIG environment variable
 * and starts the Wechaty-based bridge.
 */

const { WeChatBridge } = require('./bridge');

// Parse configuration from environment
let config = {};
try {
  const configStr = process.env.CHAT_TOOL_CONFIG;
  if (configStr) {
    config = JSON.parse(configStr);
  }
} catch (error) {
  // Send error via protocol before crashing
  const errorEvent = JSON.stringify({
    type: 'error',
    error: `Failed to parse CHAT_TOOL_CONFIG: ${error.message}`,
  });
  process.stdout.write(errorEvent + '\n');
  process.exit(1);
}

// Handle uncaught errors
process.on('uncaughtException', (error) => {
  const errorEvent = JSON.stringify({
    type: 'error',
    error: `Uncaught exception: ${error.message}`,
  });
  process.stdout.write(errorEvent + '\n');
});

process.on('unhandledRejection', (reason) => {
  const errorEvent = JSON.stringify({
    type: 'error',
    error: `Unhandled rejection: ${reason}`,
  });
  process.stdout.write(errorEvent + '\n');
});

// Start the bridge
const bridge = new WeChatBridge(config);
bridge.start().catch((error) => {
  const errorEvent = JSON.stringify({
    type: 'error',
    error: `Bridge startup failed: ${error.message}`,
  });
  process.stdout.write(errorEvent + '\n');
  process.exit(1);
});
