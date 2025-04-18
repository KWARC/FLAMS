import * as vscode from 'vscode';
import { exec } from "child_process";
import { promisify } from "util";
import * as fs from 'fs';
import https from 'follow-redirects';
import * as yauzl from 'yauzl';
import path from 'path';
import * as mkdirp from 'mkdirp';

export function insertUsemodule(doc:vscode.TextDocument,archive:string|undefined,path:string) {
  let insertline = 0;
  let lastemptyline = 0;
  let inbody = false;
  let done = false;
  while (insertline < doc.lineCount && !done) {
    let line = doc.lineAt(insertline).text;
    if (!inbody) {
      insertline += 1;
      if (line.indexOf("\\begin{document") !== -1) {
        inbody = true;
        lastemptyline = insertline;
      }
    } else if (line.trim() === "") {
      insertline += 1;
    } else if (line.indexOf("\\usemodule") !== -1 || line.indexOf("\\importmodule") !== -1) {
      insertline += 1;
      lastemptyline = insertline;
    } else { done = true;}
  }
  const pos = new vscode.Position(lastemptyline,0);
  const edit = new vscode.WorkspaceEdit();
  if (archive) {
    edit.insert(doc.uri,pos,`\\usemodule[${archive}]{${path}}\n`);
  } else {
    edit.insert(doc.uri,pos,`\\usemodule{${path}}\n`);
  }
  vscode.workspace.applyEdit(edit);
}

const execPromise = promisify(exec);

export async function call_cmd(cmd:string,args:string[]) : Promise<string | undefined> {
  try {
    const wsf = vscode.workspace.workspaceFolders;
    const cwd = wsf? wsf[0].uri.fsPath : "";
    const {stdout} = await execPromise(`"${cmd}" ` + args.join(" "),{ env: process.env, cwd});
    return stdout.trim();
  } catch (error) {
    return undefined;
  }
}

export async function download(url:string,to:string): Promise<boolean> {
  return await new Promise<boolean>((resolve,reject) => {
    const file = fs.createWriteStream(to);
    let req = https.https.get(url, (response) => {
      response.pipe(file);
      file.on('finish', () => {
        file.close();
        resolve(true);
      });
    });
    req.on('error', err => {
      vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Error downloading ${url}: ${err}`);
      file.close();
      fs.unlink(to,() => {});
      resolve(false);
    });
    req.on('abort', () => {
      vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Error downloading ${url}: aborted`);
      file.close();
      fs.unlink(to,() => {});
      resolve(false);
    });
    req.on('timeout', () => {
      vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Error downloading ${url}: timeout`);
      file.close();
      fs.unlink(to,() => {});
      resolve(false);
    });
  });
}

export async function unzip(zip:string,to:string,files:string[],skip:string[],executables:string[],progress?:vscode.Progress<{
  message?: string;
  increment?: number;
}>): Promise<boolean> {
  return await new Promise<boolean>((resolve,reject) => {
    yauzl.open(zip, {lazyEntries: true, autoClose: false}, (err, zipfile) => {
      if (err || !zipfile) {
        vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Error unzipping ${zip}: ${err}`);
        resolve(false);
        return;
      }

      zipfile.on('entry', (entry) => {
        const filename = entry.fileName;
        if (/\/$/.test(filename)) {
          mkdirp.sync(path.join(to,filename));
          zipfile.readEntry();
        } else {
          if (progress) {
            progress.report({ message: `Unzipping ${zip}/${filename}`});
          }
          if ((files.length === 0 || files.includes(filename)) && !skip.includes(filename)) {
            const target = path.join(to,filename);
            zipfile.openReadStream(entry, (err, readStream) => {
              if (err) { 
                vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Error unzipping ${zip}/${filename}: ${err}`);
                resolve(false);
                return;
              }
              const writer = fs.createWriteStream(target);
              readStream.on('end', () => {
                writer.close();
                if (!process.platform.startsWith("win") && executables.includes(filename)) {
                  fs.chmod(target, 0o755,() => {});
                }
                zipfile.readEntry();
              });
              //writer.on('finish', () => {
              //  writer.close();
              //});
              readStream.pipe(writer);
            });
          } else {
            zipfile.readEntry();
          }
        }
      });

      zipfile.on('error', (err) => {
        vscode.window.showErrorMessage(`𝖥𝖫∀𝖬∫: Error unzipping ${zip}: ${err}`);
        resolve(false);
        return;
      });

      zipfile.on('end', () => { 
        zipfile.close();
        resolve(true);
      });
      zipfile.readEntry();
    });
  });
}

export function add_exe(s:string):string {
	if (process.platform.startsWith("win")) {
		return s + ".exe";
	} else {return s;}
}