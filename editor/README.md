# Editor support

## VS Code

1. Install the `ruprizzle` extension, or open this workspace in VS Code.
2. The grammar in `editor/ruprizzle.tmLanguage.json` is registered for `*.ruprizzle`.

### Manual install

Copy or symlink `ruprizzle.tmLanguage.json` into your VS Code extension:

```bash
# macOS / Linux
mkdir -p ~/.vscode/extensions/ruprizzle.vscode-ruprizzle/syntaxes
cp editor/ruprizzle.tmLanguage.json ~/.vscode/extensions/ruprizzle.vscode-ruprizzle/syntaxes/
```

## JetBrains

JetBrains IDEs can load TextMate bundles via **Preferences → Editor → TextMate
Bundles**:

1. Open **Preferences → Editor → TextMate Bundles**.
2. Click the `+` and select the directory containing
   `editor/ruprizzle.tmLanguage.json`.
3. Restart the IDE.

## What's covered

- Comments (`//` and `/* */`)
- Strings, numbers, and booleans
- Block keywords (`datasource`, `generator`, `model`, `enum`)
- Scalar types (`String`, `Int`, `Uuid`, …)
- Field attributes (`@id`, `@default`, `@unique`, `@relation`, `@map`)
- Model attributes (`@@map`, `@@unique`, `@@index`, `@@id`)
- Referential actions (`Cascade`, `Restrict`, `SetNull`, `SetDefault`, `NoAction`)

## LSP

A full language server (completion, diagnostics, go-to-definition) is planned
for the 0.2 release. This repository already exposes spans and diagnostics from
the parser, so the LSP is intentionally cheap to add later.
