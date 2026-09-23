import { chmod, copyFile, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const artifactsDirectory = process.argv[2] ?? "artifacts";
const packagesDirectory = process.argv[3] ?? "npm";
const targets = [
  ["x86_64-unknown-linux-gnu", "linux-x64-gnu", ""],
  ["aarch64-unknown-linux-gnu", "linux-arm64-gnu", ""],
  ["x86_64-apple-darwin", "darwin-x64", ""],
  ["aarch64-apple-darwin", "darwin-arm64", ""],
  ["x86_64-pc-windows-msvc", "win32-x64-msvc", ".exe"],
  ["aarch64-pc-windows-msvc", "win32-arm64-msvc", ".exe"],
];

for (const [rustTarget, packageTarget, extension] of targets) {
  const filename = `kokorobox-traffic-presenter${extension}`;
  const source = join(
    artifactsDirectory,
    `kokorobox-traffic-presenter-${rustTarget}${extension}`,
  );
  const packageDirectory = join(packagesDirectory, packageTarget);
  const destination = join(packageDirectory, filename);
  const manifestPath = join(packageDirectory, "package.json");

  await copyFile(source, destination);
  if (!extension) await chmod(destination, 0o755);

  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.files ??= [];
  if (!manifest.files.includes(filename)) manifest.files.push(filename);
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
}

for (const [rustTarget, packageTarget] of targets.filter(([target]) => target.includes("windows"))) {
  const filename = "kokorobox-portable-updater.exe";
  const source = join(artifactsDirectory, `kokorobox-portable-updater-${rustTarget}.exe`);
  const packageDirectory = join(packagesDirectory, packageTarget);
  await copyFile(source, join(packageDirectory, filename));
  const manifestPath = join(packageDirectory, "package.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.files ??= [];
  if (!manifest.files.includes(filename)) manifest.files.push(filename);
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
}
