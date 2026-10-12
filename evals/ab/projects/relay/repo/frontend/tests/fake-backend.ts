// fairlead: use un.api
// A test double of the back end that follows contract/commands.md, for front-end tests.
import { type Invoke } from "../src/api.ts";

export function fakeBackend(): Invoke & { requests: string[] } {
  const notes = new Map<number, { title: string; body: string }>();
  let next = 0;
  const requests: string[] = [];
  const enc = (s: string) => encodeURIComponent(s);
  const invoke = async (request: string): Promise<string> => {
    requests.push(request);
    const [cmd, query = ""] = request.split("?");
    const args: Record<string, string> = {};
    for (const p of query.split("&").filter(Boolean)) {
      const [k, v = ""] = p.split("=");
      args[decodeURIComponent(k)] = decodeURIComponent(v);
    }
    switch (cmd) {
      case "notes.list":
        return "ok " + [...notes].map(([id, n]) => `id=${id}&title=${enc(n.title)}`).join(";");
      case "notes.get": {
        const n = notes.get(Number(args.id));
        return n ? `ok id=${args.id}&title=${enc(n.title)}&body=${enc(n.body)}` : "err not-found";
      }
      case "notes.create": {
        if (!args.title?.trim()) return "err empty-title";
        notes.set(++next, { title: args.title.trim(), body: args.body ?? "" });
        return `ok id=${next}`;
      }
      case "notes.delete":
        return notes.delete(Number(args.id)) ? "ok " : "err not-found";
      default:
        return `err unknown-command-${cmd}`;
    }
  };
  return Object.assign(invoke, { requests });
}
