#!/usr/bin/env node

'use strict';

const path = require('node:path');
const { spawnSync } = require('node:child_process');

const TARGETS = Object.freeze({
  'darwin-arm64': ['chutes-build-darwin-arm64', 'chutes-build'],
  'darwin-x64': ['chutes-build-darwin-x64', 'chutes-build'],
  'linux-arm64': ['chutes-build-linux-arm64-gnu', 'chutes-build'],
  'linux-x64': ['chutes-build-linux-x64-gnu', 'chutes-build'],
  'win32-arm64': ['chutes-build-win32-arm64', 'chutes-build.exe'],
  'win32-x64': ['chutes-build-win32-x64', 'chutes-build.exe']
});

function resolveBinary() {
  const override = process.env.CHUTES_BUILD_BINARY;
  if (override && override.trim()) {
    return path.resolve(override.trim());
  }

  const target = `${process.platform}-${process.arch}`;
  const descriptor = TARGETS[target];
  if (!descriptor) {
    throw new Error(
      `Chutes Build does not provide a prebuilt binary for ${target}. ` +
        `Supported targets: ${Object.keys(TARGETS).join(', ')}.`
    );
  }

  const [packageName, executable] = descriptor;
  try {
    return require.resolve(`${packageName}/bin/${executable}`);
  } catch (cause) {
    const error = new Error(
      `The optional package ${packageName} is missing. Reinstall without ` +
        '`--omit=optional`, or install it explicitly with ' +
        `\`npm install -g ${packageName}\`.`
    );
    error.cause = cause;
    throw error;
  }
}

function main() {
  let binary;
  try {
    binary = resolveBinary();
  } catch (error) {
    console.error(`chutes-build: ${error.message}`);
    process.exitCode = 1;
    return;
  }

  const result = spawnSync(binary, process.argv.slice(2), {
    stdio: 'inherit',
    windowsHide: false
  });

  if (result.error) {
    console.error(`chutes-build: failed to start ${binary}: ${result.error.message}`);
    process.exitCode = 1;
    return;
  }

  if (typeof result.status === 'number') {
    process.exitCode = result.status;
    return;
  }

  if (result.signal) {
    process.kill(process.pid, result.signal);
    return;
  }

  process.exitCode = 1;
}

main();
