import * as vscode from 'vscode';
import { call_cmd } from './utils';
import * as fs from "fs";
import { Settings } from './commands';

export class Versions {
  private _latex:boolean | undefined;
	private _stex_path: string | undefined;
	private _stex_version : Version | undefined;
	private _flams_version : Version | undefined;

  constructor() {}

  get flams_path(): string | undefined {
		const config = vscode.workspace.getConfiguration("flams");
		return config.get<string>(Settings.FlamsPath)?.trim();
	}

	get stex_path(): string | undefined {
		return this._stex_path;
	}

  async hasLatex(): Promise<boolean> {
		if (this._latex) {return true;}
		let res = await call_cmd("kpsewhich",["--version"]);
		this._latex = (res !== undefined);
		return this._latex;
	}
  async hasSTeX(): Promise<boolean> {
		if (this._stex_path) {return true;}
		let res = await call_cmd("kpsewhich",["stex.sty"]);
		if (res) {
			this._stex_path = res.trim();
			this._latex = true;
			return true;
		}
		return false;
	}

  async isValid(): Promise<boolean> {
		let flams = await this.flamsVersion();
		let stex = await this.stexVersion();
		return flams !== undefined && stex !== undefined && 
      flams.newer_than(REQUIRED_FLAMS) && 
      stex.newer_than(REQUIRED_STEX);
	}

	async stexVersion(): Promise<Version | undefined> {
		if (this._stex_version) {return this._stex_version;}
		await this.hasSTeX();
		if (this._stex_path) {
			let ret = fs.readFileSync(this._stex_path).toString();
			const regex = /\\message{\^\^J\*~This~is~sTeX~version~(\d+\.\d+\.\d+)~\*\^\^J}/;
			const match = ret.match(regex);
			if (match) {
				const vstring = match[1];
				this._stex_version = new Version(vstring);
				return this._stex_version;
			}
		}
	}

  async flamsVersion(): Promise<Version | undefined> {
		if (this._flams_version) {return this._flams_version; }
    let path = this.flams_path;
    if (path) {
      let res = await call_cmd(path,["--version"]);
      if (res) {
			  const regex = /flams (\d+\.\d+\.\d+)/;
        const match = res.match(regex);
        if (match) {
          const vstring = match[1];
          this._flams_version = new Version(vstring);
          return this._flams_version;
        }
      }
    }
	}

  reset() {
		this._flams_version = undefined;
	}
}

export class Version {
	major : number;
	minor : number;
	revision: number;
	constructor(s:string | [number,number,number]) {
		if (typeof(s) === "string") {
			const match = s.trim().split('.');
			if (match.length ===1 ) {
				[this.major,this.minor,this.revision] = [parseInt(match[0]),0,0];
			} else if (match.length === 2) {
				[this.major,this.minor,this.revision] = [parseInt(match[0]),parseInt(match[1]),0];
			} else {
				[this.major,this.minor,this.revision] = [parseInt(match[0]),parseInt(match[1]),parseInt(match[2])];
			}
		} else {
			[this.major,this.minor,this.revision] = s;
		}
	}
	toString() : string {
		return this.major.toString() + "." + this.minor.toString() + "." + this.revision.toString();
	}
	newer_than(that: Version):boolean {
		return this.major > that.major || (
			this.major === that.major && (
				this.minor > that.minor || (
					this.minor === that.minor && this.revision >= that.revision
				)
			)
		);
	}
}

export const REQUIRED_FLAMS = new Version([0,0,6]);
export const REQUIRED_STEX = new Version([4,1,0]);

