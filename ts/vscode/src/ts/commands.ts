import * as vscode from "vscode";
import { FLAMSContext, FLAMSPreContext, getContext } from "../extension";
import { CancellationToken } from "vscode-languageclient";
import * as language from "vscode-languageclient";
import { MathHubTreeProvider } from "./mathhub";
import { Clipboard } from "vscode";
import { insertUsemodule } from "./utils";
import {
    FlamsCallHierarchyTreeProvider,
    FlamsDocumentSymbolTreeProvider,
} from "./callgraph";

export enum Commands {
    openFile = "flams.openFile",
    installArchive = "flams.mathhub.install",
    buildOne = "flams.buildOne",
    buildAll = "flams.buildAll",
    exportTex = "flams.exportTex",
    exportHtml = "flams.exportHtml",
}

export enum Lsp {}

export enum Settings {
    PreviewOn = "preview",
    SettingsToml = "settings_toml",
    FlamsPath = "flams_path",
    RemoteFlams = "remote_flams",
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

interface NewArchiveParams {
    archive: string;
    urlbase: string;
}

interface BuildFileParams {
    uri: language.URI;
}

interface Usemodule {
    kind: "usemodule";
    archive: string;
    path: string;
}

interface PreviewLocal {
    kind: "preview";
    uri: string;
}

class PreviewPanel {
    panel: [string, vscode.WebviewPanel][];
    constructor() {
        this.panel = [];
    }
    open(url: string, title: string) {
        let old = this.panel.find(([u, _]) => u === url);
        if (old) {
            const column = old[1].viewColumn
                ? old[1].viewColumn
                : vscode.ViewColumn.Beside;
            old[1].reveal(column, true);
            old[1].webview.html = "";
            old[1].webview.html = iframeHtml(url, title);
        } else {
            const new_panel = openIframe(url, title, false);
            this.panel.push([url, new_panel]);
            new_panel.onDidDispose(() => {
                this.panel = this.panel.filter(([u, _]) => u !== url);
            });
        }
    }
}

export const PREVIEW: PreviewPanel = new PreviewPanel();

class Dashboard {
    panel: vscode.WebviewPanel | undefined;
    show(context: FLAMSContext, url: string) {
        if (this.panel) {
            const column = this.panel.viewColumn
                ? this.panel.viewColumn
                : vscode.ViewColumn.Beside;
            this.panel.reveal(column, false);
            this.panel.webview.html = "";
            this.panel.webview.html = iframeHtml(
                context.server.url + "/dashboard/" + url,
                "Dashboard",
            );
        } else {
            const new_panel = openIframe(
                context.server.url + "/dashboard/" + url,
                "Dashboard",
                true,
            );
            this.panel = new_panel;
            new_panel.onDidDispose(() => {
                this.panel = undefined;
            });
        }
    }
}

export const DASHBOARD = new Dashboard();

function new_archive(context: FLAMSContext) {
    vscode.window
        .showInputBox({
            prompt: "Insert the name of a new math archive here.",
            password: false,
            title: "New Math Archive",
            placeHolder: "My/Archive/Name",
            validateInput(value) {
                return undefined; // TODO
            },
        })
        .then((value) => {
            if (value !== undefined) {
                let archive = value;
                vscode.window
                    .showInputBox({
                        prompt: "Insert the URL base of the archive (where you plan to host it):",
                        password: false,
                        title: "New Math Archive",
                        value: "http://mathhub.info",
                        validateInput(value) {
                            return undefined; // TODO
                        },
                    })
                    .then((value) => {
                        if (value !== undefined) {
                            let urlbase = value;
                            context.client.sendNotification(
                                "flams/newArchive",
                                <NewArchiveParams>{
                                    archive,
                                    urlbase,
                                },
                            );
                        }
                    });
            }
        });
}

class DocumentView {
    view: vscode.Webview | undefined;
    html: string = "";
    constructor(flamscontext: FLAMSContext) {
        const self = this;
        const provider = <vscode.WebviewViewProvider>{
            resolveWebviewView(
                webviewView: vscode.WebviewView,
                context: vscode.WebviewViewResolveContext,
                token: CancellationToken,
            ): Thenable<void> | void {
                webviewView.webview.options = {
                    enableScripts: true,
                    enableForms: true,
                };
                self.view = webviewView.webview;
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
                        "node_modules",
                        "@vscode/codicons",
                        "dist",
                        "codicon.css",
                    ),
                );
                webviewView.webview.onDidReceiveMessage((msg) =>
                    flamsTools(msg, flamscontext),
                );
                const file = vscode.Uri.joinPath(
                    flamscontext.vsc.extensionUri,
                    "resources",
                    "document.html",
                );
                const url = vscode.window.activeTextEditor?.document
                    ? vscode.window.activeTextEditor.document.uri.toString()
                    : "";

                vscode.workspace.fs.readFile(file).then((c) => {
                    const prehtml = Buffer.from(c)
                        .toString()
                        .replace(
                            "%%HEAD%%",
                            `<link href="${cssuri}" rel="stylesheet"/>
            <script type="module" src="${tkuri}"></script>
            <script>const vscode = acquireVsCodeApi();</script>
            `,
                        )
                        .replace("%%SERVER_URL%%", flamscontext.server.url);
                    self.html = prehtml;
                    webviewView.webview.html = prehtml.replace(
                        "%%DOC_URL%%",
                        url,
                    );
                });
            },
        };
        vscode.window.registerWebviewViewProvider("flams-document", provider);
    }
    refresh() {
        if (this.view && this.html) {
            this.view.html = "";
            const url = vscode.window.activeTextEditor?.document
                ? vscode.window.activeTextEditor.document.uri.toString()
                : "";
            this.view.html = this.html.replace("%%DOC_URL%%", url);
        }
    }
}

