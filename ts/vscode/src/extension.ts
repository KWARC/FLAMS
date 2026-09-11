import * as vscode from "vscode";
//import { activate as act, greet } from '../pkg/flams_vscode';
import {
  Commands,
  register_commands,
  register_server_commands,
  Settings,
} from "./ts/commands";
import { setup } from "./ts/setup";
import { Versions } from "./ts/versions";
import * as language from "vscode-languageclient/node";
//import { FLAMSServer } from "@kwarc/flams";
//import * as FLAMS from "@flexiformal/ftml-backend";
import { FLAMSServer} from "./ts/flams";
import { getSettings, MathHubTreeProvider } from "./ts/mathhub";
import path from "path";
import * as fs from "fs";
//import * as ws from 'ws';

export async function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    vscode.window.registerUriHandler(new FlamsUriHandler()),
  );
  await local(context);
  //await remote(context);
}

// Our implementation of a UriHandler.
class FlamsUriHandler implements vscode.UriHandler {
  // This function will get run when something redirects to VS Code
  // with your extension id as the authority.
  handleUri(uri: vscode.Uri): vscode.ProviderResult<void> {
    return handleFlamsUri(uri);
  }
}

async function handleFlamsUri(uri: vscode.Uri) {
  // "vscode://flams/open?a={}&rp={}"
  if (uri.path !== "/open") {
    vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Unknown URI path ${uri.path}`);
    return;
  }
  const query = decodeURIComponent(uri.query);
  const args = query.split("&");
  if (args.length < 2) {
    vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Invalid number of query parameters`);
    return;
  }
  const a0 = args[0];
  if (a0.split("=")[0] !== "a") {
    vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Missing query parameter a`);
    return;
  }
  const a = args[0].split("=")[1];
  const rp0 = args[1];
  if (rp0.split("=")[0] !== "rp") {
    vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Missing query parameter rp`);
    return;
  }
  const rp = args[1].split("=")[1];
  const mathhubs = (await getSettings())?.mathhubs;
  if (!mathhubs) {
    vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: No mathhubs`);
    return;
  }
  const mh = mathhubs.find((mh) =>
    // check if mh/a exists
    fs.existsSync(a.split("/").reduce((p, seg) => path.join(p, seg), mh)),
  );
  if (!mh) {
    vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Archive ${a} not found`);
    return;
  }
  const p1 = a.split("/").reduce((p, seg) => path.join(p, seg), mh);
  const p2 = path.join(p1, "source");
  const p3 = rp.split("/").reduce((p, seg) => path.join(p, seg), p2);

  vscode.window.showTextDocument(vscode.Uri.file(p3));
}

async function local(context: vscode.ExtensionContext) {
  const ctx = new FLAMSPreContext(context);
  register_commands(ctx);
  if (await ctx.versions.isValid()) {
    launch_local(ctx);
  } else {
    setup(ctx);
  }
}

export function deactivate() {
  const context = getPreContext();
  if (context?.client) {
    context.client.stop();
  }
}

