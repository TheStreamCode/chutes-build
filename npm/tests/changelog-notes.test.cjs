'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const script = path.resolve(
  __dirname,
  '..',
  'scripts',
  'changelog-notes.mjs'
);
const repoRoot = path.resolve(__dirname, '..', '..');

// Black-box, like `launcher.test.cjs`: drive the script as a subprocess so the
// test covers the CLI contract the release workflow actually depends on, not an
// internal function it could drift from.
function run(args, { cwd = repoRoot } = {}) {
  return spawnSync(process.execPath, [script, ...args], {
    cwd,
    encoding: 'utf8',
  });
}

function withChangelog(body, fn) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'changelog-notes-'));
  const file = path.join(dir, 'CHANGELOG.md');
  fs.writeFileSync(file, body);
  try {
    return fn(file);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

const SAMPLE = [
  '# Changelog',
  '',
  '## [Unreleased]',
  '',
  '## [1.2.0] - 2026-01-02',
  '',
  '### Added',
  '',
  '- A thing.',
  '',
  '## [1.1.0] - 2026-01-01',
  '',
  '- An older thing.',
  '',
].join('\n');

test('extracts one version and stops at the next heading', () => {
  withChangelog(SAMPLE, (file) => {
    const result = run(['1.2.0', '--changelog', file]);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /### Added/);
    assert.match(result.stdout, /- A thing\./);
    assert.doesNotMatch(
      result.stdout,
      /older thing/,
      'must not bleed into the previous version'
    );
  });
});

test('a leading v is accepted, since tags carry one', () => {
  withChangelog(SAMPLE, (file) => {
    const result = run(['v1.2.0', '--changelog', file]);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /- A thing\./);
  });
});

test('a missing section fails rather than releasing blank notes', () => {
  withChangelog(SAMPLE, (file) => {
    const result = run(['9.9.9', '--changelog', file]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /no "## \[9\.9\.9\]" section/);
  });
});

// `## [Unreleased]` with nothing under it is the normal state between releases.
// Tagging while the section is still empty would publish a release whose notes
// say nothing, which is the failure this guard exists to prevent.
test('an empty section fails rather than releasing blank notes', () => {
  withChangelog(SAMPLE, (file) => {
    const result = run(['Unreleased', '--changelog', file]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /is empty/);
  });
});

test('--out writes the notes to a file', () => {
  withChangelog(SAMPLE, (file) => {
    const out = path.join(path.dirname(file), 'notes.md');
    const result = run(['1.2.0', '--changelog', file, '--out', out]);
    assert.equal(result.status, 0, result.stderr);
    assert.match(fs.readFileSync(out, 'utf8'), /- A thing\./);
    assert.equal(result.stdout, '', 'notes go to the file, not stdout');
  });
});

// The workflow calls this with no version so the release always matches the
// version it just published.
test('the version defaults to package.json', () => {
  const expected = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8')
  ).version;
  const result = run([]);
  assert.equal(result.status, 0, result.stderr);
  assert.ok(
    result.stdout.trim().length > 0,
    `CHANGELOG.md must have a non-empty section for ${expected}`
  );
});
