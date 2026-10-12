+++
id = "d-s3ecaheq"
kind = "change"
status = "ready"
impact = "fix"
sum = "fairlead-mcp --help explains how a client starts it; --version prints the version"
out = "shipped"
why = "run by hand, the server waited silently on stdin, which looked like a hang"
scope = "root"
title = "fairlead-mcp answers --help and --version"
covers = ["crates/fairlead-mcp/**", "README.md", "Cargo.toml", "crates/fairlead-core/Cargo.toml"]
created = "0.3.1"
+++

# fairlead-mcp answers --help and --version

Run by hand, `fairlead-mcp` used to wait silently for messages on stdin. `--help` now explains that an agent's client starts and stops it, that it uses no port and no network, and how to register it (`claude mcp add fairlead -- fairlead-mcp`). `--version` prints its version, and any other argument is a usage error (exit 2). The README gains a short section on using fairlead with an agent.
