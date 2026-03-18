// Auto-generated napi-rs loader
// This file loads the correct native binary for the current platform

const { existsSync, readFileSync } = require('fs');
const { join } = require('path');

const { platform, arch } = process;

let nativeBinding = null;
let localFileExisted = false;
let loadError = null;

// Try loading the local .node file first (dev builds)
const localPath = join(__dirname, 'astral.node');
if (existsSync(localPath)) {
  localFileExisted = true;
  try {
    nativeBinding = require(localPath);
  } catch (e) {
    loadError = e;
  }
}

if (!nativeBinding) {
  // Try platform-specific npm packages (release builds)
  const triples = {
    'darwin-arm64': 'astral.darwin-arm64.node',
    'darwin-x64': 'astral.darwin-x64.node',
    'linux-x64': 'astral.linux-x64-gnu.node',
    'win32-x64': 'astral.win32-x64-msvc.node',
  };

  const triple = `${platform}-${arch}`;
  const triplePath = triples[triple];
  if (triplePath) {
    const fullPath = join(__dirname, triplePath);
    if (existsSync(fullPath)) {
      try {
        nativeBinding = require(fullPath);
      } catch (e) {
        loadError = e;
      }
    }
  }
}

if (!nativeBinding) {
  if (loadError) {
    throw loadError;
  }
  throw new Error(
    `Failed to load native binding. Tried: ${localPath}\n` +
    `Platform: ${platform}, Arch: ${arch}`
  );
}

module.exports = nativeBinding;
