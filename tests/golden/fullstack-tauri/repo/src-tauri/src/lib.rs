// fairlead: def cmd.note-search
#[tauri::command]
pub fn note_search(query: String) -> Vec<String> {
    vec![query]
}
