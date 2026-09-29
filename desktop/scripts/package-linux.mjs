import { chmod, cp, mkdir, readFile, writeFile, rm, rename, readdir, lstat, open, mkdtemp } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { run, inspectElf, compareVersions, validateTree, portableCargoEnv } from "./package-lib.mjs";

const desktop = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repo = path.dirname(desktop),
  release = path.join(desktop, "release");
const require = createRequire(import.meta.url);
const manifest = JSON.parse(await readFile(path.join(desktop, "package.json"), "utf8"));
const lock = JSON.parse(await readFile(path.join(desktop, "package-lock.json"), "utf8"));
const electronPackage = require("electron/package.json");
const electronVersion = lock.packages["node_modules/electron"].version;
if (electronPackage.version !== electronVersion) throw new Error("Run npm ci: Electron must match package-lock.json");
if (process.platform !== "linux" || process.arch !== "x64") throw new Error("Build this amd64 package on Linux x64");
const args = process.argv.slice(2);
let sidecar,
  maxGlibc = "2.39",
  buildDeb = false;
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--sidecar") sidecar = args[++i];
  else if (args[i] === "--max-glibc") maxGlibc = args[++i];
  else if (args[i] === "--deb") buildDeb = true;
  else throw new Error(`Unknown argument: ${args[i]}`);
}
if (!/^\d+(\.\d+)+$/.test(maxGlibc || "")) throw new Error("--max-glibc requires a version");
if (sidecar && !path.isAbsolute(sidecar)) throw new Error("--sidecar requires an absolute path");
if (!/^\d+\.\d+\.\d+(?:[-+.][a-zA-Z0-9.]+)?$/.test(manifest.version)) throw new Error("Invalid package version");
process.umask(0o022);
const branding = path.join(desktop, "assets/branding");
for (const name of ["ssgg.png", "ssgg.svg", "ssgg-symbolic.svg", "ssgg-tray-light.png", "ssgg-tray-dark.png"])
  if (!(await lstat(path.join(branding, name))).isFile()) throw new Error(`Missing branding: ${name}`);
