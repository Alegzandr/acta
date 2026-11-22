# Security policy

Acta keeps a person's project records and their agent's memory in an encrypted local
database. A flaw that exposes that data, its key, or the machine Acta runs on matters to
us.

## Reporting a vulnerability

Report it privately through GitHub: on this repository's **Security** tab, choose
**Report a vulnerability**. Please do not open a public issue, pull request or
discussion about it.

Include what you can: the Acta version and system, what an attacker can do, and the
steps to reproduce. We will acknowledge the report, keep you informed, and credit you in
the fix's release notes unless you prefer not to be named.

## Supported versions

Acta is alpha. Only the latest release is supported, and fixes ship as a new
release that installed copies pick up at their next start.

## Scope

In scope: the app, the `acta` command line and MCP server, the install scripts, the
update path, and the release workflow of this repository.

Out of scope: problems that need an attacker who already controls your user account,
and warnings caused by the builds being unsigned (a documented decision, see
[docs/install.md](docs/install.md#unsigned-builds)).
The checks in the command line that tell Claude Code from you (for example refusing
import, export and snapshot restore to Claude Code) are guard rails against prompt
injection, not a security boundary: Claude Code runs as your user and could do the same
things another way.
