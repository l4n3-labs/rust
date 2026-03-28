# JSON Sort (Zed Extension)

Sort JSON and JSONC files directly from Zed using code actions. Powered by `json-sort-server`.

## Background

I migrated from VS Code to Neovim, then landed on Zed — it's been significantly more productive to use day-to-day and easier to maintain over time. One feature I relied on heavily in VS Code was sorting JSON files directly in the editor. Zed doesn't have this built in, and it's been requested a few times:

- [zed-industries/zed#48465](https://github.com/zed-industries/zed/issues/48465)
- [zed-industries/zed#16746](https://github.com/zed-industries/zed/issues/16746)

This extension fills that gap with a custom LSP server (`json-sort-server`) that handles sorting via code actions. It goes beyond what `json-language-server` offers — 9 sort strategies, 3 sort scopes (deep, shallow, subtree), and full JSONC support with comment preservation.

My primary languages are TypeScript, Python, and Scala — this project is how I'm learning Rust. I've used AI to help work through some problems, but I've extensively manually tested everything and have enough technical background to stand behind the code.

## Installation

### From the Zed Extensions panel

1. Open **Zed → Extensions** (or press `Cmd+Shift+X`).
2. Search for **JSON Sort**.
3. Click **Install**.

The extension will automatically download the `json-sort-server` binary for your platform on first use.

### Manual installation

If you prefer to install the LSP server yourself:

```sh
cargo install --path crates/json-sort-server
```

The extension checks your `PATH` for `json-sort-server` before attempting a download.

## Usage

1. Open a `.json` or `.jsonc` file in Zed.
2. Open the code actions menu (`Cmd+.`).
3. Select a sort action from the list.

## Sort Scopes

Actions are grouped into three scopes:

- **Deep Sort** — sorts the entire document recursively (all nested objects and arrays).
- **Shallow Sort** — sorts only the immediate top-level keys of the root object; nested structures are left untouched.
- **Subtree Sort** — sorts the object or array under the cursor and all of its descendants. Only appears when the cursor is inside a nested (non-root) container.

## Available Sort Actions

Each scope offers the same 9 sort strategies:

| Strategy | Description |
|---|---|
| Ascending | Sort object keys A→Z |
| Descending | Sort object keys Z→A |
| Randomize | Shuffle object keys randomly |
| By Value | Sort object entries by their values |
| By Key Length | Sort object keys by string length |
| By Value Length | Sort entries by the length/size of values |
| By Value Type | Group entries by JSON type (bool, null, number, string, array, object) |
| Sort List Items | Sort array elements |
| Sort All (Objects + Lists) | Sort both object keys and array elements |

For example, opening the code actions menu while inside a nested object will show actions like `Deep Sort: Ascending`, `Shallow Sort: Ascending`, and `Subtree Sort: Ascending`.

## Configuration

Add LSP settings in your Zed `settings.json` (open with `Cmd+,`):

```json
{
  "lsp": {
    "json-sort-server": {
      "initialization_options": {}
    }
  }
}
```

The `initialization_options` object is passed through to the LSP server on startup.

## Supported Languages

- JSON
- JSONC (JSON with Comments)

Both are configured in the extension manifest. JSONC features like line comments (`//`), block comments (`/* */`), and trailing commas are preserved during sorting.