run("npm", ["run", "build"], { cwd: desktop, stdio: "inherit" });
if (!sidecar) {
  run("cargo", ["build", "--release", "--locked", "--bin", "ssgg-desktop"], {
    cwd: repo,
    stdio: "inherit",
    env: portableCargoEnv(),
  });
  sidecar = path.join(repo, "target/release/ssgg-desktop");
}
if (!(await lstat(sidecar)).isFile()) throw new Error("Sidecar must be a regular file, not a symlink");
const sidecarAbi = await inspectElf(sidecar);
if (sidecarAbi.architecture !== "amd64") throw new Error("Sidecar architecture must be amd64");
await mkdir(release, { recursive: true });
const temp = await mkdtemp(path.join(release, ".app-stage-"));
const appDir = path.join(release, "app");
try {
  // Never copy node_modules/electron/dist: developers may have a host-only sandbox
  // helper symlink there. Re-extract the official, checksum-pinned archive instead.
  const { downloadArtifact } = require("@electron/get");
  const checksums = require("electron/checksums.json");
  const archiveName = `electron-v${electronVersion}-linux-x64.zip`;
  const archive = await downloadArtifact({
    version: electronVersion,
    artifactName: "electron",
    platform: "linux",
    arch: "x64",
    checksums,
  });
  const digest = createHash("sha256")
    .update(await readFile(archive))
    .digest("hex");
  if (digest !== checksums[archiveName]) throw new Error("Electron archive checksum mismatch");
  run("unzip", ["-q", archive, "-d", temp]);
  await validateTree(temp);
  await rename(path.join(temp, "electron"), path.join(temp, "ssgg-gui"));
  await rm(path.join(temp, "resources/default_app.asar"), { force: true });
  await rm(path.join(temp, "electron.d.ts"), { force: true });
  const app = path.join(temp, "resources/app");
  await mkdir(app, { recursive: true });
  await writeFile(
    path.join(app, "package.json"),
    JSON.stringify(
      {
        name: manifest.name,
        version: manifest.version,
        productName: "SSGG",
        desktopName: "io.github.MrTheSoulz.SSGG.desktop",
        main: "dist-main/main.cjs",
      },
      null,
      2,
    ) + "\n",
  );
  for (const dir of ["dist", "dist-main"]) await cp(path.join(desktop, dir), path.join(app, dir), { recursive: true });
  await cp(sidecar, path.join(temp, "resources/ssgg-desktop"));
  await chmod(path.join(temp, "resources/ssgg-desktop"), 0o755);
  await cp(branding, path.join(temp, "resources/branding"), { recursive: true });
  await cp(path.join(repo, "LICENSE"), path.join(temp, "LICENSE.ssgg"));
  const binaries = {};
  async function normalizeAndInspect(directory) {
    await chmod(directory, 0o755);
    for (const name of await readdir(directory)) {
      const file = path.join(directory, name),
        info = await lstat(file);
      if (info.isSymbolicLink()) throw new Error(`Symbolic link in app: ${file}`);
      if (info.isDirectory()) {
        await normalizeAndInspect(file);
        continue;
      }
      if (!info.isFile()) throw new Error(`Special file in app: ${file}`);
      await chmod(file, info.mode & 0o111 ? 0o755 : 0o644);
      const handle = await open(file, "r"),
        magic = Buffer.alloc(4);
      try {
        await handle.read(magic, 0, 4, 0);
      } finally {
        await handle.close();
      }
      if (magic.equals(Buffer.from([127, 69, 76, 70]))) binaries[path.relative(temp, file)] = await inspectElf(file);
    }
  }
  await normalizeAndInspect(temp);
  const glibc = Object.values(binaries)
    .map((b) => b.glibc)
    .filter(Boolean)
    .sort(compareVersions)
    .at(-1);
  if (!glibc || compareVersions(glibc, maxGlibc) > 0)
    throw new Error(
      `GLIBC floor ${glibc} exceeds target ${maxGlibc}; rebuild Rust on Ubuntu 24.04, do not mislabel compatibility`,
    );
  const buildInfo = {
    version: manifest.version,
    electronVersion,
    electronArchiveSha256: digest,
    architecture: "amd64",
    glibc,
    binaries,
  };
  await writeFile(path.join(temp, "resources/build-info.json"), JSON.stringify(buildInfo, null, 2) + "\n");
  await validateTree(temp);
  await rm(appDir, { recursive: true, force: true });
  await rename(temp, appDir);
  console.log(`Standalone application: ${appDir}`);
  if (buildDeb) {
    const stage = path.join(release, "deb-stage");
    await rm(stage, { recursive: true, force: true });
    await mkdir(path.join(stage, "opt"), { recursive: true });
    await cp(appDir, path.join(stage, "opt/ssgg"), { recursive: true });
    await chmod(path.join(stage, "opt/ssgg/chrome-sandbox"), 0o4755);
    async function install(source, target, mode = 0o644) {
      const dest = path.join(stage, target);
      await mkdir(path.dirname(dest), { recursive: true });
      await cp(source, dest);
      await chmod(dest, mode);
    }
    const pkg = path.join(desktop, "packaging");
    await install(path.join(pkg, "ssgg-gui"), "usr/bin/ssgg-gui", 0o755);
    await install(path.join(pkg, "refresh-device-access"), "usr/lib/ssgg-gui/refresh-device-access", 0o755);
    await install(path.join(pkg, "70-ssgg.rules"), "usr/lib/udev/rules.d/70-ssgg.rules");
    await install(path.join(pkg, "io.github.MrTheSoulz.SSGG.apparmor"), "etc/apparmor.d/io.github.MrTheSoulz.SSGG");
    await install(
      path.join(pkg, "io.github.MrTheSoulz.SSGG.desktop"),
      "usr/share/applications/io.github.MrTheSoulz.SSGG.desktop",
    );
    for (const file of ["postinst", "postrm"]) await install(path.join(pkg, file), `DEBIAN/${file}`, 0o755);
    await install(
      path.join(branding, "ssgg.svg"),
      "usr/share/icons/hicolor/scalable/apps/io.github.MrTheSoulz.SSGG.svg",
    );
    await install(
      path.join(branding, "ssgg-symbolic.svg"),
      "usr/share/icons/hicolor/symbolic/apps/io.github.MrTheSoulz.SSGG-symbolic.svg",
    );
    for (const file of await readdir(path.join(branding, "sizes"))) {
      if (!/^\d+\.png$/.test(file)) continue;
      const size = file.slice(0, -4);
      await install(
        path.join(branding, "sizes", file),
        `usr/share/icons/hicolor/${size}x${size}/apps/io.github.MrTheSoulz.SSGG.png`,
      );
    }
    await writeFile(path.join(stage, "DEBIAN/conffiles"), "/etc/apparmor.d/io.github.MrTheSoulz.SSGG\n");
    const depends = [
      `libc6 (>= ${glibc})`,
      "libgcc-s1",
      "libstdc++6",
      "libasound2t64",
      "libatk1.0-0t64",
      "libatk-bridge2.0-0t64",
      "libatspi2.0-0t64",
      "libcairo2",
      "libcups2t64",
      "libdbus-1-3",
      "libdrm2",
      "libexpat1",
      "libgbm1",
      "libglib2.0-0t64",
      "libgtk-3-0t64",
      "libnspr4",
      "libnss3",
      "libpango-1.0-0",
      "libpipewire-0.3-0t64",
      "libudev1",
      "libx11-6",
      "libxcb1",
      "libxcomposite1",
      "libxdamage1",
      "libxext6",
      "libxfixes3",
      "libxkbcommon0",
      "libxrandr2",
      "pulseaudio-utils",
      "udev",
      "dbus-bin",
      "apparmor (>= 4.0)",
      "desktop-file-utils",
      "hicolor-icon-theme",
    ];
    const installedSize = run("du", ["-sk", path.join(stage, "opt"), path.join(stage, "usr")])
      .trim()
      .split("\n")
      .reduce((sum, line) => sum + Number(line.split(/\s/)[0]), 0);
    await writeFile(
      path.join(stage, "DEBIAN/control"),
      `Package: ssgg-gui\nVersion: ${manifest.version}\nSection: sound\nPriority: optional\nArchitecture: amd64\nMaintainer: SSGG contributors <ssgg@users.noreply.github.com>\nInstalled-Size: ${installedSize}\nDepends: ${depends.join(", ")}\nRecommends: libnotify4\nSuggests: gnome-shell-extension-appindicator\nHomepage: https://github.com/MrTheSoulz/steelseriesgg-rs\nDescription: SSGG SteelSeries device and audio console\n Bundles the Electron desktop runtime and matching Rust service.\n Audio and HID controls are opt-in; no service or autostart is enabled.\n`,
    );
    run("desktop-file-validate", [path.join(stage, "usr/share/applications/io.github.MrTheSoulz.SSGG.desktop")]);
    run("apparmor_parser", [
      "--skip-kernel-load",
      "--skip-read-cache",
      path.join(stage, "etc/apparmor.d/io.github.MrTheSoulz.SSGG"),
    ]);
    const output = path.join(release, `ssgg-gui_${manifest.version}_amd64.deb`);
    run("dpkg-deb", ["--root-owner-group", "-Zxz", "--build", stage, output], { stdio: "inherit" });
    await writeFile(
      output + ".sha256",
      `${createHash("sha256")
        .update(await readFile(output))
        .digest("hex")}  ${path.basename(output)}\n`,
    );
    run(process.execPath, [path.join(desktop, "scripts/verify-deb.mjs"), output], { stdio: "inherit" });
    console.log(`Ubuntu package: ${output}`);
  }
} finally {
  await rm(temp, { recursive: true, force: true });
}
