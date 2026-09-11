import * as Backend from "@flexiformal/ftml-backend";

export namespace Base {
  export type ArchiveId = Backend.ArchiveId;
  export type Timestamp = number;
  export type DirectoryData = Backend.DirectoryData;
  export type FileData = Backend.FileData;
  export type ArchiveData = Backend.ArchiveData;

  export interface FileStateSummary {
    new: number;
    stale: number;
    deleted: number;
    up_to_date: number;
    last_built: Timestamp;
    last_changed: Timestamp;
  };
  export interface ArchiveGroupData {
    id: ArchiveId;
    summary?: FileStateSummary | undefined;
  }
}

export class FLAMSServer {
  _url: string;
  constructor(url: string) {
    this._url = url;
  }
  get url(): string {
    return this._url;
  }

  /**
 * Get all dependencies of the given archive (excluding meta-inf archives)
 */
  public async archiveDependencies(archives: Base.ArchiveId[]): Promise<Base.ArchiveId[] | undefined> {
    return await this.rawPostRequest("api/backend/archive_dependencies", {
      archives
    });
  }
  /**
* List all directories/files in the given archive at path (or at top-level, if undefined)
*/
  public async backendArchiveEntries(
    archive: string,
    in_path?: string,
  ): Promise<[Base.DirectoryData[], Base.FileData[]] | undefined> {
    return await this.rawPostRequest("api/backend/archive_entries", {
      archive: archive,
      path: in_path,
    });
  }

  /**
   * List all archives/groups in the given group (or at top-level, if undefined)
   */
  public async backendGroupEntries(
    in_entry?: string,
  ): Promise<[Base.ArchiveGroupData[], Base.ArchiveData[]] | undefined> {
    return await this.rawPostRequest("api/backend/group_entries", {
      in: in_entry,
    });
  }

  public async rawPostRequest<TRequest extends Record<string, unknown>, TResponse>(
    endpoint: string,
    request: TRequest
  ): Promise<TResponse | undefined> {
    const response = await this.postRequestI(endpoint, request);
    if (response) {
      const j = await response.json();
      return j as TResponse;
    }
  }

  async postRequestI<TRequest extends Record<string, unknown>>(
    endpoint: string,
    request: TRequest,
  ): Promise<Response | undefined> {
    const backendUrl = this._url;
    const formData = new URLSearchParams();
    const appendToForm = (obj: unknown, prefix = ""): void => {
      if (Array.isArray(obj)) {
        obj.forEach((v, i) => appendToForm(v, `${prefix}[${i}]`));
      } else if (obj instanceof Date) {
        formData.append(prefix, obj.toISOString());
      } else if (obj && typeof obj === "object" && !(obj instanceof File)) {
        for (const [key, value] of Object.entries(obj)) {
          const newPrefix = prefix ? `${prefix}[${key}]` : key;
          appendToForm(value, newPrefix);
        }
      } else if (obj !== undefined && obj !== null) {
        formData.append(prefix, String(obj));
      }
    };
    appendToForm(request);
    //console.log(`Calling ${backendUrl}/${endpoint} with body`, formData);
    const response = await fetch(`${backendUrl}/${endpoint}`, {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
      },
      body: formData.toString(),
    });

    if (response.ok) {
      return response;
    }
  }
}