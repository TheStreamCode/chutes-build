'use strict';

const assert = require('node:assert/strict');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const launcher = path.resolve(__dirname, '..', 'bin', 'chutes-build.js');
const platformPreload = path.join(__dirname, 'platform-override-preload.cjs');

// All six real (platform, arch) targets the launcher knows how to map to a
// package name -- mirrors TARGETS in bin/chutes-build.js without importing
// it, so this file stays a black-box test of the launcher's behavior.
const KNOWN_TARGETS = [
  ['darwin', 'arm64'],
  ['darwin', 'x64'],
  ['linux', 'arm64'],
  ['linux', 'x64'],
  ['win32', 'arm64'],
  ['win32', 'x64']
];

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

test('forwards multiple arguments in order', () => {
  const result = spawnSync(
    process.execPath,
    [
      launcher,
      '-e',
      'console.log(JSON.stringify(process.argv.slice(1)))',
      'first',
      'second arg',
      '--flag=value'
    ],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        CHUTES_BUILD_BINARY: process.execPath
      }
    }
  );

  assert.equal(result.status, 0);
  assert.deepEqual(JSON.parse(result.stdout.trim()), [
    'first',
    'second arg',
    '--flag=value'
  ]);
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

test('a binary override path containing spaces is not truncated or split', () => {
  // path.resolve() treats its argument as one path, never splitting on
  // whitespace the way a shell would -- this pins that guarantee at the
  // launcher's actual boundary. Nonexistent path: only the error path
  // (and the exact text of the path it names) is under test here.
  const missing = path.join(
    os.tmpdir(),
    'chutes build launcher test dir',
    'does not exist binary'
  );
  const result = spawnSync(process.execPath, [launcher], {
    encoding: 'utf8',
    env: {
      ...process.env,
      CHUTES_BUILD_BINARY: missing
    }
  });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /failed to start/);
  assert.ok(
    result.stderr.includes(missing),
    `expected the full space-containing path verbatim in: ${result.stderr}`
  );
});

test('reports an unsupported platform/architecture clearly', () => {
  const result = spawnSync(
    process.execPath,
    ['--require', platformPreload, launcher],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        CHUTES_TEST_PLATFORM: 'sunos',
        CHUTES_TEST_ARCH: 'ia32'
      }
    }
  );

  assert.equal(result.status, 1);
  assert.match(result.stderr, /does not provide a prebuilt binary for sunos-ia32/);
  assert.match(result.stderr, /Supported targets:/);
});

test('reports a clear recovery message for a missing optional native package', () => {
  // Pick any known target that is NOT the platform actually running this
  // test, so its optional package is guaranteed not to be installed here --
  // this exercises the real require.resolve failure path, not a mock.
  const [otherPlatform, otherArch] = KNOWN_TARGETS.find(
    ([platform, arch]) => platform !== process.platform || arch !== process.arch
  );

  const result = spawnSync(
    process.execPath,
    ['--require', platformPreload, launcher],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        CHUTES_TEST_PLATFORM: otherPlatform,
        CHUTES_TEST_ARCH: otherArch
      }
    }
  );

  assert.equal(result.status, 1);
  assert.match(result.stderr, /is missing/);
  assert.match(result.stderr, /Reinstall without `--omit=optional`/);
  assert.match(result.stderr, /npm install -g chutes-build-/);
});
