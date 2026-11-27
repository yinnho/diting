# Diting Architecture

> **Status: authoritative.** This file legislates the layering of the tracked
> codebase. A change that crosses one of these boundaries updates this file in
> the same commit or does not land. `docs/` is local scratch by repository
> policy, so the authoritative copy intentionally lives at the repository root.
>
> Basis: v0.12.0 audit, rewritten main at the post-migration workspace shape.

## 1. What Diting is

Diting is a server-side browser engine and agent-facing acquisition product:
pages are fetched, rendered, searched, watched, driven, and re-emitted as data
or artifacts without Chromium. The engine owns the browser implementation; the
product owns acquisition, interaction, authoring, accounts, and protocol
faces.

The product thesis remains:

> **The agent's eyes, hands, and mouth — on an engine we own, behind explicit
> network/protocol doors.**

- **Eyes** — acquire: fetch, metasearch, download, cache, extraction.
- **Hands** — interact: sessions, flows, accounts/personas, input, observation.
- **Mouth** — author: screenshots, PDF, video, OOXML, markdown/docgen output.
- **Foundation** — the engine: network, DOM, CSS, layout, paint, fonts, JS
  runtime, page orchestration.
- **Doors** — the protocol faces: HTTP REST, the CDP websocket bridge, and the
  CLI/diagnostic entrypoints.

## 1.1 Product strategy guardrail

The current owner-approved strategy is **engine-first, local/self-hosted, and
single-binary**. Diting is not currently trying to become a hosted multi-tenant
browser platform. Do not roadmap cloud control planes, hosted session fleets,
billing/usage metering, managed proxy businesses, hosted runtimes, or platform
template marketplaces merely because Browserbase, Steel, Hyperbrowser, or
Lexmount have them. A previous hosted/free-trial attempt was abused at a cost
the current team cannot carry; any reversal of this boundary is a product
decision requiring explicit owner direction, not an inference from competitor
movement.

Competitor watching therefore has a narrow purpose: improve the engine and its
agent-facing usability. Moli, Obscura, Lightpanda, and similar projects are
signals for browser correctness, performance, extraction quality, protocol
compatibility, and real-site reliability. Hosted-platform packaging is market
intelligence only, not a feature mandate.

High-value site-specific flows are **private operational memory**, not public
content inventory. Publishing concrete site flows can invite countermeasures,
make the flows rot faster, and create an unwanted maintenance/anti-abuse
burden. The engine ships no site-specific business logic or signature chains.
The desired loop is instead:

```text
agent explores a site once
  → distills the successful path into a private flow
  → strips or references secrets rather than embedding them
  → validates replay
  → repairs the flow from structured failure receipts when the site drifts
  → reuses the flow with zero additional model exploration cost
```

Public documentation may teach the flow format and use generic examples, but
must not become a public catalog of high-value site-specific know-how. Flow
assets belong to the user or organization that generated them; optional
sharing must stay inside an explicit trust boundary.

“Best engine” is measured by outcomes, not by counting browser API names:

1. real-site success, especially authenticated heavy SPAs and Chinese web
   workflows;
2. navigation, session, iframe, form, upload, storage, and document-lifecycle
   correctness;
3. startup time, memory use, concurrency, and long-running stability;
4. acquisition quality: extraction, markdown, search merging, receipts, and
   provenance useful to agents;
5. agent operability: stable typed APIs, diagnosable failures, replayable
   private flows, and zero-token reuse;
6. local control: no Chromium dependency, no required cloud control plane, and
   user custody of credentials, sessions, and workflow assets.

## 2. Dependency direction, crate map, and binding rules

The compile-time dependency direction is one-way:

```text
faces → product/acquisition → authoring → engine facade/page → engine crates
```

The workspace has three ownership classes.

### House product crates

- `diting-browser` — product binary, acquisition server, sessions, search,
  flows, accounts, protocol faces, panel integration.
- `diting_authoring` — document/render artifact cluster: page pump, video,
  OOXML, docgen.

### House engine crates

- `diting` — namespace facade over the engine crates. It is a compatibility
  and assembly facade, not a proof that consumers use only `Page`.
- `diting_env` — environment constants, Chrome target, route/policy tables.
- `diting_dom` — live engine DOM and selector integration.
- `diting_css` — CSS parsing/cascade/computed style.
- `diting_layout` — layout, paint, text, fonts, image/paint data.
- `diting_net` — browser-grade HTTP client surface, policies, cookies,
  callbacks, network failure typing, stealth client construction.
- `diting_js` — Web-platform runtime integration: ops, realm bootstrap, timers,
  fetch/XHR, DOM bindings, module loading, watchdog interaction.
- `diting_page` — page/browser context orchestration and the high-level `Page`.
- `diting_runtime` — owned V8 runtime substrate: isolate/vm loop, op settlement,
  promise table, snapshots, module machinery.
- `diting_ops` — op macro support used by the runtime layer.

### Vendored crates

Vendored crates under `crates/` are upstream-derived dependencies maintained
under the rules in `crates/VENDORED.md`. They are not first-party architecture
debt merely because they are large or use upstream style. Do not refactor them
for line-count or style reasons without treating the upstream sync boundary.

### R1 — Downward only

Faces may call product logic. Product and authoring may call engine APIs.
Engine crates never name product, authoring, or face modules. Nothing outside
face assembly imports a face.

The protocol faces are:

- `crates/diting-browser/src/routers/`
- `crates/diting-browser/src/firecrawl_compat.rs`
- `crates/diting-browser/src/doctor_cli.rs`
- `crates/diting-browser/src/cdp/`
- thin assembly in `crates/diting-browser/src/main.rs`

