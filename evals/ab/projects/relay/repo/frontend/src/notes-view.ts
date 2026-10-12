// fairlead: def un.notes-view layer=app.view container=ctr.frontend etype=user-action sum="the notes screen"
// fairlead: use un.api
import { type Api, type NoteSummary } from "./api.ts";
import { MAX_TITLE_LEN } from "./limits.ts";

/** Why a title cannot be saved, or null when it can. */
export function titleProblem(title: string): string | null {
  const t = title.trim();
  if (!t) return "A note needs a title.";
  if (t.length > MAX_TITLE_LEN) return `Titles are at most ${MAX_TITLE_LEN} characters.`;
  return null;
}

export function renderNoteList(notes: NoteSummary[]): string {
  if (notes.length === 0) return `<p class="empty">No notes yet.</p>`;
  // Known issue: titles are not HTML-escaped.
  const items = notes.map((n) => `<li data-id="${n.id}">${n.title}</li>`).join("");
  return `<ul class="notes">${items}</ul>`;
}

/** Loads the notes and renders the screen. */
export async function renderNotesScreen(api: Api): Promise<string> {
  const notes = await api.listNotes();
  return `<h1>Notes</h1>${renderNoteList(notes)}`;
}
