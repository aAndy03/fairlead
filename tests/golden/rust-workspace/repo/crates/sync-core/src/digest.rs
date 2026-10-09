// fairlead: def fn.digest
// fairlead: ovr rule.primitive-io why="streams file bytes; adapter split deferred" owner=@core exp=2026-12-31
pub fn digest_file(path: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}
