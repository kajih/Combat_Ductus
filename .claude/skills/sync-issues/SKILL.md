---
name: sync-issues
description: Reconcile docs/issues/ files with GitHub Issues, then print the open issues as a table in recommended working order. Use when the user asks what to work on next, what the next issue is, or to sync/mirror issues to GitHub.
---

# Sync Issues

Two jobs, always in this order: **reconcile** the local `docs/issues/` tree against GitHub Issues, then **rank** what's open and print it as a table.

Local files are the source of truth for design and reasoning. GitHub is the mirror, and is authoritative only for a single fact: whether an issue is open or closed.

## 1. Gather state

Run the bundled script from the repo root. One call replaces the whole hand-run
find/`gh`/read sequence, and it reads nothing back from GitHub that it doesn't need:

```
.claude/skills/sync-issues/gather.sh
```

It prints, in order:

- **RECONCILE** — every mechanical check in section 2 below, already run: missing footers, dangling footers, label drift, missing labels, GitHub issues with no local file, and checkbox drift in both directions.
- **OPEN ISSUES** — one row per open issue with its category, `low-priority` flag, checked/total criteria, the date it was filed, and `SRC+`, the number of commits to `src/` since that date.
- **Blocked by** — every `Blocked by` reference resolved against current GitHub state, split into live blockers and **STALE TEXT** (a blocker that has since closed, or a PR that has since merged, while the local file still reads as pending).
- **Parent rollup** — open/closed child counts per `## Parent`, which is how tier 1 below is identified.
- **OPEN ISSUE BODIES** — the full text of every open issue's local file, so the ranking runs on `Parent`/`What to build`/`Blocked by`/`Priority` rather than on titles.

Flags: `--cached` reuses GitHub state fetched in the last 15 minutes (use it when re-running within one session); `--no-bodies` prints the reconcile half only.

The script reports; it never writes. It does not judge whether an issue is already
satisfied by landed code, whether a body has drifted from its GitHub mirror, or how
to rank anything — those are sections 2–3, and they're yours.

## 2. Reconcile

`gather.sh` has already run every check in this section except the last one, so read
its RECONCILE block rather than redoing the work; what follows is what each finding
means and what to do about it. **Do not write to GitHub yet** — gather everything
first, then present it and ask once before making any changes. Creating and editing
issues is outward-facing and shouldn't happen silently just because the user asked
"what's next".

<checks>
- **Local file with no `## GitHub Issue` footer** → needs a GitHub issue created. The footer is the de-duplication marker; a file that has one is already synced and must never be re-created.
- **Label** → the issue's label is its `docs/issues/` subfolder name (`combat-foundation`, `audio`, `character-select`, `observability`, `spectator-mode`, `tooling`). A new subfolder needs a matching label created (`gh label create <name> --description "docs/issues/<name>"`).
- **GitHub issue with no local file** → flag it to the user. Never auto-create a local file: the local file carries design reasoning that can't be reconstructed from a GitHub title.
- **Closed on GitHub, unchecked acceptance criteria locally** → report as drift. Don't check boxes off as part of a sync; that's deliberate per-criterion verification work (see `docs/issues/tooling/backfill-issue-checkbox-hygiene.md`), not bookkeeping.
- **Open on GitHub, all criteria checked locally** → report as drift the other way; the issue may be closeable.
- **Body drift** → the one check the script can't make: if a local file's `What to build` has materially changed since it was mirrored, offer to update the GitHub body. Minor wording changes aren't worth a round trip.
- **STALE TEXT from the blocker pass** → the local file still describes a blocker that has since closed or merged. Offer to correct the `Blocked by` section; it's a local-only edit with no GitHub round trip, and leaving it makes an unblocked issue read as blocked for the next session.
</checks>

To create a missing issue:

```
gh issue create --title "<Title>" --label "<subfolder>" --body-file <path>
```

Then append the footer to the local file:

```

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/<n>
```

## 3. Rank the open issues

Before ranking, apply one filter that matters more than any ordering heuristic — the script's `SRC+` column is the prompt for it, not the answer:

> **Check whether an issue's acceptance criteria are already satisfied by code that landed after it was filed.** Issues filed early in a project routinely get overtaken by later work. Such an issue isn't "next to build" — it's "next to verify and close", which is a much cheaper action and belongs at the top of the table marked as such.

Then sort into tiers:

<tiers>
1. **Finishes work already in flight** — an open issue whose `Parent` PRD has every other issue closed. Leaving one tuning follow-up dangling is what makes a PRD read as unfinished; these are usually small and always high-value.
2. **Unblocked feature work**, ordered by player-visible value against size. A change a player feels in the first ten seconds of a match outranks a larger change they'd have to go looking for.
3. **Dev tooling and docs hygiene** — real work, no gameplay surface.
4. **Anything carrying the `low-priority` label, or a `## Priority` section saying Low.** Respect what past-you decided here; those sections usually say *when* the issue is worth doing ("on the next LAN playtest", "fold into the next CLAUDE.md edit"). Quote that condition rather than re-deriving a priority.
</tiers>

**Blocked issues do not get a rank.** An issue whose `Blocked by` names a still-open issue goes in a separate short list below the table, naming what blocks it. The script's blocker pass has already re-resolved every reference against current state — trust that over what the files say, since blockers get closed without the blocked file being updated.

Pair issues where it's free: two issues that both need a manual playtest, or two that both touch `CLAUDE.md`, are cheaper done together. Say so.

## 4. Output

Print the reconciliation result first — one line per finding, or a single line saying the tree and GitHub agree. Then the table:

| # | Issue | Category | Size | Why now |
|---|-------|----------|------|---------|

`Size` is S/M/L from what the file actually asks for, not from its length. `Why now` is one clause — the reason for *this* position in the order, not a summary of the issue.

Lead with the top pick called out above the table, including the specific first change it needs (file and constant, or the decision the issue leaves open to whoever picks it up), so the answer is actionable without opening the file. Close by offering to start it.