export async function launch_local(context: FLAMSPreContext) {
  let versions = context.versions;
  let flams = await versions?.flamsVersion();
  let stex = await versions?.stexVersion();
  let flams_path = versions?.flams_path;
  let stex_path = versions?.stex_path;
  if (!(stex && flams && flams_path && stex_path)) {
    vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Error initializing`, {
      modal: true,
    });
    return;
  }
  context.outputChannel.appendLine(
    `Using ${flams_path} (version ${flams.toString()})`,
  );
  context.outputChannel.appendLine(
    `sTeX at ${stex_path} (version ${stex.toString()})`,
  );
  context.outputChannel.appendLine("Initializing 𝖥𝖫∀𝖬∫ LSP Server");

  const toml = vscode.workspace
    .getConfiguration("flams")
    .get<string>(Settings.SettingsToml);
  const args = toml ? ["--lsp", "-c", toml] : ["--lsp"];
  const serverOptions: language.ServerOptions = {
    run: { command: flams_path, args: args },
    debug: { command: flams_path, args: args },
  };
  context.client = new language.LanguageClient(
    "flams",
    //"𝖥𝖫∀𝖬∫ Language Server",
    serverOptions,
    {
      documentSelector: [
        { scheme: "file", language: "tex" },
        { scheme: "file", language: "latex" },
      ],
      synchronize: {},
      traceOutputChannel: context.outputChannel,
      markdown: {
        isTrusted: true,
        supportHtml: true,
      },
    },
  );
  context.client.onNotification("flams/serverURL", (s: {url:string}) => {
    context.server = new FLAMSServer(s.url);
    const ctx = new FLAMSContext(context);
    register_server_commands(ctx);
  });

  // Setting "flams.trace.server":"verbose"

  context.client.start();
}

/*
async function remote(context: vscode.ExtensionContext) {
	const ctx = new FLAMSPreContext(context);
	register_commands(ctx);
  launch_remote(ctx);
}

export async function launch_remote(context: FLAMSPreContext) {
  // Initialize server first
  context.server = new FLAMSServer("http://localhost:3000");

  const wsock = new ws.WebSocket("http://localhost:3000/ws/lsp");
  const connection = ws.WebSocket.createWebSocketStream(wsock);

  connection.on("data",(chunk) => console.log(new TextDecoder().decode(chunk)));

  context.client = new language.LanguageClient(
    "flams-server",
    "𝖥𝖫∀𝖬∫ Language Server",
    () => Promise.resolve({
      reader: connection,
      writer: connection,
    }),
    {
        documentSelector: [
            {scheme: "file", language: "tex"},
            {scheme: "file", language: "latex"}
        ],
        synchronize: {}
    }
  );
  await context.client.start().then(() => {
    const ctx = new FLAMSContext(context);
    ctx.remote_server = undefined;
    register_server_commands(ctx);
  });
}
*/

let _context: FLAMSContext | FLAMSPreContext | undefined = undefined;
export function getPreContext(): FLAMSContext | FLAMSPreContext | undefined {
  return _context;
}

export function getContext(): FLAMSContext {
  if (!(_context instanceof FLAMSContext)) {
    throw new Error("context is undefined");
  }
  return _context;
}

async function wait(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 100));
}

export async function awaitContext(): Promise<FLAMSContext> {
  let waited = 0;
  while (!(_context instanceof FLAMSContext)) {
    waited += 1;
    if (waited > 10) {
      throw new Error("context is undefined");
    }
    await wait();
  }
  return Promise.resolve(_context);
}

export class FLAMSPreContext {
  outputChannel: vscode.LogOutputChannel;
  vsc: vscode.ExtensionContext;
  versions: Versions = new Versions();
  client: language.LanguageClient | undefined;
  server: FLAMSServer | undefined;

  constructor(context: vscode.ExtensionContext) {
    this.vsc = context;
    this.outputChannel = vscode.window.createOutputChannel("FLAMS",{log:true});
    _context = this;
  }

  register_command(
    name: string,
    callback: (...args: any[]) => any,
    thisArg?: any,
  ) {
    const disposable = vscode.commands.registerCommand(name, callback, thisArg);
    this.vsc.subscriptions.push(disposable);
  }
}

export class FLAMSContext {
  outputChannel: vscode.OutputChannel;
  vsc: vscode.ExtensionContext;
  versions: Versions;
  client: language.LanguageClient;
  server: FLAMSServer;
  remote_server: FLAMSServer | undefined;
  mathhub: MathHubTreeProvider | undefined;

  constructor(ctx: FLAMSPreContext) {
    if (!ctx.client || !ctx.server) {
      throw new Error("𝖥𝖫∀𝖬∫: Client/Server not initialized");
    }
    this.vsc = ctx.vsc;
    this.outputChannel = ctx.outputChannel;
    this.versions = ctx.versions;
    this.client = ctx.client;
    this.server = ctx.server;
    const remote = <string|undefined>vscode.workspace.getConfiguration("flams").get(Settings.RemoteFlams);
    this.remote_server = new FLAMSServer(remote?remote:"https://mathhub.info");
    _context = this;
  }

  register_command(
    name: string,
    callback: (...args: any[]) => any,
    thisArg?: any,
  ) {
    const disposable = vscode.commands.registerCommand(name, callback, thisArg);
    this.vsc.subscriptions.push(disposable);
  }
}
