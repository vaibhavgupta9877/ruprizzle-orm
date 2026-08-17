import * as path from "path";
import * as vscode from "vscode";
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const config = vscode.workspace.getConfiguration("ruprizzle");
  const serverPath = config.get<string>("languageServer.path") ?? "ruprizzle";

  const run: Executable = {
    command: serverPath,
    args: ["lsp", "--stdio"],
    transport: TransportKind.stdio,
    options: {
      env: {
        ...process.env,
      },
    },
  };

  const serverOptions: ServerOptions = {
    run,
    debug: run,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "ruprizzle" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.ruprizzle"),
    },
  };

  client = new LanguageClient(
    "ruprizzle",
    "ruprizzle Language Server",
    serverOptions,
    clientOptions
  );

  context.subscriptions.push(client.start());
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
