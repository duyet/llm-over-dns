# Desk playbook: babysit PRs

Keep every open PR moving to merged. This job is the *conveyor*, not the
triage: it fixes CI, answers review comments, and arms auto-merge. It never
files new work and never starts a feature.

Runs every 30 minutes, so every step must be cheap and idempotent. If there
is nothing to do, write that down and stop — an empty run is a good run.

## Sweep

1. `git fetch origin --prune` and `git pull --ff-only origin master` in the
   main checkout. If the checkout is not a clean `master`, write the blocker
   in `summary.md` and stop.
2. `gh pr list --state open --limit 40 --json
   number,title,headRefName,isDraft,mergeStateStatus,reviewDecision,statusCheckRollup`.
   Group by what each PR needs next:

   | State | Do |
   |---|---|
   | Required check red | Fix it (below) |
   | Required green, no auto-merge | Arm it (below) |
   | Changes requested | Read the review, fix it or answer it |
   | Merge conflict | Fix it on the branch |
   | Stale (no update > 3 days) | Nudge, or close with a reason if dead |
   | Draft, abandoned | Close with a reason, link the issue |
   | Empty diff vs master | Close as already-landed — never merge a no-op |

3. **The required gate is `All Checks Pass`**, which in
   `.github/workflows/ci.yml` `needs: [fmt, clippy, audit, test, coverage]`.
   So the five jobs that actually block a merge are:

   - `Format Check` — `cargo fmt --check`
   - `Clippy Lint` — `cargo clippy -- -D warnings`
   - `Security Audit` — `cargo audit`
   - `Test Suite (stable)` and `Test Suite (beta)`
   - `Code Coverage` — `cargo tarpaulin --fail-under 60`

   Everything else on a PR — `Build and Push Docker Image`,
   `Test Docker Image`, `Trivy`, `GitGuardian Security Checks`, the two
   `Socket Security` checks — is informational. Never hold a PR for one of
   them, and never spend a run fixing one.
4. **Arming auto-merge.** Only when `All Checks Pass` is green and the PR is
   not a draft:

   ```sh
   gh pr merge <n> --auto --squash
   ```

   Squash is the house style: every merged commit on `master` is a squash
   merge with the PR number in the subject.

## Fixing a red `Security Audit` — read this before editing any lockfile

`cargo audit` scans the **whole resolved `Cargo.lock`**, not the diff. A new
RustSec advisory therefore turns *every* open PR red at once, including PRs
that touch no vulnerable crate. This repo has been bitten by exactly that
twice (`RUSTSEC-2026-0258` in `h2`, `RUSTSEC-2026-0285` in `rustls`).

The failure is almost never the PR's fault. Check before you touch anything:

```sh
git stash -u && git checkout master && git pull --ff-only
cargo audit                       # or: gh run view <master-run> --log-failed
```

- **Master is also vulnerable** (the common case). The PR is innocent. The
  fix is to remediate on `master` *first*, then bring the blocked branches
  forward:

  ```sh
  cargo update -p <crate>          # or --precise <fixed version>
  cargo audit                      # must be clean before you push
  # open a PR against master, merge it, then:
  gh pr update-branch <n>          # per blocked PR
  ```

  Never "fix" a dependency PR by pinning the vulnerable version inside that
  PR. That reintroduces the advisory on `master` the moment it merges.
- **Only the PR is vulnerable** — the PR moved a crate *into* the advisory
  range, or its lockfile is stale. Fix it on the branch and rebase.
- **No fixed version exists yet.** Do not weaken the gate. Open an issue
  quoting the advisory ID, the affected crate, and whether the code path is
  reachable; leave the PR red and say so in `changes.md`.

Prefer `cargo update -p <crate>` over hand-editing `Cargo.lock`. The advisory
DB moves constantly — re-read the advisory page
(`https://rustsec.org/advisories/<ID>.html`) for the real patched version
rather than guessing.

## Dependency PRs

Dependabot owns these (`.github/dependabot.yml`); do not open competing bumps.

Renovate (`renovate.json`) was also configured and produced byte-identical
duplicate PRs. It is being retired — do not reopen that decision in a drive-by.

For a Dependabot PR that is red only because of `Security Audit`, the fix is
the `master`-first sequence above. If it is red on tests, read the log before
assuming the bump is safe: a `Cargo.lock` change can pull a transitive crate
that changes behaviour.

## Locally reproduced checks

Prefer these over a blind merge attempt, and skip the slow ones entirely —
CI runs the same commands:

```sh
cargo fmt --check                 # instant
cargo clippy -- -D warnings       # after one build
cargo test                        # needs no API key; see CLAUDE.md
cargo audit                       # needs `cargo install cargo-audit`
```

`cargo test` must stay hermetic. If a change makes it require a network call
or a real API key, that is a bug in the change, not a reason to set a secret
in CI. To exercise the server by hand use `DNS_PORT=5353 cargo run` — never
bind 53 in a test.

## Review comments

A bot reviewer (`coderabbitai[bot]`, Sourcery, Gemini) that leaves
`CHANGES_REQUESTED` may be dismissed **only** when every point is genuinely
fixed or stale, all required checks are green, the branch is current with
`master`, and the bot did not re-review after your push. Never dismiss a
human review.

## Worktree hygiene

After a PR merges and nothing remains for it:

```sh
git worktree remove ~/.herdr/worktrees/llm-over-dns/<name>
git branch -D <branch>          # only after the branch is merged
```

Before removing, prove the branch landed — a squash merge changes the sha, so
`git branch --merged master` is not enough:

```sh
git diff <branch> origin/master -- $(git diff --name-only \
  $(git merge-base origin/master <branch>) <branch>)   # empty = landed
```

**Never remove a worktree with uncommitted work.** Commit it to its own branch
first (a `wip(...)` commit is fine, local only) so the work cannot be lost.

## Report

Write `changes.md`: armed / fixed / nudged / closed / skipped + why, and any
worktree added or removed. Then toast:

```sh
herdr notification show "llm-over-dns PRs" --body "{{runDir}}/changes.md"
```

## Stop conditions

Stop and report instead of pushing harder when:

- a required check has been red for 3 consecutive runs — that is a real bug,
  so open an issue with the failing log rather than a fourth fix attempt
- a `Security Audit` failure has no patched version upstream
- more than 3 PRs are red at once — fix them in order of age, not breadth
- `master` itself is failing CI — fix `master` and stop babysitting PRs
