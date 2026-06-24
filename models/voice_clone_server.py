#!/usr/bin/env python3
"""
PRX Voice — 声纹克隆服务 (F5-TTS zero-shot voice cloning)
功能：
  POST /speakers          — 上传参考音频 (PCM16 16kHz)，保存为 speaker profile
  GET  /speakers          — 列出所有 speaker profiles
  GET  /speakers/{id}/audio — 播放录音原始音频
  DELETE /speakers/{id}   — 删除 speaker profile
  POST /tts               — 用 F5-TTS 克隆指定 speaker 的声音合成语音
  GET  /health            — 健康检查

端口: 8768
"""
import json
import os
import struct
import time
import traceback
import uuid
import wave
from http.server import HTTPServer, BaseHTTPRequestHandler
from socketserver import ThreadingMixIn
from pathlib import Path

import numpy as np
import soundfile as sf
import torch
import torchaudio

# Monkey-patch torchaudio.load for torchcodec compatibility issue
def _sf_load(path, *args, **kwargs):
    data, sr = sf.read(path, dtype='float32')
    if data.ndim == 1:
        data = data.reshape(1, -1)
    else:
        data = data.T
    return torch.from_numpy(data), sr

torchaudio.load = _sf_load

from f5_tts.api import F5TTS

PORT = int(os.environ.get("CLONE_PORT", "8768"))
SPEAKERS_DIR = Path("models/speakers")
SPEAKERS_DIR.mkdir(parents=True, exist_ok=True)
OUTPUT_SAMPLE_RATE = 24000  # F5-TTS native output rate

print("Loading F5-TTS model (first load downloads ~1.2GB)...")
t0 = time.time()
tts_model = F5TTS(device="mps")  # Apple Silicon GPU
print(f"F5-TTS loaded in {time.time()-t0:.1f}s")

# In-memory speaker registry
speaker_registry: dict[str, dict] = {}

ASR_URL = os.environ.get("ASR_URL", "http://localhost:8765")

def transcribe_via_asr(pcm_path: str) -> str:
    """Transcribe a PCM16 file via the local ASR HTTP server."""
    import requests
    if not pcm_path or not os.path.exists(pcm_path):
        return ""
    try:
        with open(pcm_path, "rb") as f:
            audio_bytes = f.read()
        # Trim to first 8 seconds for ASR
        max_bytes = 16000 * 2 * 8
        if len(audio_bytes) > max_bytes:
            audio_bytes = audio_bytes[:max_bytes]
        resp = requests.post(f"{ASR_URL}/asr", data=audio_bytes, timeout=30)
        if resp.status_code == 200:
            text = resp.json().get("text", "").strip()
            print(f"  ASR transcription: \"{text}\"")
            return text
    except Exception as e:
        print(f"  ASR transcription failed: {e}")
    return ""


def load_speakers():
    for f in SPEAKERS_DIR.glob("*.json"):
        try:
            data = json.loads(f.read_text())
            sid = data["id"]
            # Migrate: generate WAV from PCM if missing
            wav_path = str(SPEAKERS_DIR / f"{sid}.wav")
            pcm_path = data.get("audio_path", str(SPEAKERS_DIR / f"{sid}.pcm"))
            if not data.get("wav_path") or not os.path.exists(data.get("wav_path", "")):
                if os.path.exists(pcm_path):
                    with open(pcm_path, "rb") as pf:
                        pcm16_to_wav(pf.read(), 16000, wav_path)
                    data["wav_path"] = wav_path
                    f.write_text(json.dumps({k: v for k, v in data.items() if k != "embedding"}, ensure_ascii=False))
                    print(f"  Migrated {sid}: generated {wav_path}")
            speaker_registry[sid] = data
        except Exception:
            pass
    print(f"Loaded {len(speaker_registry)} speaker profiles")


def pcm16_to_wav(pcm_bytes: bytes, sample_rate: int, wav_path: str):
    """Convert raw PCM16 bytes to a WAV file."""
    n_samples = len(pcm_bytes) // 2
    with wave.open(wav_path, 'wb') as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sample_rate)
        w.writeframes(pcm_bytes)


