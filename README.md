# only-mini-tool

基于 Tauri 2 + Rust + Vue 3 的 Windows 桌面工具箱。统一 Workbench 壳承载多个独立小工具；首期交付多引擎 OCR（默认百度智能云 handwriting）。

## 文档

权威文档在 [`doc/`](doc/README.md)：

| 文档 | 职责 |
|------|------|
| [产品设计文档](doc/产品设计文档.md) | WHAT — 定位、体验、验收 |
| [产品蓝图](doc/产品蓝图.md) | 架构硬约束 — 分层、壳、扩展契约 |
| [技术实现文档](doc/技术实现文档.md) | HOW — 模块、表、命令、实现细节 |
| [项目进度](doc/项目进度.md) | 里程碑与变更记录 |

## 技术栈

Tauri 2 · Rust / Tokio · Vue 3 + TypeScript + Vite · SQLite WAL · vue-i18n · mise

## 运行环境

本项目使用 [mise](https://mise.jdx.dev/) 管理工具链，版本见 [mise.toml](mise.toml)。

| 工具 | 版本 |
|------|------|
| Node.js | 24 |
| Rust | stable |

系统依赖（Windows）：Visual Studio Build Tools（C++）、WebView2

### 首次 setup

```bash
mise trust
mise install
mise run install
```

### 常用命令

```bash
mise run dev         # 启动 Tauri 开发模式
mise run check       # Rust 编译检查
mise run ci          # 本地对齐 CI（version + typecheck + cargo check + types drift）
mise run build       # 构建生产包（NSIS / MSI）
mise run gen-types   # 从 Rust domain 导出 TypeScript 类型
mise run bump -- --check # 检查version
mise run release -- patch --git-tag --push   # 一键发版（bump + tag + push）
```

也可直接使用 npm / cargo：

```bash
npm run tauri dev
npm run build
npm run gen:types
npm run check:version
npm run bump -- patch
npm run release:patch
cargo check --manifest-path src-tauri/Cargo.toml
```

CI：push/PR → `master` 跑 [`.github/workflows/ci.yml`](.github/workflows/ci.yml)；tag `v*` 跑 [release.yml](.github/workflows/release.yml) 产出 draft 安装包。

## 版本 bump 与发版

[`scripts/bump-version.mjs`](scripts/bump-version.mjs) 同步四处版本号：`package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`。

**一键发版（推荐）**：

```bash
npm run release:patch    # bump patch + commit + annotated tag + push 分支与 tag
npm run release:minor
npm run release:major
# 或
mise run release -- patch --git-tag --push
```

**分步**：

```bash
npm run check:version                         # 校验四处一致
npm run bump -- patch                         # 只改文件不 commit
node scripts/bump-version.mjs patch --git-tag # commit + tag（不 push）
node scripts/bump-version.mjs patch --git-tag --push  # tag 后 push
```

- `--git-tag`：提交 `chore: release vX.Y.Z`（仅四个版本文件可脏）并打 annotated tag  
- `--push`：必须与 `--git-tag` 同用；执行 `git push -u origin HEAD` 与 `git push origin vX.Y.Z`  
- **勿用** `npm run bump -- … --tag`：npm 会吞掉 `--tag`，导致打不上 tag  

推荐流程：`master` CI 绿 → `npm run release:patch` → Release workflow 产出 draft → 冒烟后 publish。细节见 [doc/技术实现文档.md](doc/技术实现文档.md) §7。

## 目录结构（摘要）

```
src/                 # Vue 前端（api / composables / components / tools）
src-tauri/src/       # Rust 后端（commands → services → repository）
doc/                 # 权威产品与工程文档
mise.toml            # 工具链与任务
```

改 Rust DTO 后请执行 `mise run gen-types`；**禁止手改** `src/api/generated/**`。
