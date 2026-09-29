// Run graphical tests on a private bus/display with no host service activation.
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { spawn } from "node:child_process";
import path from "node:path";
import os from "node:os";
const args = process.argv.slice(2);
if (!args.length) throw new Error("Pass a test command and its arguments");
const root = await mkdtemp(path.join(os.tmpdir(), "ssgg-test-session-"));
try {
  for (const dir of ["home", "config", "data", "cache", "runtime", "tmp"])
    await mkdir(path.join(root, dir), { mode: 0o700 });
  const config = path.join(root, "bus.conf");
  await writeFile(
    config,
    "<busconfig><type>session</type><listen>unix:tmpdir=" +
      path.join(root, "runtime") +
      '</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>\n',
  );
  const child = spawn("dbus-run-session", ["--config-file", config, "--", "xvfb-run", "-a", ...args], {
    stdio: "inherit",
    env: {
      ...process.env,
      HOME: path.join(root, "home"),
      XDG_CONFIG_HOME: path.join(root, "config"),
      XDG_DATA_HOME: path.join(root, "data"),
      XDG_CACHE_HOME: path.join(root, "cache"),
      XDG_RUNTIME_DIR: path.join(root, "runtime"),
      PIPEWIRE_RUNTIME_DIR: path.join(root, "runtime"),
      TMPDIR: path.join(root, "tmp"),
      PULSE_SERVER: `unix:${root}/no-audio.sock`,
      PIPEWIRE_REMOTE: "ssgg-no-audio",
      GSETTINGS_BACKEND: "memory",
      GTK_USE_PORTAL: "0",
      GIO_USE_VFS: "local",
      XDG_CURRENT_DESKTOP: "SSGGTest",
    },
  });
  process.exitCode = await new Promise((resolve, reject) => {
    child.once("exit", (code) => resolve(code ?? 1));
    child.once("error", reject);
  });
} finally {
  await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}
