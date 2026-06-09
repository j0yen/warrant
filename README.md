# warrant

Close-claim corpus and assertion runner for the wintermute suite.

## What it does

A PRD closed with *"outcome achieved live by a different mechanism"* retires a
finding, but nothing parses, counts, or re-checks those closing claims — so a
false one sits undetected until a whole new vision rediscovers the symptom from
scratch.

`warrant` is the root of that toolkit. This workspace defines:

- A typed domain model (`CloseClaim`, `ClaimKind`, `CloseRef`, `Warrant`,
  `WarrantVerdict`, `AssertionSpec`)
- A **pure** classifier (`classify`) that turns raw close notes into typed
  claims — zero IO, zero env access
- A `CloseSource` trait + `FakeSource` for tests
- A CLI with `warrant list` and `warrant list-sources`

## Workspace members

| Crate | Role |
|---|---|
| `warrant-core` | Domain model, classifier, trait definitions |
| `warrant-cli` | Binary: `warrant list [--format json\|table]`, `list-sources` |

Downstream crates (separate PRDs):
- `warrant-audit` — real filesystem/docket `CloseSource`, runs assertions
- `warrant-docket` — durable docket store for warrant results

## Usage

```
warrant list                  # human table + tally
warrant list --format json    # stable serde AuditPlan JSON
warrant list-sources          # show available sources
```

## ClaimKind taxonomy

| Kind | Meaning |
|---|---|
| `AcsMet` | All ACs met, no special signal |
| `MechanismAsserted` | "Achieved live by a different mechanism" — audit risk |
| `Superseded` | Closed by another PRD or vision |
| `LiveDeferred` | Has `deferred_acs:` in the close note |
| `Unclassified` | No status line found |

## MSRV

Rust 1.85 (pinned via `rust-toolchain.toml`).

## License

MIT OR Apache-2.0
