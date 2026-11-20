# SonaSub

[English](README.md) | **简体中文**

开源的系统输出与麦克风实时字幕工具。SonaSub 通过 CPAL 采集音频，使用
sherpa-onnx 完成流式或分段语音识别与说话人关联，使用 Bergamot 完成本地
翻译，或通过可选的 OpenAI-Compatible 端点翻译，并在桌面界面实时显示字幕。
完整运行时为原生 Rust，安装包不包含 Node.js。

原文字幕在识别产生后立即显示；说话人标签和译文作为异步结果，独立更新到
同一条字幕上。项目不包含模拟结果、回退引擎、伪造的流式翻译或静默音源兜底。

## 功能

- 通过 CPAL / Core Audio 采集 macOS 系统输出回环与麦克风（系统输出要求
  macOS 14.6+）。
- 通过 CPAL / WASAPI 采集 Windows 系统输出回环与麦克风，不启用 ASIO。
- 按设备实际采样率、声道数和采样格式转换，跨音频块保持重采样状态，统一送入
  16 kHz 单声道识别链路。
- 流式 Zipformer 中英双语识别，输出增量文本与最终文本。
- Silero VAD + SenseVoice 分段识别，支持中、英、日、韩、粤语。
- Silero VAD + Qwen3-ASR 0.6B/1.7B INT8 分段识别，覆盖 30 种语言和
  22 种汉语方言，仅生成最终字幕。
- CAM++ 说话人嵌入，以及有界窗口的 Pyannote Diarization。
- 本地 Bergamot 翻译，支持 en -> zh-Hans 与 zh-Hans -> en。
- 可选 OpenAI-Compatible 翻译：自定义 Base URL、模型与 API Key，开放 30 个
  目标语言。只有选择该引擎时，字幕文本才会离开这台 Mac。
- Rust 字幕调度：稳定 ID、修订号，以及原文、说话人、译文独立 Patch。
- React 实时字幕工作台：模型与模式配置、模型就绪提示、会话控制、通知与
  运行指标。
- 模型齐全且选择 Bergamot 时，核心链路可断网运行；远端引擎需要能访问
  所配置的端点。

Chrome 扩展仍是独立的 Manifest V3 工程，目前只有 popup。标签页采集属于
后续工作，不会被当作隐藏的音频兜底来源。

## 技术栈

- 工程管理：pnpm workspace + Vite+（`vp` 管理格式、检查、类型检查、测试与构建）
- 桌面：Tauri 2 + Rust、React + TypeScript
- UI：Tailwind CSS 4 + CSS Variables、shadcn/ui、Iconify 静态图标
- 路由与状态：TanStack Router / Query / Form、Zustand、Day.js
- 原生语音：官方 `sherpa-onnx` Rust API v2.0.1
- 原生音频采集：`cpal 0.18.2`
- 本地翻译：`fxtranslate 0.5.0` + 固定版本 Bergamot 模型
- 远端翻译：`adk-model 2.3.0` OpenAI-Compatible 客户端
- 扩展：WXT + React + TypeScript（Manifest V3）

## 环境要求

- Node.js 24 LTS（`^24.11.0`）与 pnpm 12，仅用于开发和模型准备；打包后的
  桌面应用不依赖 Node.js。
- Rust 稳定版 >= 1.95，含 `rustfmt` 与 `clippy`。
- 系统输出回环需要 macOS 14.6 或更高版本；麦克风与其余桌面链路也支持
  Windows 8 或更高版本。
- macOS 需要 Xcode Command Line Tools；Windows 需要 Tauri 使用的 WebView2
  与 Visual Studio 构建环境。
- 模型准备需要 `curl`、`tar` 与 `zstd`（可用 `brew install zstd` 安装）。

CPAL 采集已实现 macOS 与 Windows 路径。macOS arm64 已做真实设备验证；
Windows WASAPI 路径由 CI 编译检查，但尚未在真实 Windows 音频设备上验证。
Linux 未实现。macOS 14.6 之前不支持系统输出回环，会明确报告为不支持。
系统默认设备切换会作为流警告上报；SonaSub 目前不会在每次设备变化后自动重建
采集流。明确选择的设备失效时不会回退到其他设备。

