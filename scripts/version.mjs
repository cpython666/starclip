import { readFile, writeFile, rename, rm } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const versionPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const paths = ['package.json', 'package-lock.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json'];

export function stableVersion(value) {
  if (typeof value !== 'string' || !versionPattern.test(value)) throw new Error('版本必须是稳定的 x.y.z，不能带前导零、预发布或构建后缀。');
  return value;
}
function packageVersion(text, heading) {
  const block = text.match(heading)?.[0];
  const value = block?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!value) throw new Error('找不到 Cargo 中的 starclip 版本。');
  return value;
}
const cargoPackage = /^\[package\][\s\S]*?(?=^\[|$(?![\s\S]))/m;
const cargoLockPackage = /^\[\[package\]\]\r?\nname = "starclip"\r?\n[\s\S]*?(?=^\[\[package\]\]|$(?![\s\S]))/m;
async function sources(root) {
  return Object.fromEntries(await Promise.all(paths.map(async path => [path, await readFile(resolve(root, path), 'utf8')])));
}
function versions(source) {
  const npm = JSON.parse(source['package.json']);
  const lock = JSON.parse(source['package-lock.json']);
  return {
    'package.json': npm.version,
    'package-lock.json': lock.version,
    'package-lock.json packages[""]': lock.packages?.['']?.version,
    'Cargo.toml': packageVersion(source['src-tauri/Cargo.toml'], cargoPackage),
    'Cargo.lock': packageVersion(source['src-tauri/Cargo.lock'], cargoLockPackage),
    'tauri.conf.json': JSON.parse(source['src-tauri/tauri.conf.json']).version,
  };
}
function assertVersions(source, tag) {
  const all = versions(source), version = stableVersion(all['package.json']);
  for (const [name, value] of Object.entries(all)) if (value !== version) throw new Error(`${name} 版本 ${value} 与 ${version} 不一致。`);
  if (tag !== undefined && tag !== `v${version}`) throw new Error(`发布 tag 必须是 v${version}，收到 ${tag}。`);
  return version;
}
export async function checkVersion(root = projectRoot, tag) {
  return assertVersions(await sources(root), tag);
}
export async function bumpVersion(next, root = projectRoot) {
  stableVersion(next);
  const source = await sources(root), current = assertVersions(source);
  const a = next.split('.').map(BigInt), b = current.split('.').map(BigInt);
  const changed = a.findIndex((part, i) => part !== b[i]);
  if (changed < 0 || a[changed] < b[changed]) throw new Error(`新版本必须高于 ${current}。`);
  const output = { ...source };
  for (const path of ['package.json', 'package-lock.json', 'src-tauri/tauri.conf.json']) {
    const json = JSON.parse(source[path]); json.version = next;
    if (path === 'package-lock.json') json.packages[''].version = next;
    output[path] = JSON.stringify(json, null, 2) + '\n';
  }
  for (const [path, pattern] of [['src-tauri/Cargo.toml', cargoPackage], ['src-tauri/Cargo.lock', cargoLockPackage]]) {
    output[path] = source[path].replace(pattern, block => block.replace(/^version\s*=\s*"[^"]+"/m, `version = "${next}"`));
  }
  assertVersions(output, `v${next}`);
  const suffix = `.version-${process.pid}.tmp`;
  try {
    // Stage every validated file before replacing any version file.
    for (const path of paths) await writeFile(resolve(root, path + suffix), output[path], { flag: 'wx' });
    for (const path of paths) await rename(resolve(root, path + suffix), resolve(root, path));
  } catch (error) {
    for (const path of paths) await writeFile(resolve(root, path), source[path]);
    throw error;
  } finally {
    for (const path of paths) await rm(resolve(root, path + suffix), { force: true });
  }
  return next;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [command, argument, ...extra] = process.argv.slice(2);
    if (extra.length || !['check', 'bump'].includes(command) || (command === 'bump' && !argument)) throw new Error('用法：node scripts/version.mjs check [vX.Y.Z] | bump X.Y.Z');
    console.log(command === 'check' ? await checkVersion(projectRoot, argument) : await bumpVersion(argument));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
