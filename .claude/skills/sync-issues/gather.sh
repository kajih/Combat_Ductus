#!/usr/bin/env bash
#
# gather.sh - one-shot state dump for the sync-issues skill.
#
# Replaces the hand-run find/gh/grep sequence in step 1 of SKILL.md with a
# single call: it fetches GitHub state once, joins it against the local
# docs/issues/ tree, runs every mechanical reconcile check, resolves each
# `Blocked by` against current state, and prints the full text of every open
# issue's local file so the ranking can be done without further reads.
#
# It never writes to GitHub and never edits a local file. Everything it finds
# is a *finding*, to be presented to the user before anything acts on it.
#
# Usage:
#   ./gather.sh              # fetch fresh GitHub state (default)
#   ./gather.sh --cached     # reuse a cache younger than 15 minutes, if present
#   ./gather.sh --no-bodies  # skip the open-issue full text (reconcile only)
#
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

ISSUE_DIR="docs/issues"
REPO_URL="https://github.com/kajih/Combat_Ductus/issues"

CACHE_DIR="${TMPDIR:-/tmp}/sync-issues-cache-$(id -u)"
mkdir -p "$CACHE_DIR"
ISSUES_JSON="$CACHE_DIR/issues.json"
LABELS_TXT="$CACHE_DIR/labels.txt"

USE_CACHE=0
WITH_BODIES=1
for arg in "$@"; do
  case "$arg" in
    --cached)     USE_CACHE=1 ;;
    --no-bodies)  WITH_BODIES=0 ;;
    -h|--help)    sed -n '3,20p' "$0"; exit 0 ;;
    *) echo "gather.sh: unknown argument '$arg'" >&2; exit 2 ;;
  esac
done

fetch_github() {
  gh issue list --state all --limit 500 \
     --json number,title,state,labels,updatedAt > "$ISSUES_JSON"
  gh label list --json name --jq '.[].name' > "$LABELS_TXT"
}

if [[ $USE_CACHE -eq 1 && -s "$ISSUES_JSON" && -s "$LABELS_TXT" ]] \
   && find "$ISSUES_JSON" -mmin -15 | grep -q .; then
  echo "(using cached GitHub state from $ISSUES_JSON)"
else
  fetch_github
fi

# ---------------------------------------------------------------- local side
# One TSV row per local file: path, issue number (or NONE), subfolder label,
# total criteria, unchecked criteria, low-priority flag, parent.
LOCAL_TSV="$CACHE_DIR/local.tsv"
: > "$LOCAL_TSV"

while IFS= read -r f; do
  num=$(grep -oE "$REPO_URL/[0-9]+" "$f" | grep -oE '[0-9]+$' | tail -1 || true)
  label=$(dirname "${f#"$ISSUE_DIR"/}")
  total=$(grep -cE '^[[:space:]]*- \[[ xX]\]' "$f" || true)
  unchecked=$(grep -cE '^[[:space:]]*- \[[[:space:]]\]' "$f" || true)
  low=no
  if awk '/^## Priority/{p=1;next} /^## /{p=0} p' "$f" | grep -qiE '\blow\b'; then
    low=yes
  fi
  parent=$(awk '/^## Parent/{p=1;next} /^## /{p=0} p' "$f" \
           | grep -oE '[A-Za-z0-9._/-]+\.md' | head -1 || true)
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$f" "${num:-NONE}" "$label" "$total" "$unchecked" "$low" "${parent:-none}" \
    >> "$LOCAL_TSV"
done < <(find "$ISSUE_DIR" -name '*.md' | sort)

# --------------------------------------------------------------- github side
GH_TSV="$CACHE_DIR/gh.tsv"
jq -r '.[] | [.number, .state, ([.labels[].name] | join(",")), .title]
       | @tsv' "$ISSUES_JSON" | sort -n > "$GH_TSV"

gh_state() { awk -F'\t' -v n="$1" '$1==n{print $2}' "$GH_TSV"; }
gh_title() { awk -F'\t' -v n="$1" '$1==n{print $4}' "$GH_TSV"; }
gh_labels() { awk -F'\t' -v n="$1" '$1==n{print $3}' "$GH_TSV"; }

# number -> local path, for blocked-by and parent resolution
path_for_num() { awk -F'\t' -v n="$1" '$2==n{print $1}' "$LOCAL_TSV"; }
num_for_path() { awk -F'\t' -v p="$1" '$1==p{print $2}' "$LOCAL_TSV"; }
# resolve a bare `foo.md` or `docs/issues/x/foo.md` reference to a local path
resolve_ref() {
  local ref base
  ref="$1"; base=$(basename "$ref")
  awk -F'\t' -v b="/$base" 'index($1, b) && substr($1, length($1)-length(b)+1)==b {print $1; exit}' "$LOCAL_TSV"
}

