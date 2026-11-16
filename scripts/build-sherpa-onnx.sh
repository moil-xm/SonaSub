#!/usr/bin/env bash
set -euo pipefail

# sherpa-rs 0.6.8 binds the sherpa-onnx 1.12.9 C API. That release contains an
# out-of-bounds write in TopkIndex when a frame has more active speakers than
# clustered speakers (see sherpa-onnx issue fixed in v1.13.3). Upgrading the
# prebuilt runtime would change the C struct layout, so this project builds the
# pinned v1.12.9 runtime with the upstream clamp fix instead.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="1.12.9"
TARBALL_SHA256="8e1e8708750b7adf06faefa58922ed58ec42edffdfc043c2db15d894184443ec"
PATCH_FILE="$ROOT/scripts/patches/sherpa-onnx-1.12.9-topk-clamp.patch"
VENDOR_DIR="$ROOT/apps/desktop/src-tauri/vendor/sherpa-onnx"
CACHE_DIR="$ROOT/node_modules/.cache/sherpa-onnx"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Preparing sherpa-onnx is currently supported on macOS only." >&2
  exit 1
fi

for tool in cmake curl patch shasum tar; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Missing required tool: $tool" >&2
    if [[ "$tool" == "cmake" ]]; then
      echo "Install CMake 3.20 or newer, for example: brew install cmake" >&2
    fi
    exit 1
  fi
done

CMAKE_VERSION="$(cmake --version | head -1 | awk '{print $3}')"
CMAKE_MAJOR="${CMAKE_VERSION%%.*}"
CMAKE_MINOR="${CMAKE_VERSION#*.}"
CMAKE_MINOR="${CMAKE_MINOR%%.*}"
if (( CMAKE_MAJOR < 3 || (CMAKE_MAJOR == 3 && CMAKE_MINOR < 20) )); then
  echo "CMake 3.20 or newer is required; found $CMAKE_VERSION." >&2
  exit 1
fi

PATCH_SHA="$(shasum -a 256 "$PATCH_FILE" | awk '{print $1}')"
INSTALL_DIR="$VENDOR_DIR/$VERSION-${PATCH_SHA:0:12}"
CURRENT_LINK="$VENDOR_DIR/current"
STAMP="$INSTALL_DIR/.sonasub-native"
STAMP_CONTENT="sherpa-onnx $VERSION tarball=$TARBALL_SHA256 patch=$PATCH_SHA"

if [[ -f "$STAMP" && "$(cat "$STAMP")" == "$STAMP_CONTENT" ]] &&
  [[ -L "$CURRENT_LINK" && "$(readlink "$CURRENT_LINK")" == "$(basename "$INSTALL_DIR")" ]]; then
  echo "Patched sherpa-onnx $VERSION is already prepared at $INSTALL_DIR"
  exit 0
fi

mkdir -p "$CACHE_DIR"
SOURCE_DIR="$CACHE_DIR/source-$VERSION-${PATCH_SHA:0:12}"
BUILD_DIR="$CACHE_DIR/build-$VERSION-${PATCH_SHA:0:12}"

