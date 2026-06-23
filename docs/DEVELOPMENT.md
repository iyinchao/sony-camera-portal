# 开发环境与最佳实践（sony-camera-portal）

本项目是 **Rust 工作区（Cargo workspace）+ React 前端**，编译成单个自包含二进制。
本文覆盖：工具链安装、日常开发循环（含前端热更新）、代码质量、跨平台构建（含 iSH）、
测试约定。

> 平台基线：macOS Apple Silicon（arm64）+ Homebrew。下面以此为主，并给出其他平台命令。
> 已废弃的 Go 实现保留在 `docs/prototype/go/`（仅参考，不构建）。

---

## 1. 安装工具链

### Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustc --version && cargo --version
```
`rustup` 自带版本管理（类似 nvm）：`rustup update`、`rustup toolchain list`。

### Node（仅前端构建用，build-time）
```bash
brew install node    # 或 nvm；需要 Node 18+
node -v
```

### 交叉编译到 iSH（iOS）所需
```bash
rustup target add i686-unknown-linux-musl
brew install zig
cargo install cargo-zigbuild   # 用 zig 当 musl 链接器，免装额外工具链
```

---

## 2. 仓库结构（Cargo workspace）

```
Cargo.toml                 workspace（成员: packages/{camera,server,cli}）
packages/
  camera/   Rust lib — DLNA 客户端（discover / browse / http / model）
  server/   Rust lib — HTTP server + /api 代理 + 连接状态机（AppState）
  cli/      Rust bin — sony-camera-portal：flags、rust-embed 嵌入前端、启动
  web/      React + Vite + TS 前端（npm run build → packages/web/dist）
docs/prototype/            归档的 Go MVP + Rust PoC（参考）
```

---

## 3. 日常开发循环

### 后端 / 整体（无相机，mock 数据）
```bash
cargo run -- --mock 18              # 启动 + 自动开浏览器，看 mock 相册
cargo run -- --port 9000 --mock 18  # 换端口
cargo build --release               # 产物 target/release/sony-camera-portal
```
> 连接由 web UI 驱动：没有 `--camera-host`。要连真机就在网页连接面板里输 IP，或让它自动发现。

### ⭐ 前端热更新（改 React 即时看到效果）
一键起（后端 mock + Vite HMR，nohup 脱离会话）：
```bash
./scripts/dev.sh          # 默认 mock 24；./scripts/dev.sh 100 → 100 张
./scripts/dev-stop.sh     # 关闭两个服务
```
或手动两个终端：
```bash
# 终端 1 — Rust 后端只提供 mock 的 /api
cargo run -- --port 8080 --mock 24 --no-open

# 终端 2 — Vite 开发服务器（HMR）
cd packages/web && npm run dev      # 打开它给的 http://localhost:5173
```
改 `packages/web/src/*` → 浏览器毫秒级热更新。**看 :5173，不是 :8080**（后者是嵌入的已构建版）。
`vite.config.ts` 已把 `/api` 代理到 `:8080`，端口要对上。

---

## 4. 代码质量（提交前必过）

```bash
cargo fmt                       # 格式化
cargo fmt --check               # CI 用：未格式化则失败
cargo clippy --all-targets      # 静态检查，应无 warning
cargo test                      # 全 workspace 测试
```
一行跑全部：
```bash
cargo fmt --check && cargo clippy --all-targets && cargo test
```
前端：`cd packages/web && npm run build`（`tsc -b` 会做类型检查）。

约定：保持 **纯 Rust 依赖**（不引入 C/asm，如 `ring`），以便 `i686-musl` 干净交叉编译；
HTTP 客户端手写、不设 socket 选项（iSH 兼容，见 `packages/camera/src/http.rs`）。

---

## 5. 跨平台构建

先构建前端（被 `rust-embed` 嵌入；debug 下运行时从磁盘读，release 下编进二进制）：
```bash
cd packages/web && npm ci && npm run build && cd ../..
```
再交叉编译（cargo 交叉编译只需加 `--target`）：
```bash
# iOS / iSH —— 全静态 32 位 musl
cargo zigbuild --release --target i686-unknown-linux-musl
file target/i686-unknown-linux-musl/release/sony-camera-portal   # 应为 ELF 32-bit, statically linked

# 其他目标（同理；非 musl 用 cargo build 或 cargo zigbuild）
cargo zigbuild --release --target aarch64-unknown-linux-musl     # Linux arm64 / Termux
cargo build   --release --target x86_64-pc-windows-gnu           # Windows（需对应工具链）
```
> macOS/Windows 本地目标用 `cargo build --release`；Linux/Android/iSH 的静态 musl 用 `cargo zigbuild`。

---

## 6. 在 iSH（iOS）上运行

1. App Store 装 **iSH**。
2. 把上面交叉编译出的 `sony-camera-portal` 传进 iSH（AirDrop → 文件 app → iSH 目录；或同网段 `wget`）。
3. iSH 里：`chmod +x sony-camera-portal && ./sony-camera-portal`
4. Safari 打开 `http://localhost:8080`；在连接面板里输相机 IP（如 `10.0.0.1`）——
   iSH 上自动发现可能因 iOS 封多播而失败，手输 IP 最稳。
5. 保活：iPad 用分屏；iPhone 给 iSH 开「位置：使用期间」。

> 原理与坑详见项目记忆 `go-on-ish-ios`：iSH 拒绝 socket 超时选项（EINVAL）、封多播、无路由表。
> 我们的阻塞 HTTP 客户端 + getsockname 网关探测正是为绕开这些。

---

## 7. 测试约定

- 单测与被测代码同文件 `#[cfg(test)] mod tests`，或包内 `tests/`。
- 解析类逻辑（`DmsDesc.xml`、`Browse` 响应）用 `include_str!`/`include_bytes!` 把真实抓包
  样本（`packages/camera/testdata/`）编进测试，断言解析结果——离线可测、可回归。
- server 路由测试用纯函数 `handle(state, assets, method, path, body)` + stub/mock source，
  无需绑端口（见 `packages/server/src/lib.rs`）。
- 抓相机 fixture 的离线脚本：`scripts/grab.py`（在相机 Wi-Fi 上跑，结果写日志文件）。

---

## 8. 快速自检清单

```bash
rustc --version                                   # ① Rust 已装
ls packages/web/dist/index.html 2>/dev/null || (cd packages/web && npm run build)  # ② 前端已构建
cargo fmt --check && cargo clippy --all-targets && cargo test                       # ③ 质量门禁
cargo run -- --mock 12                            # ④ 跑起来看看
```