echo
echo "=============================================================="
echo "RECONCILE"
echo "=============================================================="

findings=0
note() { findings=$((findings+1)); printf -- "- %s\n" "$*"; }

# 1. local files with no footer
while IFS=$'\t' read -r f num label _ _ _ _; do
  [[ "$num" == NONE ]] && note "NO FOOTER: $f -> needs 'gh issue create --title \"...\" --label \"$label\" --body-file $f'"
done < "$LOCAL_TSV"

# 2. footer pointing at an issue that does not exist
while IFS=$'\t' read -r f num _ _ _ _ _; do
  [[ "$num" == NONE ]] && continue
  [[ -z "$(gh_state "$num")" ]] && note "DANGLING FOOTER: $f points at #$num, which GitHub does not have"
done < "$LOCAL_TSV"

# 3. label mismatch
while IFS=$'\t' read -r f num label _ _ _ _; do
  [[ "$num" == NONE ]] && continue
  labels=$(gh_labels "$num")
  [[ -z "$labels" ]] && continue
  if [[ ",$labels," != *",$label,"* ]]; then
    note "LABEL DRIFT: #$num ($f) is in $label/ but labeled '${labels:-<none>}'"
  fi
done < "$LOCAL_TSV"

# 4. subfolder with no matching label
for d in "$ISSUE_DIR"/*/; do
  name=$(basename "$d")
  grep -qx "$name" "$LABELS_TXT" || note "MISSING LABEL: subfolder $name/ has no GitHub label -> gh label create $name --description \"$ISSUE_DIR/$name\""
done

# 5. GitHub issue with no local file
while IFS=$'\t' read -r num state _ title; do
  [[ -z "$(path_for_num "$num")" ]] && note "NO LOCAL FILE: #$num [$state] \"$title\" - flag to the user, never auto-create"
done < "$GH_TSV"

# 6. checkbox drift, both directions
closed_unchecked=0; closed_boxes=0; drifted=()
while IFS=$'\t' read -r f num _ total unchecked _ _; do
  [[ "$num" == NONE ]] && continue
  state=$(gh_state "$num")
  if [[ "$state" == CLOSED && "$unchecked" -gt 0 ]]; then
    closed_unchecked=$((closed_unchecked+1)); closed_boxes=$((closed_boxes+unchecked))
    drifted+=("#$num($unchecked/$total)")
  elif [[ "$state" == OPEN && "$total" -gt 0 && "$unchecked" -eq 0 ]]; then
    note "MAYBE CLOSEABLE: #$num ($f) is open but all $total criteria are checked"
  fi
done < "$LOCAL_TSV"
if [[ $closed_unchecked -gt 0 ]]; then
  note "CHECKBOX DRIFT: $closed_unchecked closed issues carry $closed_boxes unchecked criteria - ${drifted[*]} (unchecked/total). This is what backfill-issue-checkbox-hygiene.md tracks; do not tick boxes as part of a sync."
fi

[[ $findings -eq 0 ]] && echo "- The local tree and GitHub agree."

echo
echo "=============================================================="
echo "OPEN ISSUES - blockers, parents, staleness"
echo "=============================================================="
printf '%-5s %-18s %-4s %-9s %-11s %-6s %s\n' '#' 'CATEGORY' 'LOW' 'CRITERIA' 'FILED' 'SRC+' 'FILE'

while IFS=$'\t' read -r f num label total unchecked low parent; do
  [[ "$num" == NONE ]] && continue
  [[ "$(gh_state "$num")" == OPEN ]] || continue

  # How much production code has landed since this issue was FILED - the cheap
  # signal for "filed early, already overtaken by later work", which makes it a
  # verify-and-close rather than something to build.
  filed=$(git log --diff-filter=A --format=%ad --date=short -- "$f" 2>/dev/null | tail -1)
  touched=$(git log -1 --format=%ad --date=short -- "$f" 2>/dev/null || true)
  since=$(git log --oneline --since="${filed:-1970-01-01}" -- src 2>/dev/null | wc -l | tr -d ' ')

  printf '%-5s %-18s %-4s %-9s %-11s %-6s %s\n' \
    "#$num" "$label" "$low" "$((total-unchecked))/$total" \
    "${filed:-?}" "$since" "$(basename "$f")"
done < "$LOCAL_TSV"

echo
echo "-- legend: CRITERIA = checked/total; SRC+ = commits to src/ since the issue was filed."
echo "-- A large SRC+ is the prompt to check whether landed code already satisfies the criteria."
echo "-- That check outranks every ordering heuristic: such an issue is next to VERIFY AND CLOSE, not next to build."

echo
echo "-- Blocked by (each reference resolved against current GitHub state):"
clear_list=()
while IFS=$'\t' read -r f num _ _ _ _ _; do
  [[ "$num" == NONE ]] && continue
  [[ "$(gh_state "$num")" == OPEN ]] || continue

  block=$(awk '/^## Blocked by/{p=1;next} /^## /{p=0} p' "$f")
  [[ -z "${block//[[:space:]]/}" ]] && { clear_list+=("#$num"); continue; }

  said_something=0

  # Read line by line, not ref by ref: a line that already says the blocker is
  # done ("punch-kick-cooldown.md (done - ...)") is correct prose, not drift.
  # Only a closed blocker the text still treats as pending is worth a finding.
  while IFS= read -r line; do
    [[ -z "${line//[[:space:]]/}" ]] && continue
    claims_done=0
    shopt -s nocasematch
    # "merged" is deliberately not in here: "PR #54 ... being merged" reads as
    # pending, and once that PR lands the line IS the drift worth reporting.
    [[ "$line" =~ (done|already|no[[:space:]]longer|^[[:space:]]*None) ]] && claims_done=1
    shopt -u nocasematch

    while IFS= read -r ref; do
      [[ -z "$ref" ]] && continue
      target=$(resolve_ref "$ref")
      if [[ -z "$target" ]]; then
        echo "   #$num -> '$ref': NO SUCH LOCAL FILE"; said_something=1; continue
      fi
      tnum=$(num_for_path "$target")
      case "$(gh_state "$tnum")" in
        OPEN)
          echo "   #$num BLOCKED by #$tnum ($(basename "$ref")) - still open. No rank; list it separately."
          said_something=1 ;;
        CLOSED)
          if [[ $claims_done -eq 0 ]]; then
            echo "   #$num STALE TEXT: #$tnum ($(basename "$ref")) is CLOSED but the file reads as pending:"
            echo "        > $(echo "$line" | sed 's/^[[:space:]]*//')"
            said_something=1
          fi ;;
        *) echo "   #$num -> '$ref': unknown GitHub state"; said_something=1 ;;
      esac
    done < <(echo "$line" | grep -oE '[A-Za-z0-9._/-]+\.md' | sort -u)

    while IFS= read -r pr; do
      [[ -z "$pr" ]] && continue
      prstate=$(gh pr view "$pr" --json state --jq .state 2>/dev/null || echo UNKNOWN)
      if [[ "$prstate" == MERGED ]]; then
        [[ $claims_done -eq 0 ]] && {
          echo "   #$num STALE TEXT: PR #$pr is MERGED but listed as a blocker"; said_something=1; }
      else
        echo "   #$num BLOCKED by PR #$pr - $prstate"; said_something=1
      fi
    done < <(echo "$line" | grep -oE '(PR |pull/)#?[0-9]+' | grep -oE '[0-9]+' | sort -u)
  done <<< "$block"

  [[ $said_something -eq 0 ]] && clear_list+=("#$num")
