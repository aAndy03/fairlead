// fairlead: def ctr.shell path="crates/shell/**"
// fairlead: def un.shell layer=core.shell container=ctr.shell
// fairlead: use un.service
// fairlead: def ent.cli etype=cli unit=un.shell
fn main() {}
