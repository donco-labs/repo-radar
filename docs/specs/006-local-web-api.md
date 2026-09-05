# Feature Specification: Local Live Surface

Status: Planned
Priority: P0
Depends on: `002-structured-output`, `003-repository-intelligence`, `004-watch-mode`

## Goal

Make one command enough. `repo-radar serve` scans a repository, binds a loopback port, opens a browser view of the model, and keeps that view current as the repository changes underneath it.

This is the primary visual surface. The static `--format html` snapshot in [019](019-visual-report.md) remains, but its purpose is narrow: a frozen artifact to attach to a review or send to someone. Anything a person wants to *watch* belongs here.

## Rationale

A generated file in a temporary directory is a dead end. It is stale the moment it is written, it requires the user to know where it went and open it themselves, and it cannot show anything that changes. A repository under active development — and especially one under agentic development, where the rate of change is measured in seconds — is a moving target. A surface that cannot move with it is reporting history.

## Behavior

```text
repo-radar serve [PATH] [--bind 127.0.0.1:0] [--open] [--no-watch]
```

The server:

- Scans `PATH`, defaulting to the current directory
- Binds to the requested address, defaulting to an ephemeral port on loopback
- Prints the resolved URL to stderr
- Serves the embedded view layer of [024](024-view-layer.md)
- Streams model updates to connected clients as the repository changes
- Exits cleanly on `Ctrl-C`, closing listeners and stream connections

`--open` launches the platform browser opener against the resolved URL. `--no-watch` serves a single snapshot and streams nothing, for use where filesystem watching is unavailable or unwanted.

### Endpoints

| Path | Method | Response |
| --- | --- | --- |
| `/` | `GET` | The HTML shim for the view layer, with its style inlined |
| `/assets/*` | `GET` | The embedded wasm blob and JS glue, matched against a fixed route table, never a filesystem lookup |
| `/api/v1/report` | `GET` | The versioned JSON contract from spec 002 |
| `/api/v1/events` | `GET` | `text/event-stream` of model updates |
| anything else | `GET` | `404` with a plain-text body |
| any path | other methods | `405` with an explicit unsupported-method body |

The event stream emits a named `report` event carrying the same JSON document as `/api/v1/report`, on the debounce schedule defined by [004](004-watch-mode.md). It emits a periodic comment line as a keepalive so idle proxies and browsers do not drop the connection.

### Implementation Constraints

The **transport** is built on `std::net::TcpListener` and a bounded thread pool, with no async runtime and no HTTP framework. Server-sent events are chosen over WebSockets deliberately: SSE is plain HTTP with a documented line format, needing no handshake, no framing layer, and no upgrade negotiation. The scan engine is synchronous, and introducing `tokio` to serve a local dashboard would colour the whole crate async to solve a problem it does not have. A dependency on an HTTP framework or an async runtime requires amending this specification with a stated reason.

The **client** is the Dioxus view layer specified in [024](024-view-layer.md), compiled to `wasm32-unknown-unknown`. These are independent decisions: the server emits JSON over SSE and knows nothing about what renders it.

The client's wasm blob, JS glue, and HTML shim are embedded into the native binary with `include_bytes!` / `include_str!` and served by route from memory. There is no asset directory to locate at runtime and **no request path is ever resolved against the filesystem**, which is what makes the no-traversal property structural rather than a check.

## Safety

This surface widens the tool's exposure from a file on disk to a listening socket, so the invariants of [000](000-safety-invariants.md) apply with particular force:

- The default bind address is loopback. Binding a non-loopback address requires an explicit `--bind` value, and the tool prints a warning to stderr naming the interfaces the port will be reachable on.
- The server is read-only. No endpoint mutates the scanned repository, the server's own state, or Git.
- No route serves a path derived from a request. Asset routes match against a fixed compiled-in table and return embedded bytes; there is no static file handler and therefore no path traversal surface.
- Repository content reaching the page is escaped on insertion, and is inserted as text nodes or JSON, never as markup or as a script literal.
- The server makes no outbound network requests. It answers requests; it does not originate them.
- Nothing is logged to disk.

## Acceptance Criteria

1. `repo-radar serve` with no arguments scans the current directory, binds loopback on an ephemeral port, and prints the resolved URL to stderr.
2. The default bind address is loopback, never all interfaces. A non-loopback `--bind` emits a warning naming the exposure.
3. `/api/v1/report` returns a document byte-identical to `--format json` for the same repository state.
4. `/api/v1/events` sets `Content-Type: text/event-stream`, and a fixture change produces exactly one `report` event per debounce window.
5. `POST`, `PUT`, `DELETE`, and `PATCH` to any path return `405` and change nothing.
6. An unknown path returns `404` and no filesystem access is attempted for it.
7. `Ctrl-C` shuts the listener and all open streams down without panic, and the process exits `0`.
8. A fixture containing `<script>` in a filename renders as inert text in the page and as an escaped string in the JSON.
9. The served page issues no external network requests; a test asserts the embedded assets contain no absolute URL scheme.
10. The immutability harness of spec 000 passes across a full serve, change, stream, shutdown cycle.
11. Two concurrent clients both receive the same update stream.
12. `--no-watch` serves the report and opens no watcher.
13. `/assets/…` resolves only against a fixed compiled-in route table. A request for `/assets/../../etc/passwd` returns `404` and touches no filesystem API.
14. The binary serves the full surface with no adjacent asset files present on disk.

## Constraints

- The browser view is a consumer of the model. It contains no analysis logic and defines no field the JSON contract does not already carry. Its architecture is specified in [024 view layer](024-view-layer.md); this specification owns only how bytes reach it.
- This is a local instrument, not a service. There is no authentication, no multi-repository routing, no persistence, and no hosted deployment path — and adding any of them is a different product.
