# Feature Specification: Local Web API

Status: Optional
Priority: P2
Depends on: `002-structured-output`, `003-repository-intelligence`, `004-watch-mode`

## Goal

Expose the same local repository model to a browser UI without creating a hosted service or moving data off the machine.

## Behavior

Add an opt-in command:

```text
repo-radar serve [PATH] [--bind 127.0.0.1:0]
```

The server exposes a versioned read-only JSON endpoint for the current report and a stream for updates. It prints the selected local URL to stderr and binds to loopback by default.

## Acceptance Criteria

1. The default bind address is loopback, never all interfaces.
2. The report endpoint matches the versioned JSON contract.
3. The update stream delivers debounced watch changes.
4. The server rejects mutation methods with an explicit unsupported response.
5. The server shuts down cleanly on `Ctrl-C`.
6. Integration tests verify bind behavior, response schema, and shutdown.
7. The feature is documented as optional and local-only.

## Constraints

Do not build this phase until the library, structured output, and watch mode are stable. The browser UI is not allowed to fork the data model.