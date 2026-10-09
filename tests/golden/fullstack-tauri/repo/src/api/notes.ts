// fairlead: use cmd.note-search
export async function noteSearch(query: string) {
  return invoke("note_search", { query });
}
