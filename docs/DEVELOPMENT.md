# 开发环境与最佳实践（sony-camera-portal）

本项目是**纯 Go 单二进制**程序。要做开发 / 调试，机器上必须有 Go 工具链。
本文覆盖：安装、首次初始化、日常开发循环、代码质量、跨平台构建、调试、测试约定。

> 平台基线：本机为 macOS Apple Silicon（arm64），已安装 Homebrew。下面以此为主，
> 同时给出 Windows / Linux / Termux 的对应命令。

---

## 1. 安装 Go 工具链

需要 **Go 1.25+（最新稳定版）**。三选一：

### A. Homebrew（macOS 推荐，本机已具备 brew）
```bash
brew install go
go version          # 期望输出 go1.25.x 或更高
```

### B. 官方安装包 / 压缩包（任意平台，版本最可控）
从 https://go.dev/dl/ 下载对应平台包：
- macOS arm64：`go1.25.x.darwin-arm64.pkg`，双击安装。
- Linux：
  ```bash
  curl -LO https://go.dev/dl/go1.25.x.linux-amd64.tar.gz
  sudo rm -rf /usr/local/go && sudo tar -C /usr/local -xzf go1.25.x.linux-amd64.tar.gz
  echo 'export PATH=$PATH:/usr/local/go/bin' >> ~/.zshrc && source ~/.zshrc
  ```
- Windows：下载 `.msi` 安装即可。

### C. Termux（Android，v1 的 Android 开发/运行方式）
```bash
pkg install golang
```

### 验证
```bash
go version
go env GOROOT GOPATH GOMODCACHE
```
- `GOROOT`：Go 安装目录（brew 装的在 `/opt/homebrew/opt/go/libexec`）。
- `GOPATH`：默认 `~/go`，第三方工具会装进 `~/go/bin`——**把它加入 PATH**：
  ```bash
  echo 'export PATH=$PATH:$(go env GOPATH)/bin' >> ~/.zshrc && source ~/.zshrc
  ```

### 版本管理（可选，类比 nvm）

如果以后要在多个 Go 版本间切换，可用版本管理器；如果只搞本项目，**可跳过本节**。

| 工具 | 类比 | 说明 |
|---|---|---|
| **`g`**（voidint/g） | **最像 nvm** ✅ | `g install 1.25.4` / `g use 1.24.5` / `g ls`，轻量跨平台，推荐。 |
| **goenv** | pyenv/rbenv | 支持按目录 `.go-version` 自动切版本，适合多项目不同版本。 |
| **gvm** | 老牌 | moovweb/gvm，bash 实现，功能全但维护较慢。 |
| **asdf**（go 插件） | 统一多语言版本 | 已用 asdf 管 node/python 的话最省心。 |
| **官方多版本** | 无需第三方 | `go install golang.org/dl/go1.24.5@latest && go1.24.5 download`，需先有基础 go。 |

安装 `g`（macOS Apple Silicon，brew 已装）：
```bash
brew install g-go-version-manager
g install latest
g use 1.25.4
go version
```

> **重要：Go 1.21+ 内置 toolchain 自动管理，多数情况下不需要版本管理器。**
> Go 会按项目 `go.mod` 的 `go` 指令自动下载并使用对应工具链——例如 `go.mod`
> 写 `go 1.25.0`，本机即使是 1.24，跑 `go build` 时也会自动拉取 1.25。由
> `GOTOOLCHAIN` 控制（默认 `auto`）：
> ```bash
> go env GOTOOLCHAIN     # auto = 按 go.mod 自动切换
> ```
> 因此对**单仓库、单一目标版本**的本项目，推荐路径就是：`brew install go` 装一个
> 新版做基线，剩下交给 `go.mod` + `GOTOOLCHAIN=auto` 自动管理，无需额外工具。

---

## 2. 首次初始化项目

仓库目前只有文档，还没有 `go.mod`。第一次写代码前执行一次：

```bash
go mod init github.com/iyinchao/sony-camera-portal
```

> module path 已定为 `github.com/iyinchao/sony-camera-portal`（与 `CLAUDE.md` 的占位符对应）。

之后每次新增/改动依赖后整理依赖：
```bash
go mod tidy
```

本项目奉行 **标准库优先、避免重依赖**——`go.mod` 的 `require` 应保持尽量短。

---

## 3. 编辑器 / IDE

推荐 **VS Code + Go 扩展**（`golang.go`）。首次打开 `.go` 文件时，扩展会提示安装
`gopls`（官方语言服务器）、`dlv`（调试器）等，点允许即可。或手动：
```bash
go install golang.org/x/tools/gopls@latest
go install github.com/go-delve/delve/cmd/dlv@latest
```
JetBrains 用户可用 GoLand（开箱即用）。

