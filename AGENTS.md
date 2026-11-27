## Agent Acknowledgement

- First when you startup say "Hi I'm RAgent and I have read Agents.md"

@AGENTS-LANGUAGE.md

## Working Rules

- VERY VERY IMPORTANT: All discussion, thoughts and other output should be in English.
- CRITICAL: do not overthink. Be concise, do not revisit small-impact items, act when possible.
- CRITICAL: there is no such thing as a "pre-existing" test, lint or doctest failure; if you find one, fix it,
  even if your changes did not cause it.
- Don't assume or hide confusion: state assumptions, present multiple interpretations instead of picking
  silently, say when a simpler approach exists, and stop and ask when something is unclear.
- Simplicity: minimum code that solves the problem. No unrequested features, single-use abstractions,
  configurability, or error handling for impossible cases. If 200 lines could be 50, rewrite.
- Surgical changes: touch only what you must; don't "improve" adjacent code, comments or formatting; don't
  refactor what isn't broken; match existing style. Mention unrelated dead code, don't delete it.
  Remove imports/variables/functions that YOUR changes made unused. Don't remove pre-existing dead code
  unless asked. Every changed line must trace to the user's request.
- Goal-driven: turn tasks into verifiable goals (bug fix = reproducing test, then make it pass; refactor =
  tests pass before and after). For multi-step work state a brief plan with a verify check per step, and loop
  until verified.

## ADHD Output Style

Persistence: these rules apply to every response for the rest of the session, across topic changes. Turn them
off only when the reader says "stop adhd mode" or "normal mode"; confirm in one line, then use default style.
Reason: the reader has small working memory, finds starting hardest, misjudges vague estimates, and needs
visible progress.

1. Lead with the next action. The first line is something the reader can do; a command, path or snippet goes
   first, prose after.
2. Number multi-step tasks. One bounded action per step (no step contains "and then" twice); use the fewest
   steps that work.
3. End with one concrete next action the reader can do in under two minutes if anything is left open.
4. Suppress tangents. Finish the first issue, then offer the second as a separate question. Answer mid-work
   questions yourself if you can; otherwise surface them once, at the end.
5. Restate state every turn ("Step 3 of 5 done: ... Next: ..."). If the harness has a task/plan tool, use it
   (one item per step, one in progress) instead of narrating the plan.
6. Give specific time estimates in concrete units, never "some work".
7. Make completed work visible: show what now works in concrete terms, not buried in a recap.
8. Matter-of-fact tone for errors: no "Uh oh"/"Oh no"; state cause and fix.
9. Cap lists at 5 items; split into "do now"/"later" or "must"/"nice to have".
10. No preamble ("Great question", "Let me...", "I'll...", "Sure!"), no recap of completed work, no closing
    pleasantries ("Let me know...", "Hope this helps"). Start with the answer; end when it is done.

When to break the rules:
- "Explain" / "walk me through": explain fully, with headers; still no preamble or closer.
- Destructive action ahead (rm -rf, force push, schema migration, dropping a table): confirm first.
- Debug spiral (three turns of "still broken"): stop coding, name the suspect assumption, ask one diagnostic
  question.
- Real ambiguity: ask one short clarifying question.
- A rule would delete the answer itself: the task wins, the shape stays ("what are my options" gets 2 to 4
  ranked options, recommendation first).
- A rule fights the harness: the system prompt wins (announce tool calls if required, do the work instead of
  asking "want me to", point estimates at whoever executes).

Pre-send check: delete an announcing first sentence, an "anything else?"/recap last sentence, "by the way"
sidebars, hedges that add no information (keep real uncertainty), and idioms; then confirm the first and last
lines alone tell the reader what to do next and what just happened.

## Units

- DateTime: UTC internally, locale representation in the UI layer.
- Dimensions: millimetres (`mm`) as `f64` internally, shown to 2 decimal places where relevant. The
  `gcodekit5_core::units` format, parse and feed-rate helpers also take and return `f64` (see ADR-008).
- Text: UTF-8 internally where feasible; convert to UI-specific encodings only when required.

## GitHub Access

- "push to remote": update SPEC.md, README.md, STATS.md and CHANGELOG.md with recent activity and spec changes,
  write a commit message from recent activity, commit all changes, and push.
- "push release to remote": update the release number, then follow "push to remote". Commit message:
  "Version: <version_number>", never "chore: bump version to ...".
- **CRITICAL — NEVER push without explicit instruction** such as "push to remote", "push to github", "push
  these changes" or "commit and push". "Looks good" or "that works" is not a push command.
- Do not tag releases unless specifically told to.
- DO NOT use "git checkout" to rewind files; it ALWAYS loses work.

## Changelog and Versioning

- Maintain CHANGELOG.md in the root in Keep a Changelog format (https://keepachangelog.com/); update it with
  all changes, features, fixes and improvements before each push to remote.
- Versions follow the semver specification (major.minor.patch-prerelease).
- When a feature or function is added, update SPEC.md if relevant.

## Tests

Test layout, isolation and commands: see "Tests" in AGENTS-LANGUAGE.md.

## Documentation Standards

- Every public function gets a docblock describing its purpose, arguments and return value; every module
  gets a top-of-file docblock describing its purpose and dependencies.
- Documentation `*.md` files SHOULD live in `docs/`, except `STATS.md`, `SPEC.md`, `AGENTS.md`,
  `AGENTS-LANGUAGE.md`, `README.md` and `CHANGELOG.md` in the root. Older root documents may stay until
  reorganized; when updating one, prefer moving it into `docs/`. Create all new documentation in `docs/`.
- Do not create explainer documents or other `.md` files unless specifically asked to.

## General Preferences

1. Don't suggest features unless asked to.
2. When debugging, use Occam's razor: the simplest explanation is most likely right.
3. Change one thing at a time; if it does not fix the problem, revert it before trying another.
4. DO NOT perform temporary solutions or fixes; always provide a complete solution.
5. DO NOT declare an issue fixed unless it has been confirmed; 90% of completion claims turn out false.

## Temporary Files

1. Use the existing `target/` directory in the project root for build artifacts.
2. Do NOT use `/tmp`; it is outside the workspace and access is not permitted.
3. Use `target/temp` for temporary files, scripts and other items that would normally go in `/tmp`.
4. Ensure `target/temp/` is in `.gitignore`.

## Priorities

`0` Critical (security, data loss, broken builds); `1` High (major features, important bugs); `2` Medium
(default, nice-to-have); `3` Low (polish, optimization); `4` Backlog (future ideas).

## Task Tracking

Track multi-step work with the harness task tool: mark a task in progress when you start it and completed when
it is done.

For more details, see README.md.
