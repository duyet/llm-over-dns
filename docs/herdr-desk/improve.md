# Desk playbook: improve

The self-improvement loop. This job makes the repository and its own
automation better; it does not ship product features. Product work belongs to
`desk:github-issues`.

Runs nightly. One theme per run, fully verified, then stop. A night that files
three good issues and lands one small fix beats a night that touches forty
files.

## Standing themes for this repo

Ranked. Take the top one that is still open.

1. **Silent-failure config.** `Config::from_env` must never accept a value
   that turns into a total outage with no log line. The recurring shapes here:
   an unvalidated numeric that `unwrap_or(default)`s away a parse error, an
   API key that is only checked for emptiness, a range the docs state but the
   code does not enforce. Each one produces a server that binds, logs
   `listening`, and then SERVFAILs or Refuses 100% of queries. A wrong value
   here must be a startup `Err`, not a `default`.
2. **The UDP budget.** A DNS answer must fit one UDP datagram. The chunker's
   `max_total_size` counts *text*, but every chunk becomes a TXT record with
   its own header, and the server enforces a byte cap on the serialised
   message. When those two disagree, a routine-length answer is truncated
   with the TC bit and the client gets nothing — and there is no TCP listener
   to retry over. Treat any change to chunk sizing, record counts, or the UDP
   cap as one atomic change with a test that encodes the wire bytes.
3. **Error context reaching the log.** `anyhow`'s `Display` prints only the
   outermost context; the cause chain needs `{:#}` or `{:?}`. Every
   `error!("...: {}", e)` in this repo is a candidate — a lost chain is why
   two separate advisories looked like one mystery.
4. **Test coverage.** The floor is 60% (`cargo tarpaulin --fail-under 60`).
   Real coverage, not the number: a test that asserts nothing, a test whose
   name claims a behaviour it never checks, a reliability mechanism (request
   timeout, concurrency cap, TTL) with no test at all. A `#[test]` that only
   asserts a constructor returned is worse than no test — it buys coverage
   and buys no information.
5. **Desk health.** `herdr plugin action invoke herdr-desk.status` (or
   `bun <plugin>/src/cli.ts status`). The `Fails` column non-empty, a `Next`
   of `-` (a cron that can never match), or a job whose manager is not live.
   A desk that is quietly failing is the most expensive possible finding,
   because every other job on this list depends on it.
6. **Drift between a rule and the code.** `CLAUDE.md`, this directory, and
   `.env.example` are a contract. `CLAUDE.md` documents 7 env vars in one
   place and the code reads more; a doc that says `CACHE_MAX_ENTRIES=0`
   disables the cache while the code and another doc say it means unbounded is
   a bug an operator will act on.
7. **Dead weight.** Code with no non-test caller, a comment that contradicts
   the code below it, a duplicated helper, a test asserting behaviour nothing
   uses.
8. **Gate gaps.** `master` had no branch protection, so nothing forced
   `All Checks Pass` before a merge. Check that the required gate is still
   wired and that no job was added to `ci.yml` without adding it to the
   `needs:` list of `All Checks Pass`.

## The loop

```
measure → pick the single top item → research → fix or file → verify → record
```

1. **Measure.** Produce the evidence before you claim anything. A file path, a
   `grep -n` hit, a failing command, a CI log line. No evidence, no finding.
2. **Pick one.** Rank by (certainty × blast radius) ÷ effort. If two items tie,
   take the one whose absence would keep biting.
3. **Research before you touch.** Read the code, the tests, and the docs that
   claim the old behaviour. If the right output is a decision rather than a
   diff, write `research-<n>.md` in the run dir and stop.
4. **Fix or file, not both.** A mechanical fix (stale doc, dead code, missing
   test for a rule that already exists) gets a PR. Anything needing judgement
   gets an issue with the evidence attached.
5. **Verify.** `cargo fmt --check`, `cargo clippy -- -D warnings`, and
   `cargo test` locally — all three are the same commands CI runs. For
   doc-only changes, state plainly in the PR that no runtime behaviour
   changed and let CI confirm. Do not run the full `tarpaulin` sweep locally
   unless you changed coverage itself.
6. **Record.** Update the knowledge note you touched, in the same change, and
   bump its date. A finding that is not written down will be rediscovered next
   month.

## Never

- Never open a PR that mixes a fix with a rename, a reformat, or a dependency
  bump. Those land as their own PR or not at all.
- Never "improve" by deleting a test to make a suite green, or by relaxing the
  60% coverage floor.
- Never weaken `cargo audit` to make a run pass — no blanket ignores, no
  pinning a vulnerable version. See `babysit-prs.md` for the right sequence.
- Never make `cargo test` require a network call or a real API key.
- Never print, log, or interpolate an API key into an assertion message.
  `main.rs` has a `mask_api_key` helper; new logging must go through it.
- Never let one run open more than 2 PRs. Breadth is how a repo stops being
  reviewable.

## Report

`changes.md` with: the theme, the measurement, what landed, what was filed
(issue numbers), what was deliberately left, and the next theme you would pick.
