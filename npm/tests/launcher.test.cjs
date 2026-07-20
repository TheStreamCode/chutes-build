'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const launcher = path.resolve(__dirname, '..', 'bin', 'chutes-build.js');

test('forwards arguments and the child exit code', () => {
  const result = spawnSync(
    process.execPath,
    [launcher, '-e', 'console.log(process.argv[1]); process.exit(7)', 'FORWARDED'],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        CHUTES_BUILD_BINARY: process.execPath
      }
    }
  );

  assert.equal(result.status, 7);
  assert.equal(result.stdout.trim(), 'FORWARDED');
  assert.equal(result.stderr, '');
});

test('reports an invalid binary override without invoking a shell', () => {
  const missing = path.join(__dirname, 'does-not-exist');
  const result = spawnSync(process.execPath, [launcher], {
    encoding: 'utf8',
    env: {
      ...process.env,
      CHUTES_BUILD_BINARY: missing
    }
  });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /failed to start/);
});
