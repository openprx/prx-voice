#!/usr/bin/env python3
"""
PRX Voice — English TTS HTTP Server (sherpa-onnx Piper VITS)
Same API as tts_server.py but uses English voice model.

启动: python3 models/tts_server_en.py
端口: 8767
"""
import json
import os
import struct
import traceback
from http.server import HTTPServer, BaseHTTPRequestHandler
from socketserver import ThreadingMixIn
import sherpa_onnx

MODEL_DIR = os.environ.get(
    "TTS_EN_MODEL_DIR",
    "models/sherpa-tts-en/vits-piper-en_US-lessac-medium",
)
PORT = int(os.environ.get("TTS_EN_PORT", "8767"))
OUTPUT_SAMPLE_RATE = 16000

print(f"Loading EN TTS model from {MODEL_DIR}...")

config = sherpa_onnx.OfflineTtsConfig(
    model=sherpa_onnx.OfflineTtsModelConfig(
        vits=sherpa_onnx.OfflineTtsVitsModelConfig(
            model=f"{MODEL_DIR}/en_US-lessac-medium.onnx",
            tokens=f"{MODEL_DIR}/tokens.txt",
            data_dir=f"{MODEL_DIR}/espeak-ng-data",
        ),
        num_threads=2,
    ),
)
tts = sherpa_onnx.OfflineTts(config)

print("EN TTS model loaded.")


def resample(samples, src_rate, dst_rate):
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

                audio = tts.generate(text, sid=0, speed=speed)
                samples = audio.samples
                if audio.sample_rate != OUTPUT_SAMPLE_RATE:
                    samples = resample(samples, audio.sample_rate, OUTPUT_SAMPLE_RATE)

                pcm16 = struct.pack(f"<{len(samples)}h", *[
                    max(-32768, min(32767, int(s * 32767))) for s in samples
                ])

                duration_ms = len(samples) * 1000 // OUTPUT_SAMPLE_RATE
                print(f"EN TTS: \"{text[:40]}\" -> {len(pcm16)} bytes, {duration_ms}ms")

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
            self.send_json({"status": "ok", "model": MODEL_DIR, "lang": "en", "sample_rate": OUTPUT_SAMPLE_RATE})
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
    print(f"EN TTS server ready: http://localhost:{PORT}/tts")
    server.serve_forever()
