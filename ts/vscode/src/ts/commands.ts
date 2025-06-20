import * as vscode from "vscode";
import { FLAMSContext, FLAMSPreContext } from "../extension";
import { CancellationToken } from "vscode-languageclient";
import * as language from "vscode-languageclient";
import { MathHubTreeProvider } from "./mathhub";
import { Clipboard } from "vscode";
import { insertUsemodule } from "./utils";

export enum Commands {
  openFile = "flams.openFile",
}

export enum Settings {
  PreviewOn = "preview",
  SettingsToml = "settings_toml",
  FlamsPath = "flams_path",
}

export function register_commands(context: FLAMSPreContext) {
  context.vsc.subscriptions.push(
    vscode.commands.registerCommand(Commands.openFile, (arg) => {
      vscode.window.showTextDocument(arg);
    }),
  );
}

interface HtmlRequestParams {
  uri: language.URI;
}

interface QuizRequestParams {
  uri: language.URI;
}

interface StandaloneExportParams {
  uri: language.URI;
  target: string;
}

interface ReloadParams {}

interface Usemodule {
  archive: string;
  path: string;
}

export function register_server_commands(context: FLAMSContext) {
  vscode.commands.executeCommand("setContext", "flams.loaded", true);

  vscode.window.registerWebviewViewProvider(
    "flams-tools",
    webview(context, "stex-tools", (msg) => flamsTools(msg, context)),
  );

  const remote = context.remote_server
    ? "&remote=" + encodeURIComponent(context.remote_server.url)
    : "";
  vscode.window.registerWebviewViewProvider(
    "flams-search",
    webview_iframe(
      context,
      `${context.server._url}/vscode/search`,
      remote,
      (msg) => {
        if ("archive" in msg && "path" in msg) {
          if (vscode.window.activeTextEditor?.document) {
            let doc = vscode.window.activeTextEditor.document;
            return insertUsemodule(doc, msg.archive, msg.path);
          }
          vscode.window.showInformationMessage("No sTeX file in focus");
        }
        vscode.window.showErrorMessage(`Unknown message: ${msg}`);
      },
    ),
  );

  context.mathhub = new MathHubTreeProvider(context);
  vscode.window.registerTreeDataProvider("flams-mathhub", context.mathhub);

  context.vsc.subscriptions.push(
    vscode.commands.registerCommand("flams.mathhub.install", (e) =>
      context.mathhub?.install(e),
    ),
  );

  context.client.onNotification("flams/htmlResult", (s: string) => {
    openIframe(
      context.server.url + "?uri=" + encodeURIComponent(s),
      s.split("&d=")[1],
    );
  });
  context.client.onNotification("flams/updateMathHub", (_) =>
    context.mathhub?.update(),
  );
}

export function openIframe(url: string, title: string): vscode.WebviewPanel {
  const panel = vscode.window.createWebviewPanel(
    "webviewPanel",
    title,
    vscode.ViewColumn.Beside,
    {
      enableScripts: true,
      enableForms: true,
    },
  );
  panel.webview.html = `
  <!DOCTYPE html>
  <html>
    <head></head>
    <body style="padding:0;width:100vw;height:100vh;overflow:hidden;">
      <iframe style="width:100vw;height:100vh;overflow:hidden;" src="${url}" title="${title}" style="background:white"></iframe>
    </body>
  </html>`;
  return panel;
}

export function webview_iframe(
  flamscontext: FLAMSContext,
  url: string,
  query?: string,
  onMessage?: (e: any) => any,
): vscode.WebviewViewProvider {
  return <vscode.WebviewViewProvider>{
    resolveWebviewView(
      webviewView: vscode.WebviewView,
      context: vscode.WebviewViewResolveContext,
      token: CancellationToken,
    ): Thenable<void> | void {
      webviewView.webview.options = {
        enableScripts: true,
        enableForms: true,
      };
      if (onMessage) {
        webviewView.webview.onDidReceiveMessage(onMessage);
      }
      const file = vscode.Uri.joinPath(
        flamscontext.vsc.extensionUri,
        "resources",
        "iframe.html",
      );
      vscode.workspace.fs.readFile(file).then((c) => {
        let s = Buffer.from(c).toString().replace("%%URL%%", url);
        if (query) {
          s = s.replace("%%QUERY%%", query);
        }
        webviewView.webview.html = s;
      });
    },
  };
}

