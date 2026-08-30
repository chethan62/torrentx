# Security Policy

## Supported versions

Only the latest release receives security fixes:

| Version | Supported |
|---------|-----------|
| 18.2.x  | ✅        |
| older   | ❌ — please update |

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Use GitHub's private security advisory:
**https://github.com/chethan62/torrentx/security/advisories/new**

Include: affected version, steps to reproduce, and (if helpful) the
anonymous Install ID from the About tab. You'll get a response within a
few days; fixes land in the next release and you'll be credited unless
you prefer otherwise.

## Scope notes

TorrentX is a local GUI talking to *your* Jackett server. Areas worth
scrutiny if you're auditing:

- Indexer-supplied strings (titles, URLs, magnets) — untrusted input
  - opened URLs/URIs are scheme-allow-listed (http/https/magnet) before
    handing them to the OS opener (`safe_open` in `src/main.rs`)
  - CSV export neutralizes formula-leading characters (`csv_safe`)
  - XML entity handling is bounded (numeric refs, feature-gated named set)
- The config file holds your Jackett API key — written with mode 0600
- Outbound traffic: your Jackett server only, plus one optional GitHub
  releases check (disable via Settings → Updates). No telemetry.

Out of scope: vulnerabilities in Jackett itself, or in torrent clients
launched via magnet links.