class CloneHandler(BaseHTTPRequestHandler):

    def do_POST(self):
        if self.path == "/speakers":
            self.handle_create_speaker()
        elif self.path == "/tts":
            self.handle_tts()
        else:
            self.send_error(404)

    def do_GET(self):
        if self.path == "/speakers":
            self.handle_list_speakers()
        elif self.path.startswith("/speakers/") and self.path.endswith("/audio"):
            speaker_id = self.path.split("/")[-2]
            self.handle_get_audio(speaker_id)
        elif self.path == "/health":
            self.send_json({
                "status": "ok",
                "engine": "F5-TTS",
                "speakers": len(speaker_registry),
                "device": str(tts_model.device),
            })
        else:
            self.send_error(404)

    def do_DELETE(self):
        if self.path.startswith("/speakers/"):
            speaker_id = self.path.split("/")[-1]
            self.handle_delete_speaker(speaker_id)
        else:
            self.send_error(404)

    def handle_create_speaker(self):
        try:
            content_length = int(self.headers.get("Content-Length", 0))
            audio_bytes = self.rfile.read(content_length)
            name = self.headers.get("X-Speaker-Name", f"Speaker-{int(time.time())}")
            voice_tag = self.headers.get("X-Voice-Tag", "zh-female")

            if len(audio_bytes) < 16000 * 2 * 3:  # less than 3s at 16kHz
                self.send_json({"error": "Audio too short, need at least 3 seconds"}, 400)
                return

            speaker_id = str(uuid.uuid4())[:8]

            # Save raw PCM
            pcm_path = str(SPEAKERS_DIR / f"{speaker_id}.pcm")
            with open(pcm_path, "wb") as f:
                f.write(audio_bytes)

            # Also save as WAV (needed by F5-TTS as reference)
            wav_path = str(SPEAKERS_DIR / f"{speaker_id}.wav")
            pcm16_to_wav(audio_bytes, 16000, wav_path)

            duration_sec = len(audio_bytes) / 2 / 16000

            profile = {
                "id": speaker_id,
                "name": name,
                "voice_tag": voice_tag,
                "audio_path": pcm_path,
                "wav_path": wav_path,
                "audio_duration_sec": duration_sec,
                "created": time.strftime("%Y-%m-%d %H:%M:%S"),
            }

            (SPEAKERS_DIR / f"{speaker_id}.json").write_text(
                json.dumps(profile, ensure_ascii=False)
            )
            speaker_registry[speaker_id] = profile

            print(f"Speaker created: {speaker_id} ({name}, {duration_sec:.1f}s)")

            self.send_json({
                "id": speaker_id,
                "name": name,
                "voice_tag": voice_tag,
                "audio_duration_sec": duration_sec,
                "embedding_dim": 0,
            })

        except Exception as e:
            traceback.print_exc()
            self.send_json({"error": str(e)}, 500)

    def handle_list_speakers(self):
        items = []
        for p in speaker_registry.values():
            items.append({
                "id": p["id"],
                "name": p["name"],
                "voice_tag": p.get("voice_tag", "zh-female"),
                "audio_duration_sec": p.get("audio_duration_sec", 0),
                "created": p.get("created", ""),
            })
        self.send_json({"speakers": items})

    def handle_get_audio(self, speaker_id: str):
        if speaker_id not in speaker_registry:
            self.send_json({"error": "Speaker not found"}, 404)
            return
        audio_path = speaker_registry[speaker_id].get("audio_path", "")
        if not audio_path or not os.path.exists(audio_path):
            self.send_json({"error": "Audio file not found"}, 404)
            return
        with open(audio_path, "rb") as f:
            audio_data = f.read()
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(len(audio_data)))
        self.send_header("X-Sample-Rate", "16000")
        self.end_headers()
        self.wfile.write(audio_data)

    def handle_delete_speaker(self, speaker_id: str):
        if speaker_id not in speaker_registry:
            self.send_json({"error": "Speaker not found"}, 404)
            return
        p = speaker_registry.pop(speaker_id)
        for ext in (".json", ".pcm", ".wav"):
            path = SPEAKERS_DIR / f"{speaker_id}{ext}"
            if path.exists():
                path.unlink()
        print(f"Speaker deleted: {speaker_id} ({p['name']})")
        self.send_json({"deleted": speaker_id})

    def handle_tts(self):
        try:
            content_length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(content_length))
            text = body.get("text", "").strip()
            speaker_id = body.get("speaker", "")

            if not text:
                self.send_json({"error": "empty text"})
                return

            if not speaker_id or speaker_id not in speaker_registry:
                self.send_json({"error": f"Speaker '{speaker_id}' not found"}, 404)
                return

            profile = speaker_registry[speaker_id]
            wav_path = profile.get("wav_path", "")
            if not wav_path or not os.path.exists(wav_path):
                self.send_json({"error": "Reference audio not found"}, 404)
                return

            # Trim reference audio to max 8 seconds for speed
            ref_wav_path = wav_path
            ref_data, ref_sr = sf.read(wav_path, dtype='float32')
            max_ref_samples = ref_sr * 8  # 8 seconds max
            if len(ref_data) > max_ref_samples:
                ref_data = ref_data[:max_ref_samples]
                trimmed_path = str(SPEAKERS_DIR / f"{speaker_id}_trimmed.wav")
                sf.write(trimmed_path, ref_data, ref_sr)
                ref_wav_path = trimmed_path

            # Get reference text: use cached transcription or call ASR server
            ref_text = profile.get("ref_text", "")
            if not ref_text:
                ref_text = transcribe_via_asr(profile.get("audio_path", ""))
                if ref_text:
                    profile["ref_text"] = ref_text
                    # Cache it
                    json_path = SPEAKERS_DIR / f"{speaker_id}.json"
                    saved = {k: v for k, v in profile.items() if k != "embedding"}
                    json_path.write_text(json.dumps(saved, ensure_ascii=False))

            if not ref_text:
                ref_text = "你好"  # fallback

            # F5-TTS zero-shot voice cloning (nfe_step=8 for speed, default=32)
            t0 = time.time()
            wav, sr, _ = tts_model.infer(
                ref_file=ref_wav_path,
                ref_text=ref_text,
                gen_text=text,
                speed=1.0,
                nfe_step=8,
            )
            elapsed = time.time() - t0

            # Convert to numpy
            if isinstance(wav, torch.Tensor):
                wav_np = wav.squeeze().cpu().numpy()
            else:
                wav_np = np.asarray(wav).squeeze()

            # Resample from 24kHz to 16kHz for consistency
            if sr != 16000:
                from scipy.signal import resample_poly
                wav_np = resample_poly(wav_np, 16000, sr).astype(np.float32)
                sr = 16000

            # Float32 → PCM16 bytes
            pcm16 = np.clip(wav_np * 32767, -32768, 32767).astype(np.int16)
            pcm_bytes = pcm16.tobytes()

            duration_ms = len(pcm16) * 1000 // sr
            print(f"F5-TTS clone: speaker={speaker_id} \"{text[:30]}\" -> {len(pcm_bytes)} bytes, {duration_ms}ms, gen={elapsed:.1f}s")

            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(pcm_bytes)))
            self.send_header("X-Sample-Rate", str(sr))
            self.send_header("X-Duration-Ms", str(duration_ms))
            self.end_headers()
            self.wfile.write(pcm_bytes)

        except Exception as e:
            traceback.print_exc()
            self.send_json({"error": str(e)}, 500)

    def send_json(self, data, status=200):
        body = json.dumps(data, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass


class ThreadedHTTPServer(ThreadingMixIn, HTTPServer):
    daemon_threads = True


if __name__ == "__main__":
    load_speakers()
    server = ThreadedHTTPServer(("0.0.0.0", PORT), CloneHandler)
    print(f"Voice Clone server (F5-TTS) ready: http://localhost:{PORT}")
    server.serve_forever()