export var DOCUMENT_VIEW: DocumentView | undefined = undefined;

export function register_server_commands(context: FLAMSContext) {
    vscode.commands.executeCommand("setContext", "flams.loaded", true);

    vscode.window.registerWebviewViewProvider(
        "flams-tools",
        webview(context, "stex-tools", undefined, (msg) =>
            flamsTools(msg, context),
        ),
    );

    /*
  const imports = new FlamsDocumentSymbolTreeProvider();//FlamsCallHierarchyTreeProvider();
  context.vsc.subscriptions.push(
    vscode.window.createTreeView("flams.callgraph",{treeDataProvider: imports})
  );
  context.vsc.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(() => {
      imports.refresh();
    })
  );
  context.vsc.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument(e => {
      const editor = vscode.window.activeTextEditor;
      if (editor && e.document === editor.document) {
        imports.refresh();
      }
    })
  );
  */

    DOCUMENT_VIEW = new DocumentView(context);

    context.vsc.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor(() => {
            DOCUMENT_VIEW?.refresh();
        }),
    );
    context.vsc.subscriptions.push(
        vscode.commands.registerCommand(
            "snify/annotate",
            async (...args: any[]) => {
                await context.client.sendRequest(
                    language.ExecuteCommandRequest.type,
                    {
                        command: "snify/annotate",
                        arguments: args,
                    },
                );
            },
        ),
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
                if ("kind" in msg && msg.kind === "usemodule") {
                    const usem = <Usemodule>msg;
                    if (vscode.window.activeTextEditor?.document) {
                        const doc = vscode.window.activeTextEditor.document;
                        return insertUsemodule(doc, usem.archive, usem.path);
                    }
                    return vscode.window.showInformationMessage(
                        "No sTeX file in focus",
                    );
                }
                if ("kind" in msg && msg.kind === "preview") {
                    const pv = <PreviewLocal>msg;
                    const url =
                        context.server.url +
                        "/document?uri=" +
                        encodeURIComponent(pv.uri);
                    PREVIEW.open(url, pv.uri.split("&d=")[1]);
                }
                vscode.window.showErrorMessage(`Unknown message: ${msg}`);
            },
        ),
    );

    context.mathhub = new MathHubTreeProvider(context);
    vscode.window.registerTreeDataProvider("flams-mathhub", context.mathhub);

    context.vsc.subscriptions.push(
        vscode.commands.registerCommand(Commands.installArchive, (e) =>
            context.mathhub?.install(e),
        ),
    );

    context.vsc.subscriptions.push(
        vscode.commands.registerCommand(Commands.exportTex, (arg: vscode.Uri) =>
            exportTex(arg),
        ),
    );
    context.vsc.subscriptions.push(
        vscode.commands.registerCommand(
            Commands.exportHtml,
            (arg: vscode.Uri) => exportHtml(arg),
        ),
    );
    context.vsc.subscriptions.push(
        vscode.commands.registerCommand(Commands.buildOne, (arg: vscode.Uri) =>
            context.client
                ?.sendRequest("flams/buildOne", <BuildFileParams>{
                    uri: arg.toString(),
                })
                .then((_) => DASHBOARD.show(context, "queue")),
        ),
    );
    context.vsc.subscriptions.push(
        vscode.commands.registerCommand(Commands.buildAll, (arg: vscode.Uri) =>
            context.client
                ?.sendRequest("flams/buildAll", <BuildFileParams>{
                    uri: arg.toString(),
                })
                .then((_) => DASHBOARD.show(context, "queue")),
        ),
    );

    context.client.onNotification("flams/htmlResult", (s: { url: string }) => {
        DOCUMENT_VIEW?.refresh();
        PREVIEW.open(
            context.server.url + "?uri=" + encodeURIComponent(s.url),
            s.url.split("&d=")[1],
        );
    });
    context.client.onNotification("flams/updateMathHub", (_) =>
        context.mathhub?.update(),
    );
    context.client.onNotification("flams/openFile", (v: { uri: string }) => {
        const uri = vscode.Uri.parse(v.uri);
        vscode.window.showTextDocument(uri);
    });
}