## 初始化

```bash
pnpm install
pnpm prepare:models
pnpm dev:desktop:app
```

首次采集系统输出时，macOS 会请求“屏幕与系统音频录制”权限；麦克风采集会
单独请求“麦克风”权限。macOS 上如果某个输出设备同时暴露输入通道（例如 USB
耳机），SonaSub 会为该输出创建 Core Audio Process Tap 和私有聚合设备，再
交给 CPAL 采集。选中的是该设备的输出混音，不会误开麦克风，也不会静默改用
其他音源。

官方 `sherpa-onnx` crate 会在 Rust 编译阶段下载并静态链接与 v2.0.1 匹配的
原生运行时。它不是独立服务，也不包含 Node.js 进程。

模型文件不进入安装包，默认目录为：

```text
~/Library/Application Support/dev.sonasub.desktop/models/
```

模型来源、版本、校验值与断点续传流程见 [docs/models.md](docs/models.md)。

在打包后的应用中，打开“模型管理”即可按模型组选择要安装的模型。每张卡片都显示大致下载量、
磁盘与计算资源需求，以及效果说明。点击“下载并安装”即可一键获取固定的官方安装包或模型
文件，也可以导入本地已有文件或解压后的模型文件夹；还可以单独选择下载目录。只有你主动
操作时才会开始下载。

### Qwen3 ASR 模型来源

`qwen3-asr-0.6b-int8` 来自 sherpa-onnx 官方 2026-03-25 INT8 Release。
`qwen3-asr-1.7b-int8` 使用 `zengshuishui` 在 ModelScope 发布的 ONNX 导出，
并固定到 revision `9c182309f7bb075f241424441add9e16c5086dfb`。ModelScope
模型卡声明 Apache-2.0，并链接 sherpa-onnx 兼容推理路径，同时明确说明该导出
仅作参考演示，不保证生产可用性。

两个参数规模共用 Rust `Qwen3Backend`。SonaSub 将 Silero VAD 分段交给离线
识别器，只生成最终字幕。固定的 sherpa-onnx 2.0.1 Qwen stream 没有语言
覆盖选项，因此 Qwen 模型使用自动语言检测，会话界面的语音语言选择会被
禁用；原生不提供临时 Partial 字幕。

## 常用命令

```bash
pnpm dev:desktop        # 桌面前端开发服务器 http://localhost:5173
pnpm dev:desktop:app    # Tauri 桌面应用（编译 Rust）
pnpm dev:extension      # WXT 扩展开发

pnpm prepare:models     # 下载并校验全部固定模型
pnpm check              # 格式 + 代码检查 + 类型检查
pnpm check:fix          # 自动修复格式与可修复的检查问题
pnpm typecheck          # 仅类型检查
pnpm lint               # 仅代码检查
pnpm fmt                # 仅格式化
pnpm check:rust         # cargo fmt --check + clippy -D warnings
pnpm test               # 桌面 UI/状态与共享 i18n 测试
pnpm build              # 构建桌面前端与扩展

pnpm --filter @sonasub/desktop tauri:build
pnpm --filter @sonasub/desktop exec tauri build --debug --bundles app --no-sign
pnpm build:extension:zip
```

Rust 测试：

```bash
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1
```

依赖大模型的测试默认忽略：

```bash
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --test native_pipeline -- --ignored --test-threads=1 --nocapture
```

## 配置规则

设置面板包含：

- 实时音频来源（`systemOutput` 或 `microphone`）及一个设备，或跟随该来源的
  当前系统默认设备；
- 录制音源：任意数量的系统输出与麦克风设备，每路独立采集并维护自己的字幕
  时间轴；
