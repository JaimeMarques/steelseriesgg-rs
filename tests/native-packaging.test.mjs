import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile } from 'node:fs/promises';

const repo = new URL('../', import.meta.url);
const source = async (path) => readFile(new URL(path, repo), 'utf8');

test('Ubuntu build stages PipeWire headers and checks native ELF dependency', async () => {
  const workflow = await source('.github/workflows/build-linux.yml');
  assert.match(workflow, /libpipewire-0\.3-dev/);
  assert.match(workflow, /libspa-0\.2-dev/);
  assert.match(workflow, /readelf -d .*ssgg-desktop/);
  assert.match(workflow, /libpipewire-0\.3\.so\.0/);
  assert.match(workflow, /python3 tests\/desktop_pipewire_isolated\.py/);
});

test('Debian control explicitly carries native PipeWire runtime dependency', async () => {
  const packaging = await source('desktop/scripts/package-linux.mjs');
  assert.match(packaging, /"libpipewire-0\.3-0t64"/);
});

test('strict Snap stages native library and grants PipeWire only to GUI', async () => {
  const snap = await source('snap/snapcraft.yaml');
  assert.match(snap, /- libpipewire-0\.3-0t64/);
  assert.match(snap, /plugs: \[audio-playback, pipewire, hidraw, electron-sandbox\]/);
  assert.doesNotMatch(snap, /plugs: \[audio-playback, pipewire, hidraw\]/);
  assert.match(snap, /confinement: strict/);
  assert.match(snap, /allow-sandbox: true/);
});