if [[ ! -d "$SOURCE_DIR" ]]; then
  WORK_DIR="$(mktemp -d "$CACHE_DIR/prepare.XXXXXX")"
  trap 'rm -rf "$WORK_DIR"' EXIT

  if [[ -n "${SONASUB_SHERPA_TARBALL:-}" ]]; then
    echo "Using sherpa-onnx $VERSION source from $SONASUB_SHERPA_TARBALL"
    cp "$SONASUB_SHERPA_TARBALL" "$WORK_DIR/sherpa-onnx.tar.gz"
  else
    echo "Downloading sherpa-onnx $VERSION source..."
    curl --fail --location --silent --show-error \
      --connect-timeout 30 --max-time 300 --retry 4 --retry-all-errors --retry-delay 2 \
      "https://github.com/k2-fsa/sherpa-onnx/archive/refs/tags/v$VERSION.tar.gz" \
      --output "$WORK_DIR/sherpa-onnx.tar.gz"
  fi

  ACTUAL_SHA="$(shasum -a 256 "$WORK_DIR/sherpa-onnx.tar.gz" | awk '{print $1}')"
  if [[ "$ACTUAL_SHA" != "$TARBALL_SHA256" ]]; then
    echo "sherpa-onnx source checksum mismatch." >&2
    echo "Expected: $TARBALL_SHA256" >&2
    echo "Actual:   $ACTUAL_SHA" >&2
    exit 1
  fi

  tar -xzf "$WORK_DIR/sherpa-onnx.tar.gz" -C "$WORK_DIR"
  mv "$WORK_DIR/sherpa-onnx-$VERSION" "$WORK_DIR/source"
  patch -p1 -d "$WORK_DIR/source" < "$PATCH_FILE"
  mv "$WORK_DIR/source" "$SOURCE_DIR"

  rm -rf "$WORK_DIR"
  trap - EXIT
fi

JOBS="${SONASUB_CMAKE_JOBS:-$(sysctl -n hw.ncpu)}"
DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.4}"
DEPS_DIR="${SONASUB_SHERPA_DEPS_DIR:-$CACHE_DIR/deps}"
DISCONNECTED="OFF"
if [[ -d "$DEPS_DIR/onnxruntime-src" && -d "$DEPS_DIR/kaldi_native_fbank-src" ]]; then
  DISCONNECTED="ON"
fi

mkdir -p "$BUILD_DIR" "$DEPS_DIR"
cmake -S "$SOURCE_DIR" -B "$BUILD_DIR" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
  -DCMAKE_INSTALL_PREFIX="$INSTALL_DIR" \
  -DCMAKE_OSX_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET" \
  -DFETCHCONTENT_BASE_DIR="$DEPS_DIR" \
  -DFETCHCONTENT_FULLY_DISCONNECTED="$DISCONNECTED" \
  -DBUILD_SHARED_LIBS=OFF \
  -DSHERPA_ONNX_ENABLE_C_API=ON \
  -DSHERPA_ONNX_ENABLE_BINARY=ON \
  -DSHERPA_ONNX_ENABLE_PORTAUDIO=OFF \
  -DSHERPA_ONNX_ENABLE_WEBSOCKET=OFF \
  -DSHERPA_ONNX_ENABLE_TTS=OFF \
  -DSHERPA_ONNX_BUILD_C_API_EXAMPLES=OFF

cmake --build "$BUILD_DIR" \
  --target sherpa-onnx-c-api sherpa-onnx-offline-speaker-diarization \
  --parallel "$JOBS"

mkdir -p "$INSTALL_DIR/lib" "$INSTALL_DIR/include/sherpa-onnx/c-api" "$INSTALL_DIR/bin"
cp "$BUILD_DIR"/lib/*.a "$INSTALL_DIR/lib/"
cp "$DEPS_DIR/onnxruntime-src/lib/libonnxruntime.a" "$INSTALL_DIR/lib/"
cp "$SOURCE_DIR/sherpa-onnx/c-api/c-api.h" \
  "$SOURCE_DIR/sherpa-onnx/c-api/cxx-api.h" \
  "$INSTALL_DIR/include/sherpa-onnx/c-api/"
cp "$BUILD_DIR/bin/sherpa-onnx-offline-speaker-diarization" "$INSTALL_DIR/bin/"

for library in libsherpa-onnx-c-api.a libsherpa-onnx-core.a libonnxruntime.a; do
  if [[ ! -f "$INSTALL_DIR/lib/$library" ]]; then
    echo "Missing expected static library: $INSTALL_DIR/lib/$library" >&2
    exit 1
  fi
done

printf '%s\n' "$STAMP_CONTENT" > "$STAMP"
ln -sfn "$(basename "$INSTALL_DIR")" "$CURRENT_LINK"
echo "Prepared patched sherpa-onnx $VERSION at $INSTALL_DIR"
