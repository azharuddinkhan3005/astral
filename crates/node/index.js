// napi-rs native binding loader
const { existsSync } = require('fs');
const { join } = require('path');

const { platform, arch } = process;

let nativeBinding = null;
let loadError = null;

// Candidate file names: napi uses the package napi.name for the prefix
const names = ['astral-code', 'astral'];
const triples = {
  'darwin-arm64': 'darwin-arm64',
  'darwin-x64': 'darwin-x64',
  'linux-x64': 'linux-x64-gnu',
  'linux-arm64': 'linux-arm64-gnu',
  'win32-x64': 'win32-x64-msvc',
};

// Build list of candidate paths to try
const candidates = [];
const triple = triples[`${platform}-${arch}`];
for (const name of names) {
  // Platform-specific name (napi build output)
  if (triple) {
    candidates.push(join(__dirname, `${name}.${triple}.node`));
  }
  // Simple name (local dev builds)
  candidates.push(join(__dirname, `${name}.node`));
}

for (const candidate of candidates) {
  if (existsSync(candidate)) {
    try {
      nativeBinding = require(candidate);
      break;
    } catch (e) {
      loadError = e;
    }
  }
}

if (!nativeBinding) {
  if (loadError) throw loadError;
  throw new Error(
    `Failed to load native binding for ${platform}-${arch}\n` +
    `Tried: ${candidates.join(', ')}`
  );
}

module.exports = nativeBinding;
