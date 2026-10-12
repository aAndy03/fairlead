// fairlead: def ctr.frontend path="frontend/**"
// fairlead: def un.api layer=app.view container=ctr.frontend sum="the front end's only door to the back end"
// The only module that talks to the back end. Every command of contract/commands.md has one
// function here.

/** Sends one request line and resolves with the reply line (`ok ...` or `err ...`). */
export type Invoke = (request: string) => Promise<string>;

export interface NoteSummary {
  id: number;
  title: string;
}

export interface Note extends NoteSummary {
  body: string;
}

export class BackendError extends Error {
  code: string;
  constructor(code: string) {
    super(`backend error: ${code}`);
    this.code = code;
  }
}

function fields(record: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const pair of record.split("&")) {
    if (!pair) continue;
    const [k, v = ""] = pair.split("=");
    out[decodeURIComponent(k)] = decodeURIComponent(v);
  }
  return out;
}

async function call(invoke: Invoke, command: string, args: Record<string, string> = {}): Promise<string> {
  // Known issue: argument values are not percent-encoded.
  const query = Object.entries(args)
    .map(([k, v]) => `${k}=${v}`)
    .join("&");
  const reply = await invoke(query ? `${command}?${query}` : command);
  if (reply.startsWith("ok ")) return reply.slice(3);
  if (reply === "ok") return "";
  throw new BackendError(reply.replace(/^err /, ""));
}

export function createApi(invoke: Invoke) {
  return {
    // fairlead: def brg.notes-list side=ts ver=1
    async listNotes(): Promise<NoteSummary[]> {
      const payload = await call(invoke, "notes.list");
      return payload
        .split(";")
        .filter(Boolean)
        .map((r) => {
          const f = fields(r);
          return { id: Number(f.id), title: f.title };
        });
    },
    // fairlead: def brg.notes-get side=ts ver=1
    async getNote(id: number): Promise<Note> {
      const f = fields(await call(invoke, "notes.get", { id: String(id) }));
      return { id: Number(f.id), title: f.title, body: f.body };
    },
    // fairlead: def brg.notes-create side=ts ver=1
    async createNote(title: string, body = ""): Promise<number> {
      const f = fields(await call(invoke, "notes.create", { title, body }));
      return Number(f.id);
    },
    // fairlead: def brg.notes-delete side=ts ver=1
    async deleteNote(id: number): Promise<void> {
      await call(invoke, "notes.delete", { id: String(id) });
    },
  };
}

export type Api = ReturnType<typeof createApi>;
