#!/usr/bin/env python3
"""
PRX Voice — 本地 TTS HTTP Server (sherpa-onnx VITS)
接收 POST /tts 的 JSON {text, speed?}，返回 PCM16 音频。

启动: python3 models/tts_server.py
端口: 8766
"""
import json
import os
import struct
import traceback
from http.server import HTTPServer, BaseHTTPRequestHandler
from socketserver import ThreadingMixIn
import sherpa_onnx

MODEL_DIR = os.environ.get("TTS_MODEL_DIR", "models/vits-zh-hf-theresa")
PORT = int(os.environ.get("TTS_PORT", "8766"))
OUTPUT_SAMPLE_RATE = 16000  # Resample to 16kHz for WebSocket streaming

print(f"Loading TTS model from {MODEL_DIR}...")

config = sherpa_onnx.OfflineTtsConfig(
    model=sherpa_onnx.OfflineTtsModelConfig(
        vits=sherpa_onnx.OfflineTtsVitsModelConfig(
            model=f"{MODEL_DIR}/theresa.onnx",
            tokens=f"{MODEL_DIR}/tokens.txt",
            lexicon=f"{MODEL_DIR}/lexicon.txt",
            dict_dir=f"{MODEL_DIR}/dict",
        ),
        num_threads=2,
    ),
)
tts = sherpa_onnx.OfflineTts(config)

print("TTS model loaded.")


def resample(samples, src_rate, dst_rate):
    """Simple linear interpolation resampling."""
    if src_rate == dst_rate:
        return samples
    ratio = dst_rate / src_rate
    n_out = int(len(samples) * ratio)
    out = []
    for i in range(n_out):
        src_pos = i / ratio
        idx = int(src_pos)
        frac = src_pos - idx
        if idx + 1 < len(samples):
            val = samples[idx] * (1 - frac) + samples[idx + 1] * frac
        elif idx < len(samples):
            val = samples[idx]
        else:
            val = 0.0
        out.append(val)
    return out


class TTSHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path == "/tts":
            try:
                content_length = int(self.headers.get("Content-Length", 0))
                body = self.rfile.read(content_length)
                req = json.loads(body)
                text = req.get("text", "").strip()
                speed = float(req.get("speed", 1.0))

                if not text:
                    self.send_json({"error": "empty text"})
                    return

                # Synthesize
                audio = tts.generate(text, sid=0, speed=speed)
                src_rate = audio.sample_rate
                samples = audio.samples

                # Resample to output rate if needed
                if src_rate != OUTPUT_SAMPLE_RATE:
                    samples = resample(samples, src_rate, OUTPUT_SAMPLE_RATE)

                # Convert float32 → PCM16 bytes
                pcm16 = struct.pack(f"<{len(samples)}h", *[
                    max(-32768, min(32767, int(s * 32767)))
                    for s in samples
                ])

                duration_ms = len(samples) * 1000 // OUTPUT_SAMPLE_RATE
                print(f"TTS: \"{text[:40]}\" -> {len(pcm16)} bytes, {duration_ms}ms")

                # Return binary PCM16 audio
                self.send_response(200)
                self.send_header("Content-Type", "application/octet-stream")
                self.send_header("Content-Length", str(len(pcm16)))
                self.send_header("X-Sample-Rate", str(OUTPUT_SAMPLE_RATE))
                self.send_header("X-Duration-Ms", str(duration_ms))
                self.end_headers()
                self.wfile.write(pcm16)

            except Exception as e:
                traceback.print_exc()
                self.send_json({"error": str(e)})
        else:
            self.send_error(404)

    def do_GET(self):
        if self.path == "/health":
            self.send_json({"status": "ok", "model": MODEL_DIR, "sample_rate": OUTPUT_SAMPLE_RATE})
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
        pass


class ThreadedHTTPServer(ThreadingMixIn, HTTPServer):
    daemon_threads = True


if __name__ == "__main__":
    server = ThreadedHTTPServer(("0.0.0.0", PORT), TTSHandler)
    print(f"TTS server ready: http://localhost:{PORT}/tts")
    server.serve_forever()
