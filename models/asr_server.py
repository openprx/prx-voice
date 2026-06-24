#!/usr/bin/env python3
"""
PRX Voice — 本地 ASR HTTP Server (sherpa-onnx)
接收 POST /asr 的 PCM16 音频，返回识别文本。

启动: python3 models/asr_server.py
端口: 8765
"""
import json
import os
import struct
import traceback
from http.server import HTTPServer, BaseHTTPRequestHandler
from socketserver import ThreadingMixIn
import sherpa_onnx

# 模型路径 — 优先中文模型，可通过环境变量 ASR_MODEL_DIR 覆盖
MODEL_DIR = os.environ.get(
    "ASR_MODEL_DIR",
    "models/sherpa-onnx-streaming-zipformer-zh-14M-2023-02-23",
)

PORT = int(os.environ.get("ASR_PORT", "8765"))

print(f"Loading ASR model from {MODEL_DIR}...")

recognizer = sherpa_onnx.OnlineRecognizer.from_transducer(
    encoder=f"{MODEL_DIR}/encoder-epoch-99-avg-1.int8.onnx",
    decoder=f"{MODEL_DIR}/decoder-epoch-99-avg-1.int8.onnx",
    joiner=f"{MODEL_DIR}/joiner-epoch-99-avg-1.int8.onnx",
    tokens=f"{MODEL_DIR}/tokens.txt",
    num_threads=4,
    sample_rate=16000,
    feature_dim=80,
    decoding_method="greedy_search",
)

print("ASR model loaded.")


class ASRHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path == "/asr":
            try:
                content_length = int(self.headers.get("Content-Length", 0))
                audio_bytes = self.rfile.read(content_length)

                if len(audio_bytes) < 100:
                    self.send_json({"text": "", "error": "audio too short"})
                    return

                # PCM16 int16 little-endian → float32
                n_samples = len(audio_bytes) // 2
                samples = struct.unpack(f"<{n_samples}h", audio_bytes[:n_samples * 2])
                float_samples = [s / 32768.0 for s in samples]

                # Process in chunks of 16000 samples (1 second) to avoid memory issues
                stream = recognizer.create_stream()
                chunk_size = 16000
                for i in range(0, len(float_samples), chunk_size):
                    chunk = float_samples[i : i + chunk_size]
                    stream.accept_waveform(16000, chunk)
                    while recognizer.is_ready(stream):
                        recognizer.decode_stream(stream)

                # Feed tail silence to flush the decoder
                tail_padding = [0.0] * 4800  # 0.3s silence
                stream.accept_waveform(16000, tail_padding)
                stream.input_finished()

                while recognizer.is_ready(stream):
                    recognizer.decode_stream(stream)

                result = recognizer.get_result(stream)
                text = (result.text.strip() if hasattr(result, 'text') else str(result).strip())
                print(f"ASR: [{len(audio_bytes)} bytes, {n_samples} samples] -> \"{text}\"")
                self.send_json({"text": text})
            except Exception as e:
                traceback.print_exc()
                self.send_json({"text": "", "error": str(e)})
        else:
            self.send_error(404)

    def do_GET(self):
        if self.path == "/health":
            self.send_json({"status": "ok", "model": MODEL_DIR})
        else:
            self.send_error(404)

    def send_json(self, data):
        body = json.dumps(data, ensure_ascii=False).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass  # quiet


class ThreadedHTTPServer(ThreadingMixIn, HTTPServer):
    """Handle requests in separate threads."""
    daemon_threads = True


if __name__ == "__main__":
    server = ThreadedHTTPServer(("0.0.0.0", PORT), ASRHandler)
    print(f"ASR server ready: http://localhost:{PORT}/asr")
    server.serve_forever()