export function webview(
  flamscontext: FLAMSContext,
  html_file: string,
  onMessage?: (e: any) => any,
): vscode.WebviewViewProvider {
  return <vscode.WebviewViewProvider>{
    resolveWebviewView(
      webviewView: vscode.WebviewView,
      context: vscode.WebviewViewResolveContext,
      token: CancellationToken,
    ): Thenable<void> | void {
      webviewView.webview.options = {
        enableScripts: true,
        enableForms: true,
      };
      const tkuri = webviewView.webview.asWebviewUri(
        vscode.Uri.joinPath(
          flamscontext.vsc.extensionUri,
          "resources",
          "bundled.js",
        ),
      );
      const cssuri = webviewView.webview.asWebviewUri(
        vscode.Uri.joinPath(
          flamscontext.vsc.extensionUri,
          "resources",
          "codicon.css",
        ),
      );
      if (onMessage) {
        webviewView.webview.onDidReceiveMessage(onMessage);
      }
      const file = vscode.Uri.joinPath(
        flamscontext.vsc.extensionUri,
        "resources",
        html_file + ".html",
      );
      vscode.workspace.fs.readFile(file).then((c) => {
        webviewView.webview.html = Buffer.from(c)
          .toString()
          .replace(
            "%%HEAD%%",
            `<link href="${cssuri}" rel="stylesheet"/>
          <script type="module" src="${tkuri}"></script>
          <script>const vscode = acquireVsCodeApi();</script>
          `,
          );
      });
    },
  };
}

const USE_CLIPBOARD = false;

function flamsTools(msg: any, context: FLAMSContext) {
  const doc = vscode.window.activeTextEditor?.document;
  switch (msg.command) {
    case "dashboard":
      openIframe(context.server.url + "/dashboard", "Dashboard");
      break;
    case "preview":
      if (doc) {
        context.client
          .sendRequest<
            string | undefined
          >("flams/htmlRequest", <HtmlRequestParams>{ uri: doc.uri.toString() })
          .then((s) => {
            if (s) {
              openIframe(
                context.server.url + "?uri=" + encodeURIComponent(s),
                doc.fileName,
              );
            } else {
              vscode.window.showInformationMessage(
                "No preview available; building possibly failed",
              );
            }
          });
      } else {
        vscode.window.showInformationMessage("(No sTeX file in focus)");
      }
      break;
    case "standalone":
      if (doc) {
        vscode.window.showOpenDialog({
          title: "Export packaged standalone document",
          openLabel:"Select directory",
          canSelectFiles: false,
          canSelectFolders: true,
          canSelectMany: false,
        }).then((uri) => {
          const path = uri?.[0].fsPath;
          if (!path) { return; }
          context.client.sendNotification("flams/standaloneExport", 
            <StandaloneExportParams>{ 
              uri: doc.uri.toString() ,
              target:path,
            }
          );
        });
      } else {
        vscode.window.showInformationMessage("(No sTeX file in focus)");
      }
      break;
    case "quiz":
      if (doc) {
        context.client
          .sendRequest<
            string | undefined
          >("flams/quizRequest", <QuizRequestParams>{ uri: doc.uri.toString() })
          .then((s) => {
            if (s) {
              if (USE_CLIPBOARD) {
                vscode.env.clipboard.writeText(s).then(
                  () => {
                    vscode.window.showInformationMessage("Copied to clipboard");
                  },
                  (e) => {
                    vscode.window.showErrorMessage(
                      "Failed to copy to clipboard: " + e,
                    );
                  },
                );
              } else {
                vscode.window
                  .showSaveDialog({
                    title: "Save Quiz JSON",
                  })
                  .then((uri) => {
                    if (uri) {
                      vscode.workspace.fs.writeFile(uri, Buffer.from(s)).then(
                        () => {
                          vscode.window.showInformationMessage("Saved");
                        },
                        (e) => {
                          vscode.window.showErrorMessage(
                            "Failed to save: " + e,
                          );
                        },
                      );
                    }
                  });
              }
            } else {
              vscode.window.showErrorMessage(
                "No quiz available; possibly dependency missing",
              );
            }
          });
      } else {
        vscode.window.showInformationMessage("(No sTeX file in focus)");
      }
      break;
    case "reload":
      context.client.sendNotification("flams/reload", <ReloadParams>{});
      break;
    case "browser":
      if (doc) {
        context.client
          .sendRequest<
            string | undefined
          >("flams/htmlRequest", <HtmlRequestParams>{ uri: doc.uri.toString() })
          .then((s) => {
            if (s) {
              const uri = vscode.Uri.parse(context.server.url).with({
                query: "uri=" + encodeURIComponent(s),
              });
              vscode.env.openExternal(uri);
            } else {
              vscode.window.showInformationMessage(
                "No preview available; building possibly failed",
              );
            }
          });
      }
      break;
  }
}
