# Speech Models

PRX Voice Engine uses several third-party speech models for ASR, TTS, voice
activity detection, and speaker verification. These models are **not** included
in this repository (the `models/` directory is excluded via `.gitignore` because
the files are large binaries). Download them separately from their upstream
sources.

## Models Used

| Model | Purpose | Upstream Source | License |
|-------|---------|-----------------|---------|
| Silero VAD (`silero_vad.onnx`) | Voice activity detection | https://github.com/snakers4/silero-vad | MIT |
| Sherpa-ONNX streaming Zipformer (zh) | Chinese ASR | https://github.com/k2-fsa/sherpa-onnx | Apache-2.0 |
| Sherpa-ONNX (en) | English ASR | https://github.com/k2-fsa/sherpa-onnx | Apache-2.0 |
| Sherpa-ONNX TTS (en) | English TTS | https://github.com/k2-fsa/sherpa-onnx | Apache-2.0 |
| VITS zh (`vits-zh-hf-theresa`) | Chinese TTS | https://huggingface.co/csukuangfj/vits-zh-hf-theresa | See upstream |
| 3D-Speaker ERES2Net | Speaker verification | https://github.com/modelscope/3D-Speaker | Apache-2.0 |

## Important: Verify Licenses Before Redistribution

> The licenses above reflect the upstream projects at the time of writing. Model
> weights can carry terms that differ from the surrounding toolkit, and upstream
> licensing can change. **Before redistributing, bundling, or using any model
> commercially, verify the exact license and attribution requirements at its
> upstream source.** This project's own MIT license covers the source code only,
> not the third-party model weights.

## Downloading

Models are expected under `models/`. Obtain each model from its upstream source
listed above and place it in the path the configuration expects. See the helper
scripts and server configuration for the exact directory layout.
