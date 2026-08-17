# Editor support

## VS Code

1. Install the `ruprizzle` extension, or open this workspace in VS Code.
2. The grammar in `editor/ruprizzle.tmLanguage.json` is registered for `*.ruprizzle`.
3. If the `ruprizzle` CLI is on your `$PATH`, the extension starts `ruprizzle lsp --stdio` for
   diagnostics, completion, go-to-definition and hover.

### Manual install

Copy or symlink the TextMate grammar into your VS Code extension directory:

```bash
# macOS / Linux
mkdir -p ~/.vscode/extensions/ruprizzle.vscode-ruprizzle/syntaxes
cp editor/ruprizzle.tmLanguage.json ~/.vscode/extensions/ruprizzle.vscode-ruprizzle/syntaxes/
```

For the full extension, build the TypeScript in `editor/vscode`:

```bash
cd editor/vscode
npm install
npm run compile
```

## JetBrains

JetBrains IDEs can load TextMate bundles via **Preferences → Editor → TextMate
Bundles**:

1. Open **Preferences → Editor → TextMate Bundles**.
2. Click the `+` and select the directory containing
   `editor/ruprizzle.tmLanguage.json`.
3. Restart the IDE.

## Other editors

Any LSP client can connect to `ruprizzle lsp --stdio`. The server reads the
`schema.ruprizzle` file over the LSP text document sync and publishes
diagnostics as you type.

## What's covered

- Comments (`//` and `/* */`)
- Strings, numbers, and booleans
- Block keywords (`datasource`, `generator`, `model`, `enum`)
- Scalar types (`String`, `Int`, `Uuid`, …)
- Field attributes (`@id`, `@default`, `@unique`, `@relation`, `@map`, `@updatedAt`)
- Model attributes (`@@map`, `@@unique`, `@@index`, `@@id`)
- Referential actions (`Cascade`, `Restrict`, `SetNull`, `SetDefault`, `NoAction`)
- LSP diagnostics, completion, go-to-definition and hover
