# craft-mcp

The MCP plumbing of the Craft apps, shared so a second app is agent-native in a day instead of a
sprint.

The apps are agent-native: each exposes what it can do as **commands** (id, label, parameter
description, enabled state), and an agent drives it through a [Model
Context Protocol](https://modelcontextprotocol.io) server on stdio. This crate holds the part that
is the same in every app:

| module | what it is |
|---|---|
| `jsonrpc` | Newline-delimited JSON-RPC 2.0 framing, the MCP revisions this crate speaks, the bounded line reader. |
| `server` | `Server<H>` over stdio in front of a `Handler` the app implements (its tools, resources, prompts and the calls that run them). |
| `script` | `run --script job.json`: a JSON array or JSON Lines of steps, and the `"$N.path"` references between them. |
| `automation` | The `--automation-read-root` / `--automation-write-root` flags and the containment check they promise. |
| `remote` | The bridge to a running app over its loopback JSON control channel (`mcp --connect PORT`). |

The app still owns its commands, document model, renders and control channel. This crate never
draws anything and keeps no state beyond one server's connection.

## Adopting it in an app

1. Vendor this crate at `libs/craft-libs/crates/mcp` (see the suite's `local-libs` protocol) and
   add it to the root `Cargo.toml`:
   ```toml
   craft-mcp = { path = "libs/craft-libs/crates/mcp/" }
   ```
2. Implement `Handler` over what the app already has. `call_tool` is the only required method;
   the tool schemas are the app's own (`tools()` returns them as MCP wants them).
3. Serve it from the app's CLI:
   ```rust
   let mut server = craft_mcp::Server::new(my_handler);
   server.serve(std::io::stdin().lock(), std::io::stdout().lock())?;
   ```
4. Take the same flags as the rest of the suite:
   ```rust
   let (roots, rest) = craft_mcp::automation::RootFlags::take(&args)?;
   ```
5. Read a batch or a job with `script::parse_commands` and substitute `"$N"` with `script::resolve`.
6. Document the server in the app's `docs/mcp.md` and point an MCP client at it; the same client
   configuration shape works for every app of the suite.

`Local Vector` (`local-vector`) is the reference implementation: its `crates/mcp` shows the full
surface (910 commands, resources, prompts, completion, progress and cancellation) on top of the
same ideas. Its engine already enforces the automation roots process-wide, so it uses
`RootFlags` only to read the command line the same way; an app without that uses `Roots` too.

## Flags (the suite contract)

```text
--automation-read-root DIR   (repeatable; --flag=DIR also works)
--automation-write-root DIR
```

- A path inside any of its kind's folders is allowed; `--root` itself counts as inside.
- A kind left out grants none of its access: read roots grant no writes, write roots grant no reads.
- The folders must exist when given; links are followed; a Windows device name (`NUL`) or an
  alternate data stream (`x.svg:evil`) is refused.
- `mcp --connect PORT` takes a bare port (loopback), `HOST:PORT`, or `localhost:PORT`; a host that
  isn't loopback is refused.
- A loopback port is **not authentication**. An app that needs a key adds one to its own control
  protocol; this crate doesn't pretend a local port is private.

## Deliberate limits

- The stdio server is single-threaded: one request at a time, answered before the next line is
  read. No cancellation, no progress notifications, no "busy" replies. An app that needs those
  grows them on top (Local Vector's own server is the worked example); a new app should not need
  them on day one.
- `script::resolve` binds at most 256 MiB of copied values per step and walks at most 64 levels
  deep; `parse_commands` takes at most 100 000 steps and `read_source` 64 MiB of text. Scripts are
  untrusted input.

## Provenance and licence

Adapted from **`storytold/vectorcraft`** (`crates/mcp`, `docs/mcp.md`, `docs/control-protocol.md`)
and **`storytold/wordcraft`** (the `mcp --connect` bridge and `run --script`), both
`MIT OR Apache-2.0`. What changed: the app-specific tools, resources, prompts, cancellation and
progress were left behind; the protocol framing, the automation-root containment and the step
references are theirs. `script::resolve` and its tests are Local Vector's
`crates/engine/src/steps.rs`, which is itself vectorcraft's, moved here so every app shares one
implementation.

This crate is `MIT OR Apache-2.0`, as the rest of the suite. The vendored copy is the mirror; the
source of truth is the suite's `local-libs` repository (`crates/mcp`), replicated by `sync.ps1`.