- ASR 模式（`auto` / `streaming` / `segmented`）、模型与源语言；
- 最大分段长度、VAD 阈值、最小静音/语音时长、稳定等待时间；
- 翻译模型（本地 Bergamot 方向或已配置供应商中的模型）、模式
  （`auto` / `streaming` / `text`）、触发策略
  （`finalOnly` / `stableSegment`）与目标语言；
- 大模型供应商（Base URL、API Key、可用模型与并发数）；
- 说话人嵌入/Diarization、窗口长度、最大人数与匹配阈值；
- 字幕历史上限。

`auto` 由当前模型与已编译适配器决定；不支持的显式模式会被拒绝并在界面
禁用。Bergamot 只能返回完整文本，因此 `streaming` 不可用。配置原子写入，
会话运行期间锁定，下次启动或重启时生效。

Bergamot 只开放两个固定方向（`en` 与 `zh-Hans` 目标），其余目标会在界面中
禁用并需要选择供应商模型。远端适配器接受 30 个已注册目标语言，
最终能力由远端模型决定。Base URL 必须是完整的 `http://` 或 `https://` 地址；
末尾 `/` 和误填的 `/chat/completions` 后缀会在请求前移除。本地无鉴权端点
可以留空 API Key。

供应商的 Base URL、模型与 API Key 以明文保存在本地 `providers.json`；会话从供应商中
选择模型，解析后的连接信息会保存在 `config.json` 和 `records.sqlite` 配置快照中。
调试输出、日志与错误信息会脱敏 API Key；不提供环境变量回退。

## 架构

```
apps/desktop                 React 工作台 + src-tauri Rust 应用
  src/                       Tauri IPC/事件、实时字幕与配置界面
  src-tauri/src/audio/       CPAL 采集与设备格式 -> 16k DSP
  src-tauri/src/asr/         流式 Zipformer、分段 SenseVoice 与 Qwen3
  src-tauri/src/speaker/     CAM++ 注册表与窗口化 Diarization
  src-tauri/src/translation/ TranslationEngine 边界、Bergamot 与远端适配器
  src-tauri/src/subtitles/   稳定 ID、修订号与 Patch 调度
  src-tauri/src/session/     实时字幕会话的生命周期、状态与事件
  src-tauri/src/records/     会话、各设备的采集任务与 SQLite 存储
  src-tauri/src/pipeline/    实时字幕与录制共用的推理线程、队列与指标
  src-tauri/src/models/      模型注册表、能力查询与安装
apps/extension               Chrome 扩展 popup（尚无采集）
packages/ui                  共享组件、主题、图标与样式
packages/i18n                英文与简体中文界面资源
docs/architecture.md         运行时边界与链路细节
docs/models.md               模型来源、版本与校验信息
docs/validation.md           已执行的检查、原生验证结果与遗留项
```

React 只通过 Tauri 命令与事件控制 Rust。音频样本、模型句柄与推理 Worker
不会进入 JavaScript。完整链路见 [docs/architecture.md](docs/architecture.md)，
开发约定见 [AGENTS.md](AGENTS.md)。

## 打包

macOS 验证包位于：

```text
apps/desktop/src-tauri/target/debug/bundle/macos/SonaSub.app
```

安装包只包含可执行文件、图标与 plist，不包含 Node.js、旧引擎产物、
WebSocket 协议或模型权重。本地未签名包不等同于已签名/公证的发布包。

## 验证

本次变更的最终验证清单会覆盖 TypeScript 检查与测试、Rust 格式/Clippy/测试、
忽略标记的原生模型测试，以及使用真实系统输出播放的 macOS `.app` 验证：
两条 ASR 路径、两个 Bergamot 方向、说话人与重叠语音行为、配置持久化、
停止/重启与退出清理。

Windows、Linux 与 macOS x64 安装包尚未验证。

## Star History

<a href="https://www.star-history.com/?repos=moil-xm%2FSonaSub&type=date&legend=top-left">
 <picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&theme=dark&legend=top-left" />
  <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&theme=light&legend=top-left" />
  <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&legend=top-left" />
 </picture>
</a>
