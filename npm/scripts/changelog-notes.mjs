#!/usr/bin/env node

// Slice one version's section out of CHANGELOG.md, for use as GitHub release
// notes.
//
// Releasing was step 8 of a manual checklist and was skipped for v0.4.1, v0.4.2
// and v0.4.3 — three tags pushed, no releases, and a README that sends people to
// an empty Releases page for binaries. The workflow now creates the release
// itself, and it needs the notes from somewhere; CHANGELOG.md already has them,
// so it is the source rather than a second thing to keep in sync.
//
// Usage:
//   node npm/scripts/changelog-notes.mjs [version] [--changelog path] [--out path]
//
// `version` defaults to package.json's. Writes to stdout unless `--out` is given.
// Exits non-zero when the section is missing or empty, so a release cannot be
// published with blank notes.

'use strict';

import { readFileSync, writeFileSync } from 'node:fs';
import { argv, exit, stderr, stdout } from 'node:process';

/** Heading that opens a version's section: `## [1.0.0] - 2026-08-07`. */
const VERSION_HEADING = /^##\s+\[([^\]]+)\]/;

/**
 * Extract the body of `version`'s section: everything after its heading, up to
 * the next `## [` heading or end of file.
 *
 * @param {string} changelog Full CHANGELOG.md contents.
 * @param {string} version Version to slice, without a leading `v`.
 * @returns {string} The section body, trimmed.
 */
function extractNotes(changelog, version) {
  const lines = changelog.split(/\r?\n/);
  let start = -1;
  for (let i = 0; i < lines.length; i += 1) {
    const match = VERSION_HEADING.exec(lines[i]);
    if (match && match[1] === version) {
      start = i + 1;
      break;
    }
  }
  if (start === -1) {
    throw new Error(
      `CHANGELOG.md has no "## [${version}]" section. Add one before releasing.`
    );
  }
  let end = lines.length;
  for (let i = start; i < lines.length; i += 1) {
    if (VERSION_HEADING.test(lines[i])) {
      end = i;
      break;
    }
  }
  const body = lines.slice(start, end).join('\n').trim();
  if (!body) {
    throw new Error(
      `The "## [${version}]" section in CHANGELOG.md is empty. Release notes ` +
        `must say what changed.`
    );
  }
  return body;
}

function parseArgs(args) {
  const options = { version: null, changelog: 'CHANGELOG.md', out: null };
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--changelog' || arg === '--out') {
      const value = args[i + 1];
      if (!value) {
        throw new Error(`${arg} needs a path.`);
      }
      options[arg === '--out' ? 'out' : 'changelog'] = value;
      i += 1;
    } else if (arg.startsWith('--')) {
      throw new Error(`Unknown option ${arg}.`);
    } else if (options.version === null) {
      options.version = arg.replace(/^v/, '');
    } else {
      throw new Error(`Unexpected argument ${arg}.`);
    }
  }
  return options;
}

function main() {
  const options = parseArgs(argv.slice(2));
  const version =
    options.version ??
    JSON.parse(readFileSync('package.json', 'utf8')).version;
  const notes = extractNotes(readFileSync(options.changelog, 'utf8'), version);
  const document = `${notes}\n`;
  if (options.out) {
    writeFileSync(options.out, document);
    stderr.write(`Wrote ${options.out} (${notes.length} chars) for ${version}.\n`);
  } else {
    stdout.write(document);
  }
}

// No "only when invoked directly" guard: the tests drive this as a subprocess,
// matching `launcher.test.cjs`, so the module seam would be dead weight — and
// `import.meta.filename === process.argv[1]` is not a reliable comparison on
// Windows, where the two can differ in separators.
try {
  main();
} catch (error) {
  stderr.write(`changelog-notes: ${error.message}\n`);
  exit(1);
}
