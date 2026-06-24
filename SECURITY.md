# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in PRX Voice Engine, please report it
responsibly.

**Do NOT open a public issue for security vulnerabilities.**

Instead, please email security@prx.dev (or open a private security advisory).

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We aim to acknowledge reports within 48 hours and provide a fix within 7 days
for critical issues.

## Security Design

- API credentials (ASR / Agent / TTS providers) are read from environment
  variables or configuration, never hard-coded
- Secrets are never written to logs; sensitive fields are redacted
- Database access uses parameterized queries only
- External inputs are validated at API boundaries; errors return typed responses
  rather than panicking
- Multi-tenant isolation and RBAC gate access to sessions and resources
