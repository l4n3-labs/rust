# json-sort-server

An LSP server for sorting JSON and JSONC files. Provides up to 27 code actions accessible from any editor that supports the Language Server Protocol.

## Code Actions

Actions are organised into three scopes:

- **Deep Sort** (9 actions) — sorts the entire document recursively
- **Shallow Sort** (9 actions) — sorts only the top-level keys of the root object
- **Subtree Sort** (9 actions, contextual) — sorts the object or array under the cursor and all its descendants; only offered when the cursor is inside a nested container

Each scope offers 9 sort strategies: ascending, descending, randomize, by value, by key length, by value length, by value type, sort list items, and sort both objects and arrays.

## Running

The server communicates over stdin/stdout using JSON-RPC:

```bash
json-sort-server
```

Connect it to your editor's LSP client for `json` and `jsonc` file types.

## Configuration

The server accepts `initializationOptions` to control which scopes and actions are offered. All scopes and actions are enabled by default.

```jsonc
{
  // Toggle entire scopes
  "scopes": {
    "deep": true,
    "shallow": true,
    "subtree": false  // disable subtree actions
  },
  // Toggle individual actions globally
  "actions": {
    "ascending": true,
    "descending": true,
    "randomize": false  // hide randomize from all scopes
  }
}
```

Scopes also accept per-scope action overrides:

```jsonc
{
  "scopes": {
    "deep": {
      "ascending": true,
      "descending": true
      // unspecified actions default to true
    }
  }
}
```

See [`settings-schema.json`](settings-schema.json) for the full JSON Schema.

## Installation

Download a prebuilt binary from the [GitHub Releases](https://github.com/l4n3-labs/rust/releases) page, or build from source:

```bash
cargo build -p json-sort-server --release
```

Prebuilt binaries are available for:
- macOS (aarch64, x86_64)
- Linux (aarch64, x86_64)
- Windows (x86_64)

## License

Apache-2.0