CDP is a deliberate, current product face. Any document or script claiming
that CDP was removed is stale.

### R2 — Engine purity

Engine crates must not depend on product web-framework or face dependencies
such as Axum, tower, product authoring formats, account/session logic, or the
product cross-check pipeline. Engine capabilities are consumed through typed
Rust APIs.

### R3 — Faces are thin and memory-safe

Faces own wire parsing, routing, parameter validation, protocol state, and
translation into typed product/session commands. Page behavior, browser
policy, and engine decisions move down. Face code carries no `unsafe`.

The CDP face may own protocol session/routing state because CDP session and
event ownership are themselves protocol concerns. It must not grow a second
browser engine or duplicate session business logic.

### R4 — One owner per runtime concern

A runtime concern has one owner. Examples already legislated by the audit:

- `diting_page::Page` owns document-start script groups.
- `diting_runtime` owns V8 loop/op-settlement semantics.
- `diting_js` owns web-platform integration atop that substrate.
- Network policy belongs in `diting_net`, not in protocol faces.
- Authoring output ownership belongs in `diting_authoring`, not in acquisition
  route handlers.

### R5 — Contracts are typed

Values crossing layer boundaries are named types with documentation. Ad-hoc
JSON is allowed only at the outer wire edge and in deliberately dynamic flow
DSL positions; it is validated there and translated before crossing downward.
Handles that cross ownership boundaries must be opaque values with failure
semantics, not raw pointers that bypass Rust lifetimes.

### R6 — Features stay few and honest

The product feature gates are:

- `stealth` — compatibility no-op; the wire/BoringSSL stack is always built.
  It exists so existing deploy/CI feature strings keep resolving.
- `screenshot` — layout/paint/image support for raster output.
- `authoring` — optional authoring cluster, enabled by default;
  `--no-default-features` is intended to produce the acquisition-only face.

Every feature combination documented as supported must have at least a
compile-level CI check. A no-op feature must say so in the manifest. Optional
dependencies belong to the feature that actually uses them.

## 3. Engine API levels

The engine facade exposes more than `Page` by design. There are three explicit
API levels:

1. **Page/runtime API** — high-level page navigation, evaluation, events.
2. **Acquisition/render API** — network clients/policies and layout/paint
   primitives required by the acquisition and authoring product.
3. **Internal engine API** — implementation details that are exported today
   because the workspace split is younger than the call graph.

Level 3 is tolerated legacy surface, not the desired end state. New product
code should prefer levels 1 and 2. Moving an internal API into product use is
an architecture change and requires updating this file.

## 4. Product decomposition and ownership

### D4 — Authoring cluster

`diting_authoring` owns the mouth-side artifact pipeline: deterministic docgen
HTML, page PDF/PNG/JPEG pumping, timeline video, OOXML, and native PPTX. Its
`screenshot` feature owns the rendering pieces that need engine layout/paint;
docgen is pure text and does not.

### DOM and query boundaries

There are two deliberately separate DOM-shaped worlds:

1. the live engine DOM in `diting_dom`, connected to layout, JS, mutation,
   and page lifetime;
2. the inert static query tree in product `htmlq`, used for search/extraction
   over already-downloaded HTML.

They do not share node identity. Selector behavior or tree lifetime must not
be silently assumed interchangeable. Shared parser/selector crates are allowed;
shared semantic identity is not.

### Product module splits

Large product modules split by responsibility, not by mechanical line count:
session actor/state/interaction/record, acquisition routes, output routes,
search engines, CDP protocol domains. A split must preserve behavior and make
the owner easier to name; it must not scatter one algorithm across files just
to satisfy a metric.

## 5. Runtime, interaction, generated, and asset boundaries

Interaction truth is the live page at the moment an action runs. Cached or
previously observed geometry is advisory only; action-side re-checks belong
with the interaction implementation so a click is not dispatched into a dead,
moved, disabled, or occluded target merely because stale state once saw it.

Generated and external-data boundaries:

- Generated files are freshness-checked by `tools/webplatform/check_fresh.sh`.
- Large asset ownership is ratcheted by `scripts/check_assets.py`.
- Chrome font truth is external and pinned through the truth repository; the
  main repository carries the compressed runtime artifact and reproducibility
  metadata, not the raw measurement corpus.
- `platform_bundle.js` is a generated concatenation. Its source modules are
  subject to normal complexity review; the bundle itself is not treated as a
  hand-written god file.
- Linguist classification in `.gitattributes` is descriptive, never a way to
  hide ownership problems.

## 6. Size and complexity rule

### P1 — Production-line cap

The production-line cap for first-party files is 1,500 lines. Test-only items
and whole `tests.rs` files do not count. Generated concatenations are exempt
only when their sources are individually governed.

### P2 — Responsibility-preserving splits

Files over cap must be split along responsibility boundaries. Do not split a
cohesive algorithm mechanically just to satisfy the number, and never raise a
grandfather count to make growth pass. If a rare file truly cannot be split
without behavior risk, it needs an explicit grandfather entry explaining the
behavioral constraint.

## 7. Verification contract

The local gate contract in `AGENTS.md` is binding. CI must run the same logic
or a stronger equivalent. A red CI job on main is a breakage, not noise.

Architectural audits are tripwires, not substitutes for type-system or runtime
verification. When they disagree with reality, the correct fix is to reconcile
this file, the audit, and the code — not to delete the signal.
