# Security Policy — dsh-diting

The DSH plugin package (`dsh-diting`) is a thin HTTP client over an
diting engine; it registers `agx_*` tools, holds no credentials beyond
the optional `apiKey` config value, and executes no page JavaScript itself.

## Supported versions

Security fixes land on `main` and ship in the next npm release. Only the
latest published version receives updates.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on the parent repository:
[yinnho/diting](https://github.com/yinnho/diting/security/advisories/new).

Please do not open public issues for suspected vulnerabilities. We aim to
respond within 72 hours. Include the package version, the harness version,
and a minimal repro (config + tool call) when possible.

## Scope notes

- Cookies or `apiKey` values passed through this plugin are forwarded to the
  configured engine only; they are never logged by the plugin.
- Vulnerabilities in the engine itself (SSRF posture, sandbox escapes,
  stealth/anti-bot handling) belong in the parent repository's security
  policy — see the root `SECURITY.md`.