done < "$LOCAL_TSV"
[[ ${#clear_list[@]} -gt 0 ]] && echo "   No live blockers: ${clear_list[*]}"

echo
echo "-- Parent rollup (a parent with exactly one open child is tier-1 'finishes work in flight'):"
awk -F'\t' '$7!="none"{print $7}' "$LOCAL_TSV" | sort -u | while IFS= read -r prd; do
  open_children=(); closed=0
  while IFS=$'\t' read -r f num _ _ _ _ parent; do
    [[ "$parent" == "$prd" ]] || continue
    if [[ "$(gh_state "$num")" == OPEN ]]; then open_children+=("#$num"); else closed=$((closed+1)); fi
  done < "$LOCAL_TSV"
  printf '   %-50s %d closed, %d open %s\n' \
    "$(basename "$prd")" "$closed" "${#open_children[@]}" "${open_children[*]:-}"
done

if [[ $WITH_BODIES -eq 1 ]]; then
  echo
  echo "=============================================================="
  echo "OPEN ISSUE BODIES (rank from these, not from titles)"
  echo "=============================================================="
  while IFS=$'\t' read -r f num _ _ _ _ _; do
    [[ "$num" == NONE ]] && continue
    [[ "$(gh_state "$num")" == OPEN ]] || continue
    echo
    echo "############## #$num  $f"
    cat "$f"
  done < "$LOCAL_TSV"
fi