建议在 VS Code 设置开启「保存时 `goimports` + 组织 import」，与下文质量约定一致。

---

## 4. 日常开发循环

```bash
# 本地运行（开发态），默认端口见 main.go 的 flag
go run . --port 8080

# 编译本地静态二进制（产物即可分发）
CGO_ENABLED=0 go build -o sony-camera-portal .

# 运行测试（首选；camera/ 的 XML 解析可离线测试，不需要真机）
go test ./...

# 带覆盖率
go test -cover ./...

# 只测单个包
go test ./camera/
```

**调试相机相关代码时**：M1 的 `camera/` 解析逻辑用「捕获的 `DmsDescPush.xml` /
`Browse` 响应」做表驱动单测，**无需连真机**即可迭代。只有端到端验证才需要：
连上相机 Wi-Fi AP（SSID `DIRECT-xxxx:ILCE-6000`）→ 相机固定为 `192.168.122.1`。

---

## 5. 代码质量（提交前必过）

`CLAUDE.md` 硬性要求：`gofmt` 与 `go vet` 必须干净。

```bash
gofmt -l .          # 列出未格式化文件；应为空
gofmt -w .          # 自动格式化
go vet ./...        # 静态检查；应无输出
```

推荐再加（开发工具，不进 runtime 依赖，符合「标准库优先」原则）：
```bash
go install golang.org/x/tools/cmd/goimports@latest
goimports -w .      # 格式化 + 自动增删 import

# 可选：更强的聚合 linter
go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest
golangci-lint run
```

一行跑全部检查：
```bash
gofmt -l . && go vet ./... && go test ./...
```

---

## 6. 跨平台构建（发布目标）

Go 交叉编译只需设环境变量，无需目标机器。统一 `CGO_ENABLED=0` 产静态二进制：

```bash
# macOS
CGO_ENABLED=0 GOOS=darwin  GOARCH=arm64 go build -o dist/sony-camera-portal-darwin-arm64 .
CGO_ENABLED=0 GOOS=darwin  GOARCH=amd64 go build -o dist/sony-camera-portal-darwin-amd64 .
# Windows
CGO_ENABLED=0 GOOS=windows GOARCH=amd64 go build -o dist/sony-camera-portal-windows-amd64.exe .
CGO_ENABLED=0 GOOS=windows GOARCH=arm64 go build -o dist/sony-camera-portal-windows-arm64.exe .
# Linux
CGO_ENABLED=0 GOOS=linux   GOARCH=amd64 go build -o dist/sony-camera-portal-linux-amd64 .
CGO_ENABLED=0 GOOS=linux   GOARCH=arm64 go build -o dist/sony-camera-portal-linux-arm64 .
# Termux / Android
CGO_ENABLED=0 GOOS=android GOARCH=arm64 go build -o dist/sony-camera-portal-android-arm64 .
```

> 正式发布由 **GoReleaser**（`.goreleaser.yaml`）+ GitHub Actions 统一产出（M5），
> 上面的手动命令用于本地验证某个目标能否编译。

---

## 7. 调试

```bash
# 用 delve 调试主程序
dlv debug . -- --port 8080
# 调试某个测试
dlv test ./camera/
```
VS Code Go 扩展可直接打断点 F5 调试（底层就是 dlv）。

简单排查也可用标准库 `log` / `log/slog` 打日志；**runtime 不得有遥测或除相机外的网络调用**。

---

## 8. 测试约定

- **表驱动测试**（table-driven），文件命名 `*_test.go`，与被测代码同包。
- 解析类逻辑（UPnP `DmsDescPush.xml`、`Browse` SOAP 响应）应把真实抓包样本
  存为 `testdata/` 下的固定文件，断言解析结果——保证离线可测、可回归。
- 每个任务应映射到一个可验证产物：一个测试、一个构建目标，或一条 `/api` 路由。

目录示意：
```
camera/
  client.go
  client_test.go
  testdata/
    DmsDescPush.xml
    browse_response.xml
```

---

## 9. 快速自检清单（开始写代码前）

```bash
go version                              # ① Go 已安装
go env GOPATH                           # ② GOPATH/bin 已入 PATH
ls go.mod 2>/dev/null || \
  go mod init github.com/iyinchao/sony-camera-portal   # ③ 模块已初始化
gofmt -l . && go vet ./... && go test ./...        # ④ 质量门禁
```