export function exportTex(doc: vscode.Uri) {
    const context = getContext();
    vscode.window
        .showOpenDialog({
            title: "Export packaged standalone tex",
            openLabel: "Select directory",
            canSelectFiles: false,
            canSelectFolders: true,
            canSelectMany: false,
        })
        .then((uri) => {
            const path = uri?.[0].fsPath;
            if (!path) {
                return;
            }
            context.client.sendNotification("flams/standaloneExport", <
                StandaloneExportParams
            >{
                uri: doc.toString(),
                target: path,
            });
        });
}

export function exportHtml(doc: vscode.Uri) {
    const context = getContext();
    vscode.window
        .showOpenDialog({
            title: "Export packaged standalone HTML",
            openLabel: "Select directory",
            canSelectFiles: false,
            canSelectFolders: true,
            canSelectMany: false,
        })
        .then((uri) => {
            const path = uri?.[0].fsPath;
            if (!path) {
                return;
            }
            context.client.sendNotification("flams/htmlExport", <
                StandaloneExportParams
            >{
                uri: doc.toString(),
                target: path,
            });
        });
}

export function openIframe(
    url: string,
    title: string,
    focus: boolean,
): vscode.WebviewPanel {
    const panel = vscode.window.createWebviewPanel(
        "webviewPanel",
        title,
        {
            viewColumn: vscode.ViewColumn.Beside,
            preserveFocus: !focus,
        },
        {
            enableScripts: true,
            enableForms: true,
        },
    );
    panel.webview.html = iframeHtml(url, title);
    return panel;
}

function iframeHtml(url: string, title: string): string {
    return `
  <!DOCTYPE html>
  <html>
    <head></head>
    <body style="padding:0;width:100vw;height:100vh;overflow:hidden;">
      <iframe style="width:100vw;height:100vh;overflow:hidden;" src="${url}" title="${title}" style="background:white" id="miframe"></iframe>
      <script>
        var _theframe = document.getElementById("miframe");
        _theframe.contentWindow.location.href = _theframe.src;
      </script>
    </body>
  </html>`;
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
    then?: (s: string) => string,
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
                /*vscode.Uri.joinPath(
          flamscontext.vsc.extensionUri,
          "resources",
          "codicon.css",
        ),*/
                vscode.Uri.joinPath(
                    flamscontext.vsc.extensionUri,
                    "node_modules",
                    "@vscode/codicons",
                    "dist",
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
                    )
                    .replace("%%SERVER_URL%%", flamscontext.server.url);
            });
        },
    };
}

const USE_CLIPBOARD = false;

function flamsTools(msg: any, context: FLAMSContext) {
    const doc = vscode.window.activeTextEditor?.document;
    switch (msg.command) {
        case "dashboard":
            DASHBOARD.show(context, "");
            break;
        case "newarchive":
            new_archive(context);
            break;
        case "preview":
            if (doc) {
                context.client
                    .sendRequest<
                        string | undefined
                    >("flams/htmlRequest", <HtmlRequestParams>{ uri: doc.uri.toString() })
                    .then((s) => {
                        if (s) {
                            PREVIEW.open(
                                context.server.url +
                                    "?uri=" +
                                    encodeURIComponent(s),
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
                exportTex(doc.uri);
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
                                        vscode.window.showInformationMessage(
                                            "Copied to clipboard",
                                        );
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
                                            vscode.workspace.fs
                                                .writeFile(uri, Buffer.from(s))
                                                .then(
                                                    () => {
                                                        vscode.window.showInformationMessage(
                                                            "Saved",
                                                        );
                                                    },
                                                    (e) => {
                                                        vscode.window.showErrorMessage(
                                                            "Failed to save: " +
                                                                e,
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
                            const uri = vscode.Uri.parse(
                                context.server.url,
                            ).with({
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
        case "snify":
            if (doc) {
                context.client.sendNotification("flams/snify", <
                    HtmlRequestParams
                >{ uri: doc.uri.toString() });
            }
            break;
    }
}
