# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Unified WebSocket streaming endpoint (`/api/v1/stream`) with session
  create / close / audio-end handlers
- Per-connection metadata and a connection manager
- Background health-check loop that detects dead connections and zombie sessions

## [0.1.0] - 2026-03-27

### Added
- Initial PRX Voice Engine: real-time voice conversation orchestration
- Session state machine and event bus
- Pluggable ASR / Agent / TTS adapter interfaces
- REST / gRPC / WebSocket control API
- Multi-tenant isolation and RBAC
- Billing, quota, and metering
- Observability (tracing, metrics, diagnostics)
- Storage layer with multi-plane architecture
