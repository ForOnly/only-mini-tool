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
mise run build       # 构建生产包（NSIS / MSI）
mise run gen-types   # 从 Rust domain 导出 TypeScript 类型
```

也可直接使用 npm / cargo：

```bash
npm run tauri dev
npm run build
npm run gen:types
cargo check --manifest-path src-tauri/Cargo.toml
```

## 目录结构（摘要）

```
src/                 # Vue 前端（api / composables / components / tools）
src-tauri/src/       # Rust 后端（commands → services → repository）
doc/                 # 权威产品与工程文档
mise.toml            # 工具链与任务
```

改 Rust DTO 后请执行 `mise run gen-types`；**禁止手改** `src/api/generated/**`。
