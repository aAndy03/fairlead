+++
id = "d-0mr5p13k"
kind = "change"
status = "sealed"
impact = "fix"
scope = "root"
title = "seal --reseal names the tag left on the undone seal"
covers = ["crates/fairlead-core/src/git.rs", "crates/fairlead/src/commands/seal.rs"]
created = "0.2.2"
sealed = "0.2.2"
body-hash = "b3:3va6ybv79k18x2ke4wv9"
ordinal = 14
+++

# seal --reseal names the tag left on the undone seal

When `fairlead seal --reseal` renumbers your own unintegrated seal, a `fairlead/<version>` tag made earlier by `seal --tag` still points at the undone seal. The plan and the result now name that tag and the git commands that delete it; fairlead does not delete tags itself.
