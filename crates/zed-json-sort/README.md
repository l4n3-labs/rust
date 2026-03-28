# JSON Sort (Zed Extension)

Sort JSON and JSONC files in Zed using code actions. Powered by `json-sort-server`.

## Background

I migrated from VS Code to Neovim, then landed on Zed — it's been way more productive day to day and easier to maintain over time. One thing I relied on in VS Code was sorting JSON files directly in the editor. Zed doesn't have this, and it's come up a few times:

- [zed-industries/zed#48465](https://github.com/zed-industries/zed/issues/48465)
- [zed-industries/zed#16746](https://github.com/zed-industries/zed/issues/16746)

This extension fills that gap with a custom LSP server (`json-sort-server`) that handles sorting via code actions. It does more than `json-language-server` — 9 sort strategies, 3 sort scopes (deep, shallow, subtree), and full JSONC support with comment preservation.

My primary languages are TypeScript, Python, and Scala — this project is how I'm learning Rust. I've used AI to help work through some problems, but I've manually tested everything extensively and have enough technical background to stand behind the code.

## Installation

### From the Zed Extensions panel

1. Open **Zed → Extensions** (or press `Cmd+Shift+X`).
2. Search for **JSON Sort**.
3. Click **Install**.

The extension automatically downloads the `json-sort-server` binary for your platform on first use.

### Manual installation

If you'd rather install the LSP server yourself:

```sh
cargo install --path crates/json-sort-server
```

The extension checks your `PATH` for `json-sort-server` before trying to download anything.

## Usage

1. Open a `.json` or `.jsonc` file in Zed.
2. Open the code actions menu (`Cmd+.`).
3. Pick a sort action.

## Sort scopes

Actions are grouped into three scopes:

- **Deep** — sorts the entire document recursively, all nested objects and arrays.
- **Shallow** — sorts only the top-level keys of the root object. Nested structures stay untouched.
- **Subtree** — sorts the object or array under the cursor and everything inside it. Only shows up when the cursor is inside a nested (non-root) container.

## Available sort actions

Each scope has the same 9 strategies:

| Strategy | What it does |
|---|---|
| Ascending | Sort keys A→Z |
| Descending | Sort keys Z→A |
| Randomize | Shuffle keys randomly |
| By Value | Sort entries by their values |
| By Key Length | Sort keys by string length |
| By Value Length | Sort entries by the length/size of values |
| By Value Type | Group entries by JSON type (bool, null, number, string, array, object) |
| Sort List Items | Sort array elements |
| Sort All (Objects + Lists) | Sort both keys and array elements |

So if your cursor is inside a nested object, the code actions menu shows things like `Deep Sort: Ascending`, `Shallow Sort: Ascending`, and `Subtree Sort: Ascending`.

## Configuration

Add LSP settings in your Zed `settings.json` (open with `Cmd+,`). All scopes and actions are enabled by default — you only need to configure what you want to disable.

A [JSON Schema](../json-sort-server/settings-schema.json) is available for the settings object.

```json
{
  "lsp": {
    "json-sort-server": {
      "initialization_options": {
        "scopes": {
          "deep": true,
          "shallow": false,
          "subtree": { "ascending": true, "randomize": false }
        },
        "actions": {
          "randomize": false,
          "by_value_type": false
        }
      }
    }
  }
}
```

### Scopes

Each scope (`deep`, `shallow`, `subtree`) accepts:

| Value | Meaning |
|---|---|
| `true` | Enabled — uses the global `actions` settings (default) |
| `false` | Disabled — no code actions for this scope |
| `{ ... }` | Enabled with per-scope action overrides, independent of global `actions` |

### Actions

Toggle individual sort strategies globally or per-scope. All default to `true`.

| Key | Strategy |
|---|---|
| `ascending` | Sort keys A→Z |
| `descending` | Sort keys Z→A |
| `randomize` | Shuffle keys randomly |
| `by_value` | Sort entries by their values |
| `by_key_length` | Sort keys by string length |
| `by_value_length` | Sort entries by the length/size of values |
| `by_value_type` | Group entries by JSON type |
| `sort_list_items` | Sort array elements |
| `sort_all` | Sort both keys and array elements |

Global `actions` apply to any scope set to `true`. A scope set to an object uses its own action toggles and ignores the global `actions`.

## Supported languages

- JSON
- JSONC (JSON with Comments)

Both are configured in the extension manifest. JSONC features like line comments (`//`), block comments (`/* */`), and trailing commas are preserved during sorting.
