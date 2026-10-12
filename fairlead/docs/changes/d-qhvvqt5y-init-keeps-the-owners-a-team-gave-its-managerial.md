+++
id = "d-qhvvqt5y"
kind = "change"
status = "draft"
impact = "fix"
sum = "init no longer appends a CODEOWNERS line that overrides the owners a team already gave the managerial folder"
out = "shipped"
why = "CODEOWNERS applies the last matching line, so init narrowed the owners and then warned about it (HN-A004)"
scope = "root"
title = "init keeps the owners a team gave its managerial folder in CODEOWNERS"
covers = ["crates/fairlead-core/src/write/init.rs"]
created = "0.3.2"
+++

# init keeps the owners a team gave its managerial folder in CODEOWNERS

When CODEOWNERS already assigned the managerial folder (for example `/fairlead/ @frontend @backend`), init still appended `/fairlead/ @frontend` and `**/fairlead/ @frontend`. CODEOWNERS uses the last matching line, so the second owner was dropped and check then reported HN-A004 about it. init now skips a pattern the file already assigns and gives the nested pattern the same owners.
