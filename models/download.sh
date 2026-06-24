#!/bin/bash
set -e
echo "=== PRX Voice Engine — 模型下载 ==="

MODELS_DIR="$(cd "$(dirname "$0")" && pwd)"

# Sherpa ASR: streaming zipformer English
echo ""
echo "[1/3] 下载 Sherpa ASR 模型 (streaming zipformer en)..."
ASR_DIR="$MODELS_DIR/sherpa-asr-en"
if [ ! -d "$ASR_DIR" ]; then
    mkdir -p "$ASR_DIR"
    curl -SL "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-en-20M-2023-02-17.tar.bz2" | tar xj -C "$MODELS_DIR"
    mv "$MODELS_DIR/sherpa-onnx-streaming-zipformer-en-20M-2023-02-17" "$ASR_DIR"
    echo "  ✓ ASR 模型已下载到 $ASR_DIR"
else
    echo "  ✓ ASR 模型已存在"
fi

# Sherpa TTS: VITS piper English
echo ""
echo "[2/3] 下载 Sherpa TTS 模型 (VITS piper en)..."
TTS_DIR="$MODELS_DIR/sherpa-tts-en"
if [ ! -d "$TTS_DIR" ]; then
    mkdir -p "$TTS_DIR"
    curl -SL "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-lessac-medium.tar.bz2" | tar xj -C "$MODELS_DIR"
    mv "$MODELS_DIR/vits-piper-en_US-lessac-medium" "$TTS_DIR"
    echo "  ✓ TTS 模型已下载到 $TTS_DIR"
else
    echo "  ✓ TTS 模型已存在"
fi

# Sherpa VAD: silero
echo ""
echo "[3/3] 下载 Sherpa VAD 模型 (silero)..."
VAD_FILE="$MODELS_DIR/silero_vad.onnx"
if [ ! -f "$VAD_FILE" ]; then
    curl -SL "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx" -o "$VAD_FILE"
    echo "  ✓ VAD 模型已下载到 $VAD_FILE"
else
    echo "  ✓ VAD 模型已存在"
fi

echo ""
echo "=== 全部模型就绪 ==="
echo ""
echo "启动方式:"
echo "  cargo build --features sherpa -p prx-voice-bin --release"
echo "  PRX_VOICE_PORT=3200 ./target/release/prx-voice"
