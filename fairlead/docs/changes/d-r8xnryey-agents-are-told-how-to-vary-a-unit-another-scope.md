+++
id = "d-r8xnryey"
kind = "change"
status = "ready"
impact = "fix"
sum = "the agent block, explain, new --help and new request lead an agent to the variation protocol"
out = "shipped"
why = "in agent trial B-3 no agent found the protocol; each copied the unit instead"
scope = "root"
title = "Agents are told how to vary a unit another scope owns"
covers = ["crates/fairlead-core/src/explain/**", "crates/fairlead-core/src/explain.rs", "crates/fairlead-core/src/write/init.rs", "crates/fairlead-core/src/check/builtin/agents.rs", "crates/fairlead-core/src/registry/item.rs", "crates/fairlead/src/**"]
created = "0.3.1"
+++

# Agents are told how to vary a unit another scope owns

The block fairlead writes into AGENTS.md and CLAUDE.md now says what fairlead is for, where its rules are explained (with the list of explain topics), and never to copy or edit code another scope owns. A new topic, explain variation, walks through the four moves a team has when a unit it uses lacks something: use what exists, request it from the home scope, wrap it meanwhile, or keep an adapter after a rejection. explain on a unit adds a vary section with the exact commands, new --help lists request, and new request prints the wrapper to write next. A repository whose block still has the earlier text sees no new finding; init --yes updates it.
