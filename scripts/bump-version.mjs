#!/usr/bin/env node
/**
 * 同步 package.json / package-lock.json / tauri.conf.json / Cargo.toml 的版本号。
 *
 * 用法:
 *   node scripts/bump-version.mjs patch|minor|major
 *   node scripts/bump-version.mjs 1.2.3
 *   node scripts/bump-version.mjs patch --tag
 *   node scripts/bump-version.mjs --check
 */
import { execFileSync, execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const paths = {
  pkg: join(root, "package.json"),
  lock: join(root, "package-lock.json"),
  tauri: join(root, "src-tauri", "tauri.conf.json"),
  cargo: join(root, "src-tauri", "Cargo.toml"),
};

const SEMVER_RE = /^(\d+)\.(\d+)\.(\d+)$/;

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function writeJson(path, data) {
  writeFileSync(path, `${JSON.stringify(data, null, 2)}\n`, "utf8");
}

function readVersions() {
  const pkg = readJson(paths.pkg);
  const lock = readJson(paths.lock);
  const tauri = readJson(paths.tauri);
  const cargoText = readFileSync(paths.cargo, "utf8");
  const cargoMatch = cargoText.match(/^\[package\][^\[]*?^version\s*=\s*"([^"]+)"/ms);
  if (!cargoMatch) {
    throw new Error("无法在 src-tauri/Cargo.toml 的 [package] 段解析 version");
  }
  return {
    pkg: pkg.version,
    lockRoot: lock.version,
    lockPkg: lock.packages?.[""]?.version,
    tauri: tauri.version,
    cargo: cargoMatch[1],
    cargoText,
  };
}

function assertConsistent(versions = readVersions()) {
  const set = new Set([
    versions.pkg,
    versions.lockRoot,
    versions.lockPkg,
    versions.tauri,
    versions.cargo,
  ]);
  if (set.size !== 1 || [...set][0] == null) {
    console.error("版本不一致：");
    console.error(`  package.json:              ${versions.pkg}`);
    console.error(`  package-lock.json (root):  ${versions.lockRoot}`);
    console.error(`  package-lock.json (""):    ${versions.lockPkg}`);
    console.error(`  tauri.conf.json:           ${versions.tauri}`);
    console.error(`  Cargo.toml:                ${versions.cargo}`);
    process.exit(1);
  }
  return versions.pkg;
}

function bumpSemver(current, level) {
  const m = current.match(SEMVER_RE);
  if (!m) {
    throw new Error(`当前版本不是 x.y.z：${current}`);
  }
  let major = Number(m[1]);
  let minor = Number(m[2]);
  let patch = Number(m[3]);
  if (level === "major") {
    major += 1;
    minor = 0;
    patch = 0;
  } else if (level === "minor") {
    minor += 1;
    patch = 0;
  } else if (level === "patch") {
    patch += 1;
  } else {
    throw new Error(`未知 bump 级别：${level}`);
  }
  return `${major}.${minor}.${patch}`;
}

function normalizeVersion(input) {
  const raw = input.startsWith("v") ? input.slice(1) : input;
  if (!SEMVER_RE.test(raw)) {
    throw new Error(`版本须为 x.y.z（可选 v 前缀）：${input}`);
  }
  return raw;
}

function writeVersions(next) {
  const pkg = readJson(paths.pkg);
  pkg.version = next;
  writeJson(paths.pkg, pkg);

  const lock = readJson(paths.lock);
  lock.version = next;
  if (lock.packages?.[""]) {
    lock.packages[""].version = next;
  }
  writeJson(paths.lock, lock);

  const tauri = readJson(paths.tauri);
  tauri.version = next;
  writeJson(paths.tauri, tauri);

  let cargoText = readFileSync(paths.cargo, "utf8");
  // 仅替换 [package] 段内第一个 version =
  cargoText = cargoText.replace(
    /^(\[package\][^\[]*?^version\s*=\s*")([^"]+)(")/ms,
    `$1${next}$3`,
  );
  writeFileSync(paths.cargo, cargoText, "utf8");
}

function gitPorcelain() {
  return execSync("git status --porcelain", { cwd: root, encoding: "utf8" }).trim();
}

function createReleaseCommitAndTag(version) {
  const dirty = gitPorcelain();
  if (dirty) {
    // 允许仅包含我们即将提交的四个文件；若还有其它脏文件则拒绝
    const lines = dirty.split(/\r?\n/).filter(Boolean);
    const allowed = new Set([
      "package.json",
      "package-lock.json",
      "src-tauri/tauri.conf.json",
      "src-tauri/Cargo.toml",
    ]);
    const extra = lines.filter((line) => {
      const path = line.slice(3).replace(/\\/g, "/");
      // rename: "R  old -> new"
      const normalized = path.includes(" -> ") ? path.split(" -> ").pop() : path;
      return !allowed.has(normalized);
    });
    if (extra.length > 0) {
      console.error("工作区有未提交改动，无法使用 --tag：");
      console.error(extra.join("\n"));
      process.exit(1);
    }
  }

  const files = [
    "package.json",
    "package-lock.json",
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
  ];
  execFileSync("git", ["add", ...files], { cwd: root, stdio: "inherit" });
  const staged = execSync("git diff --cached --name-only", {
    cwd: root,
    encoding: "utf8",
  }).trim();
  if (staged) {
    const msg = `chore: release v${version}`;
    execFileSync("git", ["commit", "-m", msg], { cwd: root, stdio: "inherit" });
  } else {
    console.log("无版本文件变更，跳过 commit");
  }

  let tagExists = false;
  try {
    execSync(`git rev-parse -q --verify refs/tags/v${version}`, {
      cwd: root,
      stdio: "ignore",
    });
    tagExists = true;
  } catch {
    tagExists = false;
  }
  if (tagExists) {
    console.error(`tag v${version} 已存在`);
    process.exit(1);
  }
  execFileSync("git", ["tag", "-a", `v${version}`, "-m", `v${version}`], {
    cwd: root,
    stdio: "inherit",
  });
  console.log(`\n已创建 tag v${version}。推送：`);
  console.log(`  git push && git push origin v${version}`);
}

function printUsage() {
  console.log(`用法:
  npm run bump -- patch|minor|major [--tag]
  npm run bump -- 1.2.3 [--tag]
  npm run bump -- --check`);
}

function main() {
  const args = process.argv.slice(2).filter((a) => a !== "--");
  const check = args.includes("--check");
  const tag = args.includes("--tag");
  const positional = args.filter((a) => a !== "--check" && a !== "--tag");

  if (check) {
    if (positional.length > 0) {
      printUsage();
      process.exit(1);
    }
    const v = assertConsistent();
    console.log(`版本一致：${v}`);
    return;
  }

  if (positional.length !== 1) {
    printUsage();
    process.exit(1);
  }

  const current = assertConsistent();
  const spec = positional[0];
  const next =
    spec === "patch" || spec === "minor" || spec === "major"
      ? bumpSemver(current, spec)
      : normalizeVersion(spec);

  if (next === current && !tag) {
    console.error(`版本未变化：${current}`);
    process.exit(1);
  }

  if (next !== current) {
    writeVersions(next);
    assertConsistent();
    console.log(`${current} → ${next}`);
  } else {
    console.log(`版本已是 ${next}，仅执行 --tag`);
  }

  if (tag) {
    createReleaseCommitAndTag(next);
  } else {
    console.log("已写入四处 version。需要打 tag 时：");
    console.log(`  npm run bump -- ${next} --tag`);
  }
}

main();
