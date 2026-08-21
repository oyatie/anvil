//! The Product seat: a change must state its bet and its acceptance bar.
//!
//! ADR-0002 Discover §1 — "Product. Job: the bet and the acceptance bar.
//! Artifact: written problem + done-when. Measurement: Quality cannot sign off
//! without it." The sequence section adds the consequence: "Quality sign-off
//! must fail if Product's bar is missing."
//!
//! # Why absence is `Failed`, not `NotMeasured`
//!
//! `NotMeasured` exists for a gate that was asked to judge and had nothing to
//! read — no telemetry endpoint, no shape spec adopted. That is not this. The
//! artifact is authored on the change under review, by the person opening it.
//! A change with no bar has not withheld evidence from the gate; it has failed
//! to produce the artifact. Reporting that as `NotMeasured` or `Warning` would
//! let every change certify while the seat measures nothing, which is the
//! "named gate, no measurement" pattern the ADR's honesty law forbids.
//!
//! This is the whole ballgame, so it is pinned on the input shapes where a
//! hurried gate is most tempted to fail open: a body that uses none of the
//! headings the gate happens to recognise, and a body whose headings are real,
//! well-filled and simply not the Product artifact. Ordinary prose, an unfilled
//! template, and a `## Summary` over a `## Test plan` are the commonest real
//! pull request bodies there are, and every one of them must be `Failed` — see
//! `a_real_pull_request_body_with_no_bar_fails_closed`. A test plan says what
//! the author ran; it is not a statement of what done looks like, and reading
//! it as one certifies a very large fraction of real changes with no bar.
//!
//! # Why an empty heading must fail
//!
//! A gate that accepts `## Done when` followed by nothing is a shallow check
//! wrapped as a measurement: it rewards pasting a template. The same is true of
//! `TBD`, `N/A`, `todo`, an unticked checkbox, an unfilled template comment,
//! and a bullet with nothing after the dash. The measurement is the content,
//! not the marker.
//!
//! # How this suite forces the gate to read the content
//!
//! Two earlier revisions of this file were each vacuous against a different
//! cheap gate, and the fixtures now close both off. Every property below is a
//! constraint on the fixtures, not on the implementation.
//!
//!   1. **The two sets overlap in length at both ends.** The failing set
//!      reaches above and below the passing set — the shortest failing body is
//!      empty and `long_prose()` under a placeholder is over a kilobyte — and
//!      the passing set now reaches above the failing one too:
//!      `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact` carries a
//!      real acceptance bar at the far end of the same background written once,
//!      eight times and sixty-four times over, and the largest is longer than
//!      every body in this file that must fail AND longer than
//!      `GITHUB_MAX_BODY_CHARS`. Both halves are needed and the file used to
//!      have only one. Mutual bracketing rules out a *threshold* on total
//!      length; it does not rule out a *truncation*, and truncation is the
//!      direction that produces a fabricated accusation rather than a false
//!      green. Review verified the hole twice, each time against a working
//!      reference: `&pr_body[..450]` and `pr_body.lines().take(18)` passed the
//!      revision before last, and `&pr_body[..2100.min(pr_body.len())]` and
//!      `pr_body.lines().take(50)` passed the last one, because no must-pass
//!      body put its bar further in than that. Three magnitudes an order of
//!      magnitude apart, each with its emptied mirror, close every clamp
//!      between them — but only between them, and the previous revision
//!      overstated what that reached. Review measured the residual in both
//!      units: `pr_body.lines().take(3000)` and `.take(5000)` passed all
//!      forty-one tests, because x64 of a thirty-eight-line background is only
//!      about twenty-five hundred lines while a GitHub body may hold 65,536
//!      characters and therefore up to 65,536 LINES; and a byte clamp at
//!      200,000 passed too, because GitHub's limit is 65,536 CHARACTERS and a
//!      character may be four bytes. So the family gains a fourth member built
//!      the other way round, out of sixty-six thousand short lines
//!      (`TALL_BACKGROUND_LINES`), and the fixture invariants now bound the
//!      passing side in both units against the real limit:
//!      `GITHUB_MAX_BODY_BYTES` in bytes and `GITHUB_MAX_BODY_CHARS` in lines.
//!      A clamp that survives this family sits above every body the guard layer
//!      can be handed, in either unit, and truncates nothing.
//!   2. **Length is not monotone per section, in either direction either.**
//!      `"TODO: write the acceptance criteria here"` is a thirty-nine-byte
//!      placeholder that must fail, so no minimum section length can be the
//!      measurement — but that only rules out a length rule used *instead of* a
//!      content check, not a floor bolted *on top of* one, and
//!      `core.chars().count() >= 9` passed every behavioural test in the
//!      previous revision. `SHORT_REAL_CONTENT` closes it from below:
//!      `"- p99<5ms"` has no space in it at all and `"- 빌드 통과"` is five
//!      characters of Hangul in thirteen bytes, and
//!      `content_passes_however_few_characters_and_words_it_takes` asserts each
//!      of them is shorter than the longest must-fail string in characters AND
//!      in words before running them through both sections.
//!   3. **Both sections are held to one standard, in every marker SHAPE.**
//!      Every placeholder family runs through the problem position and the
//!      done-when position, so an implementation cannot screen the bar for
//!      substance and settle for "non-empty" on the bet. It also runs through
//!      every shape the marker takes, which is where the previous revision
//!      broke its own rule: the inline colon form (`## Problem: <text>`) was
//!      pinned as PASSING in four places and had no must-fail mirror anywhere,
//!      and the inline done-when form had exactly one must-fail token out of
//!      six families. The inline form is a separate code path — the content is
//!      on the marker line rather than under it — so the cheapest predicate
//!      that satisfied every other fixture read anything after the colon as
//!      content, and certified `## Done when: #4192` and `## Problem: TBD`.
//!      `a_deferral_on_the_marker_line_itself_fails_in_both_sections` runs one
//!      representative of every must-fail family through the inline position
//!      and through the no-blank-line position, in both sections, with the
//!      passing mirrors kept and widened so the fix cannot swing the other way
//!      into rejecting `## Done when: p99 < 5ms`.
//!   4. **The must-fail content is not enumerable.** A previous revision drew
//!      every must-fail fixture from one fifteen-entry table, so a hardcoded
//!      exact-match copy of that table was a complete implementation — and it
//!      shipped a gate that passed a done-when reading `TBD.` or `WIP`.
//!      `derived_deferrals()` multiplies stems by trailing punctuation, letter
//!      case and bullet wrapping into hundreds of strings that appear nowhere
//!      as literals; `PHRASE_DEFERRALS` adds deferrals sharing no prefix with
//!      anything in `PLACEHOLDERS`; and `UNICODE_BLANKS` adds sections that a
//!      `trim().is_empty()` check reads as substantive.
//!   5. **No body may fail open.** Every fixture that carries neither artifact
//!      reaches `expect_failed`, never `assert_ne!(.., "Passed")`, so
//!      `NotMeasured`, `Warning` and `Errored` are rejected everywhere.
//!   6. **Section boundaries are falsified in both directions.** A previous
//!      revision contained no body with a third section anywhere, so an
//!      extractor that ran the done-when to the next `"\n## "` was never
//!      falsified: an empty `## Done when` followed by `**Testing**` came back
//!      `Passed`, which is the pasted-template defect in the shape real
//!      templates produce. `a_third_section_does_not_hide_an_empty_one` places
//!      a real third section after an empty and after a deferred section at
//!      every heading weight that terminates one, and its passing counterpart
//!      places one after a genuine bar, so the fix cannot degenerate into
//!      "ignore everything after the marker". In both directions means in both
//!      directions ON THE HEADINGS THE FAMILY RUNS, and two spellings are
//!      deliberately not among them: a colon-terminated lead-in and a heading
//!      deeper than the marker are pinned as CONTENT, not as boundaries, and
//!      each of those decisions buys a false green that is stated in "What this
//!      suite does NOT close" below rather than left to be inferred from a doc
//!      comment about a different fixture. Both are decisions with vetoes, not
//!      oversights: see `BOUNDARY_HEADERS`,
//!      `a_short_colon_terminated_lead_in_line_is_writing_not_a_section_boundary`
//!      and `a_heading_deeper_than_the_marker_is_content_inside_the_section`.
//!   7. **The passing side is wide enough not to block everyone.** See the next
//!      section: a suite whose only passing bodies spelled the marker one exact
//!      way admitted a gate that fails closed on nearly every real change,
//!      which is the fabricated accusation at 100% incidence.
//!   8. **Sections whose lines disagree are pinned, in both directions.** The
//!      revision before this one was exhaustive on *single-content* sections —
//!      all real, all deferral, all blank — and silent on mixed ones, so a gate
//!      reading only the first non-blank line of a section passed every test
//!      and then rejected a three-item bar whose first line was an empty
//!      checkbox. It also passed a gate that rejected any section *containing*
//!      an invisible character, which fails every artifact pasted out of a
//!      document editor. Real content beside a deferral, beside an invisible
//!      character and beside a colon-terminated lead-in are now all pinned as
//!      passing, and the all-deferral counterparts as failing, so the rule the
//!      suite means — strip what is not content, then judge what is left — is
//!      the only rule that satisfies it.
//!   9. **Every must-fail token is also pinned INSIDE real content.** This is
//!      the same property as (8) applied to the vocabulary rather than to the
//!      line, and the revision before this one had it for the invisible
//!      characters and the deferral prefixes and for nothing else. Three
//!      must-fail families were pinned only as whole sections, so a gate could
//!      satisfy each of them by rejecting on sight, and each rejection then
//!      blocked an ordinary pull request:
//!      * the **checkbox**. `"[ ]"`, `"[x]"`, `"- [x]"`, `"- [ ] "` and `"- [ ]"`
//!        all had to fail, and nothing anywhere required `- [ ] p99 < 5ms` to be
//!        read as content — so a gate treating the checkbox prefix as announcing
//!        a deferral the way `TODO: ` does was green across the file and then
//!        rejected the single commonest spelling of an acceptance bar there is,
//!        the one every PR template produces.
//!      * the **pointer**. No passing fixture contained `http`, a bare domain or
//!        a `#1234`, so rejecting any line *containing* one satisfied
//!        `a_pointer_to_somewhere_else_is_not_the_artifact` and then killed a bar
//!        that cites the panel it will be checked on and a body that opens
//!        `Fixes #4192:`. Same shape for `PHRASE_DEFERRALS`:
//!        `section.contains("same as above")` satisfied the derived family and
//!        rejected a bar saying the retry budget behaves the same as above.
//!      * the **template comment**, IN BOTH ITS SHAPES. It was pinned only as a
//!        whole section, so a section-level `!s.contains("<!--")` passed
//!        everything here and then failed the commonest filled-in template body
//!        there is — a prompt comment left in place with the author's text typed
//!        under it. And every comment fixture in the file opened and closed on
//!        ONE line, so the only rule the suite required was line-local:
//!        `t.starts_with("<!--") && t.ends_with("-->")`, which is what an
//!        implementer writes once property (8) has made `is_content` per-line.
//!        Handed the multi-line form GitHub's own template documentation uses,
//!        the inner prompt line matches neither delimiter, both sections read as
//!        carrying content, and the completely unfilled template certifies whole.
//!        Both forms now have both halves — see `MULTILINE_PROMPT_PROBLEM`,
//!        `an_unfilled_template_whose_prompts_are_multi_line_comments_fails_closed`
//!        and its passing mirrors.
//!
//!      Every one now has its mirror, so the rule those pairs state together —
//!      a section whose whole content is the token is no artifact; the token
//!      beside real content does not erase the content — is the only one that
//!      satisfies the file.
//!  10. **The marker is a heading line, not a phrase.** Until this revision no
//!      passing fixture contained the words "problem" or "done when" anywhere
//!      except on a marker line, so `normalise(line).contains("done when")` —
//!      the cheapest way to satisfy the marker cross-product — was unfalsified
//!      in both directions. Taking the first match rejects a body whose summary
//!      paragraph mentions the problem; taking the last swallows a later
//!      paragraph as the acceptance bar of an empty section.
//!      `the_marker_is_a_heading_line_not_a_phrase_anywhere_in_the_prose` pins
//!      both — and pins the last-match half properly only because the
//!      marker-bearing prose sentence is no longer the final line of the body.
//!      In the revision before this one it was, in both fixtures, so the
//!      spurious section a `contains` predicate opens after it was empty either
//!      way and the wrong gate reached the right verdict by accident. Review
//!      reproduced it: the any-over-sections form of that predicate certified an
//!      empty `## Done when` whose words were supplied by a later `## Rollout`
//!      paragraph, and passed every behavioural test in the file while doing it.
//!      `assert_the_marker_prose_is_followed_by_content` now pins the fixture
//!      shape the family depends on.
//!  11. **The marker's own formatting is pinned in all three spacings.** Every
//!      passing body used to put a blank line between the marker and its
//!      content, so `**Done when**` above a list that starts on the next line,
//!      and `## Done when: p99 < 5ms` with the bar on the marker's own line,
//!      were unpinned — and the boundary rule this file forces pushes an
//!      implementer straight into rejecting both. The marker family now runs
//!      all three, with the empty and deferred mirrors under each.
//!  12. **The deferral vocabulary is bounded from below as well as above.**
//!      `PLACEHOLDERS` forces `"TODO: write the acceptance criteria here"` to
//!      fail while `derived_deferrals()` forbids an enumeration, and the
//!      cheapest implementation satisfying both is a prefix test on the
//!      normalised line. That gate rejects `- Navigation completes in under
//!      200ms` and a problem opening `Native TLS …`.
//!      `real_content_that_merely_begins_with_a_deferral_stem_passes` forces
//!      token-level matching, and pins the same-line case where a deferral
//!      token opens a line of real content.
//!  13. **Every family that can be line-ending-sensitive runs over both.** The
//!      boundary and marker families are where CRLF actually bites: a trailing
//!      `\r` defeats the `ends_with` test that recognises `**Testing**`,
//!      `Testing:` and `**Done when**`, and `body.split('\n')` instead of
//!      `body.lines()` is an entirely ordinary way to write an extractor. Under
//!      LF alone the headline defect this file exists to close stayed open in
//!      the exact line endings the GitHub web UI submits.
//!  14. **The determinism rule names effects, not spellings.** The source scan
//!      inside `the_verdict_depends_on_nothing_but_the_change_it_was_handed` used
//!      to be a whitelist of import prefixes, and three rounds of review found a
//!      correct implementation turned red by it over an import with no effect
//!      behind it — `regex`, then the `unicode_*` crates, then `std::mem` in a
//!      line-splitting loop, accused of reading "a file, an environment
//!      variable, a clock or the network". A guard that misreads what it guards
//!      is worse than none, and a settled specification test that has to be
//!      edited mid-implementation is the one thing this project's method
//!      forbids. `impure_import` states the rule as the denylist it always was:
//!      the `std` subtrees that reach outside the process, the machine or the
//!      moment, plus any third-party crate that is not on a short pure-text
//!      list. It is exercised on thirty-eight paths, both sides, before it is
//!      trusted.
//!  15. **Whitespace on the marker line is pinned on the PASSING side.** Every
//!      marker literal is exactly terminated and `as_eol` only ever appends
//!      `\r`, so before `MARKER_PADDINGS` no must-pass body anywhere carried
//!      `## Done when   ` or `**Done when** `. That is property (13)'s defect
//!      class left open for the character next to `\r`, and the file's own CRLF
//!      commentary steered an implementer at the narrow fix
//!      (`trim_end_matches('\r')`) rather than a full trim. Review verified it:
//!      a `heading_text` that trims only `\r` passes all 84 marker fixtures
//!      under both line endings and then reports BOTH artifacts missing from a
//!      complete, well-written body whose author left a space after the
//!      heading, or used markdown's two-trailing-spaces line-break idiom. The
//!      marker family now runs the whole cross-product over trailing, leading
//!      and surrounding whitespace, with the empty and deferred mirrors under
//!      each, and the inline family runs the extra space after the colon.
//!  16. **A body need not end in a newline.** Every must-pass body used to,
//!      without exception; the unterminated shape appeared only in
//!      `awkward_bodies`, which is entirely on the failing side, and whose own
//!      comment records that GitHub bodies routinely have no trailing newline.
//!      That is property (9) applied to the line ending: a shape pinned only
//!      among must-fail inputs can be rejected on sight, invisibly. `let nl =
//!      rest.find('\n')?` is an ordinary way to read the line a marker sits on
//!      and returns `None` exactly when the marker line ends the body.
//!      `a_body_that_does_not_end_in_a_newline_is_still_the_artifact` puts the
//!      shape on the passing side in both orders, both line endings and the
//!      inline form, with the emptied mirrors under each.
//!  17. **The wiring guards judge what happens to the measurement's INPUT and
//!      to its RESULT, not only what survives the call's deletion.**
//!      `unmeasured_alternatives_in` cuts the whole `judge(..)` expression out
//!      before it looks, so it was blind on both sides of the call.
//!      `judge(&pr_body[..2000.min(..)])` reinstates the truncation
//!      that (1) exists to close, one layer up where `judge` stays perfectly
//!      correct; `judge(&pr_body).softened()`, mapping `Failed` onto
//!      the acceptable `Warning`, certifies every change from one method call
//!      to the right of both the conditional and the reassignment guards.
//!      `truncated_argument` and `post_processing_after` close them, and
//!      `assignments_to` now reads a write to the field on ANY receiver
//!      (`report.product_bar_status = ..`, the idiom report.rs already uses for
//!      a neighbour) rather than only a bare-identifier reassignment. The
//!      determinism scan reads the whole module CLOSURE the gate can reach —
//!      the `product_bar*` seeds, plus every file any of them imports, to a
//!      fixed point — because scanning one hardcoded path made the whole
//!      property escapable by moving the parser into a sibling module, and
//!      scanning a filename prefix left it escapable by NAMING that sibling
//!      anything else. `impure_import` returns `None` for every `crate::`,
//!      `self::` and `super::` path and has to, so the delegation is invisible
//!      by design and the file at the other end of it has to be opened. Both
//!      halves run over the closure and both are exercised on a two-file
//!      delegation fixture — one sibling reaching for `std::fs`, one reaching
//!      only for `std::fmt` — before either is trusted. `without_string_literals`
//!      no longer
//!      blanks the rest of the file out from under that scan when it meets a
//!      `'"'` char literal or a raw string ending in a backslash. Every one of
//!      those rules is exercised on both sides in
//!      `assert_the_wiring_parsers_read_a_real_wiring`, because a guard that
//!      misreads a correct wiring is worse than no guard.
//!  19. **The wiring guards' universe is the whole value chain, not one file
//!      and one call site.** The rules in (17) were applied only to text inside
//!      `src/pre_merge_guard/evaluator.rs`, so every one of them was open one
//!      line up or one hop out, and review measured all three against a working
//!      reference gate:
//!      * **The call site.** `truncated_argument` and `shadowed_argument` never
//!        ran on the argument the *pipeline* passes. Changing it from `body,` to
//!        `&body[..body.len().min(4000)],` gave `41 passed; 0 failed` —
//!        `root_ident` is still `body`, the position check still lines up, and
//!        `judge` stays perfectly correct. A named local
//!        (`let body_excerpt = &body[..2000];`) slipped through the same way, so
//!        `shadowed_argument` now follows the value under a new name as well as
//!        under a shadow. `assignments_to` had the same blind spot:
//!        `cert_report.product_bar_status = GateStatus::Warning(..)` written in
//!        the pipeline after the call was invisible to every guard.
//!      * **Around the call rather than after it.** `post_processing_after`
//!        reads only what FOLLOWS the closing paren and treats `)` as clean,
//!        and cutting the call out of `soften(product_bar::judge(pr_body))`
//!        leaves `soften( )`, which held none of the four entries in the
//!        denylist that used to stand in for this rule.
//!        `residue_beyond_the_measurement` replaces the denylist with the rule
//!        itself: once the call and the syntax that binds it are removed,
//!        nothing may be left. `41 passed; 0 failed` before, with every change
//!        certifying because `Warning` is `is_acceptable()`.
//!      * **One hop further up the chain.** The argument for forcing the body
//!        through a parameter — "a parameter is not a grep; it can hold only
//!        what the caller passed" — applies just as hard to
//!        `execute_pr_review`'s own `body: &str`, and there a caller was
//!        already passing `""`: the outage-recovery startup sweep at
//!        `src/cli/server.rs`. Wiring the gate arms it, and every pull request
//!        certified through that path is then told it wrote neither artifact.
//!        `every_caller_of_the_review_pipeline_hands_it_the_change_body`
//!        enumerates every call site under `src/` and holds each to an effect
//!        rule, because the correct call sites do not agree on one spelling:
//!        three reach the body through a field, one through a local.
//!      * **The binding at that hop, not its spelling.** Every rule in the
//!        bullet above reads the ARGUMENT's text, and none of them asked what a
//!        local at that position was bound TO — so `shadowed_argument`, the rule
//!        that follows the value one line up, was applied at the pipeline's own
//!        call site and not at its callers'. `let pr_body = String::new();`
//!        written above `src/cli/server.rs`'s call, and `&pr_body` passed, made
//!        every assertion pass: the argument is a plain local whose name says
//!        body, carries no literal and no truncating token. The sweep still
//!        handed the gate the empty string, so every pull request certified
//!        through that path was told it wrote neither artifact — the same
//!        100%-incidence fabricated accusation the test exists to prevent,
//!        reinstated one line above the call it guards. `caller_binding_defect`
//!        follows the value: when the argument is ROOTED at the identifier that
//!        names the body, the binding in effect at the call must name the body
//!        too, and must be neither a literal nor an empty-value constructor.
//!        Both sides are exercised, because the one correct call site that
//!        reaches the body through a local — `let pr_body =
//!        pr.body.unwrap_or_default();` — must stay clean.
//!  18. **The change's body reaches the gate as a PARAMETER.** The guards used
//!      to accept a second route — a body field on `PrDiffContext`, the
//!      change-under-review struct the evaluator already takes — because
//!      forcing a sixty-ninth positional argument decides the implementer's
//!      surface for them. Review found that route's last link unassertable:
//!      nothing readable from source establishes that `prepare_pr_diff` STORES
//!      the body it was handed, so `pr_body: String::new()` satisfies every
//!      wiring assertion, leaves `judge` perfectly correct, and reports both
//!      artifacts missing on 100% of pull requests. A parameter is not a grep:
//!      it can hold only what the caller passed. See
//!      `evaluator_body_parameters`, and open_questions for the veto.
//!
//! # What this suite does NOT close
//!
//! Duplicate-marker precedence is deliberately OPEN. A body that carries two
//! `## Done when` headings — a filled one and a leftover empty one — is not
//! pinned either way: an implementation that reads only the first occurrence of
//! each marker, one that reads only the last, and one that accepts the artifact
//! if ANY occurrence carries content all pass this file. The `awkward_bodies`
//! duplicates are built so the verdict is unambiguous whichever rule is chosen,
//! so nothing here is vacuous on account of it — but nothing here decides it
//! either, and an implementer should not read the silence as an accident. It is
//! listed in open_questions.
//!
//! TWO FALSE GREENS ARE BOUGHT BY THE TWO BOUNDARY DECISIONS, and they are
//! written down here rather than only in the doc comment of the constant that
//! creates them. A false green a reader has to infer from a comment about a
//! different fixture is the undisclosed silence this file calls the worse half.
//!
//! The first is what taking `"Testing:"` out of `BOUNDARY_HEADERS` costs. This
//! body is pinned nowhere and an implementation may reach either verdict on it:
//!
//! ```text
//! ## Problem
//!
//! <a real problem statement>
//!
//! ## Done when
//!
//! Testing:
//!
//! Ran `cargo test --all` locally and re-ran the canary suite twice.
//! ```
//!
//! Under the decision that a colon-terminated line is never a boundary, the
//! done-when section reads as `["Testing:", the testing prose]`, `any(is_content)`
//! is true and the bar is reported PRESENT. That is this file's headline defect —
//! a pasted template with the middle section skipped, certified — in a shape
//! real templates produce, and its incidence is every author who writes a
//! colon-terminated lead-in instead of a heading for the section after an
//! unfilled one. It is not pinned as `expect_passed`: asserting the false green
//! would forbid a better implementation from closing it. The VETO, if a human
//! decides the trade is wrong: restore `"Testing:"` to `BOUNDARY_HEADERS` and
//! flip the blank-line fixtures in
//! `a_short_colon_terminated_lead_in_line_is_writing_not_a_section_boundary` to
//! `expect_missing` — which costs a rejected `Acceptance criteria:` above a real
//! list of bullets, the other half of the same trade.
//!
//! The second is what taking `"### Testing"` out of `BOUNDARY_HEADERS` costs,
//! and it is the same shape one heading weight over: an empty `## Done when`
//! above `### Testing` and real testing notes. Under the decision that a heading
//! deeper than the marker is nested content, the notes are inside the done-when
//! section and the bar is reported present. Incidence is lower than the first —
//! templates that skip a section usually head the next one at the same depth,
//! which `## Testing` still terminates — but it is the same class. It is not
//! pinned either way. The VETO: put `"### Testing"` back into `BOUNDARY_HEADERS`
//! and flip the `expect_passed` fixtures in
//! `a_heading_deeper_than_the_marker_is_content_inside_the_section`, which costs
//! a rejected acceptance bar written under `### Criteria`.
//!
//! One more combination is deliberately open, and it is recorded here for the
//! same reason: an inline deferral on a colon marker line WITH real content
//! under it — `## Done when: TBD` followed by a genuine, checkable bar. Reading
//! only the inline remainder makes that a failure; reading the union of the
//! marker line and the lines below it makes it a pass; and both readings satisfy
//! every fixture in this file. Review found it unpinned and undisclosed, which
//! is the worse half: the silence now says so. Listed in open_questions.
//!
//! Truncation IS closed, in every place it can be written. At the two call
//! sites — the pipeline's argument to the evaluator, and every caller's argument
//! to the pipeline — `truncated_argument` and `truncating_token_in` forbid
//! slicing the argument, and `shadowed_argument` forbids a `let` that clamps the
//! value one line above the call, whether it shadows the parameter or gives the
//! clamped value a new name. Inside `judge`, the close is a fixture property
//! rather than a source scan:
//! `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact` runs the complete
//! artifact at four magnitudes, each with its emptied mirror, so a clamp at any
//! fixed byte count or line count must sit above the largest fixture to pass the
//! passing side and below the smallest to fail the failing side, and no number
//! does both. The passing side clears `GITHUB_MAX_BODY_BYTES` in bytes and
//! `GITHUB_MAX_BODY_CHARS` in lines — the largest body GitHub can deliver, in
//! each unit — so a clamp that does survive this file is above every body the
//! guard layer can be handed and cannot truncate anything. Every step of that
//! was measured rather than argued: the revision before last conceded one
//! magnitude and `&pr_body[..2100.min(pr_body.len())]` passed; the last one
//! pinned three magnitudes of PROSE and `pr_body.lines().take(5000)` and a byte
//! clamp at 200,000 both passed.
//!
//! Character count and byte count are also deliberately decoupled: the Korean
//! fixtures are short in characters and long in bytes, so a heuristic in either
//! unit fails one of them.
//!
//! # Why the measurement is a function and not a string
//!
//! `product_bar::missing_artifacts` returns which halves of the artifact are
//! absent; `judge` renders the verdict and the message from it. The tests
//! assert the set, and assert the message in three places: positively on the
//! whole message (it must name each missing artifact), negatively on the
//! *residue* (the message with the body's own lines subtracted must not name an
//! artifact the author did write), and for distinctness on the residues of the
//! three shapes of absence.
//!
//! The residue is what closed the last hole here. An earlier revision asserted
//! the negative as a raw substring ban — "a missing-bar message must not
//! contain the word problem" — which turned a correct, helpful implementation
//! red for quoting the offending section back at the author. Dropping the ban
//! entirely then left the whole contract satisfiable by one constant string
//! naming both artifacts with the body echoed after it: every positive
//! assertion holds, and the three messages differ because the three *bodies*
//! differ. Subtracting the body first keeps quoting legal — it is removed
//! before the rule is applied — while holding the gate to what it said on its
//! own account. See `message_residue` and
//! `three_shapes_of_absence_produce_three_distinct_messages`.
//!
//! Subtracting the body cuts both ways, and the revision before this one was
//! cut by it. Every `expect_missing` with one artifact PRESENT wrote that
//! present section under the byte-identical heading `## Problem` or
//! `## Done when` — the marker cross-product varies the spelling only on the
//! MISSING side — so a constant message spelling the two artifacts with those
//! two literals was subtracted out of the residue for exactly the bodies where
//! the negative rule bites. Positive naming held (`"## Problem"` lowercases to
//! contain `problem`), the negative held because the surviving heading was in
//! the body, and the three "distinct" messages differed only by which heading
//! got subtracted. The author whose problem statement was present was still
//! told to write one. `the_message_holds_its_ground_however_the_surviving_section_is_headed`
//! runs the one-artifact-missing families over all thirteen heading spellings
//! on the surviving side: no constant can embed thirteen spellings, so the
//! residue now really does hold the gate to what it measured.
//!
//! # The marker vocabulary is open above a floor; the marker *formatting* is not
//!
//! Which words announce the two sections is left to the implementer: an
//! implementation that also recognises `## Acceptance criteria`, `## Why`, a
//! YAML block or unheaded prose passes unchanged, because no test here requires
//! a body that genuinely states both artifacts to fail.
//!
//! Open ABOVE A FLOOR, and the floor is a decision this file makes on the
//! implementer's behalf: the English words "Problem" and "Done when" must be
//! among the markers recognised, in the thirteen formattings
//! `the_same_two_words_are_the_marker_however_the_author_formats_them` runs.
//! Every passing fixture in the file is headed with one of them, so nothing here
//! obliges a gate to recognise a body headed `## 문제` / `## 완료 조건`, even
//! though the Korean CONTENT fixtures require the content itself to be read in
//! any script. That is a real product decision — an English-only marker
//! vocabulary — and it belongs in open_questions rather than only in these docs.
//!
//! With ONE bound, added this revision, and it is a bound on the artifact and
//! not on the vocabulary: a section that says what the author RAN is not a
//! statement of what done looks like. `## Test plan` over `- cargo test --all`
//! and `## Testing` over test notes must both leave the acceptance bar missing
//! (`a_real_pull_request_body_with_no_bar_fails_closed`). Without that, the
//! generosity this section invites steers an implementer at a marker table
//! including `"test plan"` and `"testing"`, which then certifies a very large
//! fraction of real pull requests that carry no acceptance bar at all — the
//! exact false green this seat exists to prevent, with every test here green.
//! It is the same decision `a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it`
//! already states in prose ("no reading of 'Testing', 'Rollout' or 'Notes' is a
//! synonym for the bet or the bar"), made enforceable when the marker is absent
//! rather than only when it is present and empty. Listed in open_questions.
//!
//! That promise was false in the revision before this one, in the one place it
//! mattered most. `a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it`
//! required `## Problem` / a real problem / `## Done when` / `**Acceptance
//! criteria**` / three real checkable criteria to be reported as missing its
//! acceptance bar — a body that genuinely states both artifacts, failed, over
//! the first synonym these docs name as free to recognise. An implementer who
//! took the promise at face value wrote `is_done_when_marker(t) = t == "done
//! when" || t == "acceptance criteria"`, produced a correct and more forgiving
//! gate, and was told by a settled specification test that it was broken —
//! which leaves them editing the spec mid-implementation, the one thing this
//! project's method forbids. That test now uses sub-labels no reading makes a
//! synonym for either artifact (`**Testing**`, `**Rollout**`, `**Notes**`) over
//! testing prose rather than over an acceptance bar, so it pins the boundary
//! rule it claims to pin and closes no vocabulary. The promise above is true
//! again, and it is the load-bearing one: a gate more generous about *which
//! words* announce a section is a better gate, and nothing here may punish it.
//!
//! What is no longer left open is the markdown *around* the same two words.
//! A previous revision built every one of its passing bodies from the
//! byte-identical strings `## Problem` and `## Done when`. Paired with the
//! (correct) requirement that a body carrying no bar fails closed, that admits
//! a gate matching exactly two byte strings — one that rejects `## Done When`,
//! `### Done when`, `## Done when:` and `**Done when**`, and therefore withholds
//! certification from essentially every real pull request once wired into
//! `seal()`. This repository has no PULL_REQUEST_TEMPLATE forcing one spelling,
//! so nothing else would have caught it. A false accusation is the same defect
//! as a false green pointed the other way, so
//! `the_same_two_words_are_the_marker_however_the_author_formats_them` pins case,
//! depth, a trailing colon and a bold label as passing — and pins the mirror,
//! that an empty section under each of those spellings still fails, so the
//! widened recognition cannot itself become a fail-open.
//!
//! # What these tests deliberately do NOT pin
//!
//! Synonyms for the two headings, per above. The render order of
//! `missing_artifacts` (the helper sorts before comparing). The prose of the
//! failure messages beyond naming each missing artifact and differing from one
//! another. And the change's *title*: `judge` takes the body alone, because no
//! behavioural test in this suite could distinguish a gate that read the title
//! from one that ignored it, and this suite is not going to require plumbing an
//! input it cannot measure.
//!
//! That last one is a decision recorded here and NOT a test. The previous
//! revision carried one named for the claim, and its body could not falsify it:
//! with no title parameter the compiler already enforces the fact, and its two
//! assertions were byte-identical duplicates of assertions in two other tests.
//! A test that cannot fail for the reason its own name gives publishes
//! assurance it has not earned — the defect class this file exists to prevent —
//! so it was deleted rather than kept as decoration.
//!
//! Stage discipline: these are red tests, written before the gate exists.
//! `pre_merge_guard::product_bar::{judge, missing_artifacts}` are `todo!()`,
//! and the evaluator carries a placeholder status rather than a call to them,
//! so the wiring tests at the bottom of this file are red for the same reason
//! as the rest.
//!
//! And they are not the only red on this branch, which is worth stating plainly
//! rather than leaving a reader to infer that only this file moves. The
//! scaffolding adds a seventy-third `: GateStatus` field to
//! `PreMergeCertificationReport` without moving `all_statuses()`,
//! `named_statuses()` or `TOTAL_GATES` — because moving them is implementation,
//! and it is the implementation that
//! `the_product_bar_gate_joins_the_corpus_without_desynchronising_the_declared_total`
//! and `the_product_bar_name_is_bound_to_the_product_bar_field` specify. Three
//! pre-existing tests are red against that field until they are:
//! `pre_merge_guard::report::tests::all_statuses_covers_every_gate_field`,
//! `brand_absence::tests::real_gate_count_reads_the_corpus`, and
//! `every_computed_gate_reaches_the_report_test::the_declared_total_matches_what_the_report_actually_carries`.
//! Those are the corpus invariants this change genuinely moves, and they go
//! green with the corpus work the two tests above demand.

use anvil::pre_merge_guard::product_bar;
use anvil::pre_merge_guard::product_bar::Artifact;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use anvil::pre_merge_guard::{GateStatus, PreMergeCertificationReport};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

// There is no TITLE fixture. A well-written conventional-commit subject
// ("fix(certify): stop reporting an unread canary as passed") says what
// changed; it never says what done looks like, and it is a label for a problem
// statement rather than one. `judge` takes the body alone, so no fixture here
// can distinguish a gate that read a title from one that ignored it — see the
// module docs, and the block comment headed "THE TITLE IS NOT AN INPUT" further
// down for the test that used to claim otherwise.

/// A real problem statement: what is wrong, and why it matters.
const PROBLEM: &str = "The canary gate rebuilds its verdict from `passed`, which is `true` for a \
     canary nobody queried. Every change that touches the rollout path is \
     certified against a measurement that never happened, so the scorecard \
     reads green for the exact condition the gate exists to catch.";

/// A real acceptance bar: the condition under which this is done, stated so
/// that someone other than the author can check it.
const BAR: &str = "An unqueried canary reports NotMeasured and withholds merge-queue admission; a \
     queried canary with divergent P99 reports Failed; the scorecard names which \
     of the two happened.";

/// A real acceptance bar written the way most of them are written: as a list of
/// separately checkable criteria. Multi-line on purpose — an extractor that
/// takes only the first line after the marker, and one that takes only the
/// first *non-blank* line, are two different wrong gates and this fixture is
/// what several families below use to separate them.
const MULTILINE_BAR: &str = "- `slo_status` is NotMeasured when no telemetry endpoint is configured\n\
     - `is_admissible()` is false while any gate is NotMeasured\n\
     - the posted scorecard names every unmeasured gate by id";

/// A genuine problem statement that happens to be one line long. Paired with
/// `SHORT_BAR` it is the smallest change that has still done Product's job, and
/// it is what stops "substantive" from collapsing into "long".
const SHORT_PROBLEM: &str = "Checkout p99 regressed to 40ms after the cache change.";

/// A genuine acceptance bar, eleven bytes of it. Shorter than four of the
/// placeholders that must fail.
const SHORT_BAR: &str = "- p99 < 5ms";

/// A one-line acceptance bar with no bullet, for the shapes where the bar is
/// written on the marker's own line (`## Done when: …`).
const INLINE_BAR: &str = "checkout p99 is under 5ms and the scorecard names the canary it queried";

/// A one-line problem with no bullet, for the same inline shapes.
const INLINE_PROBLEM: &str = "checkout p99 regressed to 40ms when the cache change landed";

/// Genuine content written in as few characters and as few words as anyone ever
/// writes it, and every one of these must pass.
///
/// # Why this exists
///
/// The shortest content that had to pass anywhere in this file used to be
/// `SHORT_BAR` — `"- p99 < 5ms"`, which normalises to nine characters and three
/// words. Nothing between one and eight characters, and nothing of one or two
/// words, was ever required to pass. Module-doc property (2) called that closed
/// because "a minimum section length that admits `SHORT_BAR` admits the
/// thirty-nine-byte placeholder too" — which is true of a length rule used
/// *instead of* a content check, and false of a length floor bolted *on top of*
/// one. Review verified it: appending `core.chars().count() >= 9` to an
/// otherwise-correct content predicate passed every behavioural test in this
/// file, and so did "the line must contain a space" and "the line must have at
/// least three tokens". The Korean fixtures did not close it either — `KO_BAR`'s
/// lines are long.
///
/// Each of these is shorter, in characters AND in whitespace-separated words,
/// than the longest string this file requires to fail, and
/// `content_passes_however_few_characters_and_words_it_takes` asserts exactly
/// that before using them. `"- p99<5ms"` has no space in it at all, and
/// `"- 빌드 통과"` is five characters of Hangul in thirteen bytes, so a floor in
/// either unit rejects one of them.
///
/// They are terse, and terse is not the same as absent: "no 5xx" is a checkable
/// condition, and rejecting it measures how much the author typed rather than
/// whether they said what done looks like.
const SHORT_REAL_CONTENT: &[&str] = &["- no 5xx", "- p99<5ms", "- 빌드 통과", "- 결제 실패 급증"];

/// Sections whose first word merely *begins* with a deferral stem, plus one
/// that opens with a deferral token used as an ordinary word.
///
/// These are the bound on the deferral vocabulary, and without them the whole
/// derived family is satisfiable by a prefix test on the normalised line —
/// `STEMS.iter().any(|s| normalised.starts_with(s))` with `na`, `tbd`, `todo`,
/// `wip`, `xxx` in the table. That gate passes every other fixture in this file
/// (`"Today:"` misses `"todo"` by one character) and then reports a missing bar
/// for `- Navigation completes in under 200ms`, a missing problem for a section
/// opening `Native TLS …`, and kills anything starting `X-Ray`, `Wipe` or
/// `NAT`. Every string below is a real thing a person writes, and every one of
/// them must pass.
///
/// The last entry is the same defect on the same line rather than at the start
/// of a word: `TODO` is the first token and the rest of the line is content, so
/// a rule keyed on "the first token is a deferral" rejects it. What separates
/// it from `"TODO: write the acceptance criteria here"` and `"TBD - will fill
/// this in before merge"` — both of which must still fail — is the separator: a
/// deferral announces itself with `:` or ` - ` or nothing at all, while `TODO
/// comments` is the word used in a sentence.
const REAL_CONTENT_WITH_A_DEFERRAL_PREFIX: &[&str] = &[
    "- Navigation completes in under 200ms",
    "Native TLS was disabled by the cache change, so every canary poll now falls back to the \
     plaintext listener.",
    "- NAT rebinding no longer drops the canary connection mid-poll",
    "- Wipe the stale rollout entries before the queue admits the change",
    "TODO comments are removed from src/pre_merge_guard/",
];

/// Every one of these has been shipped in a real pull request body. A gate that
/// reads any of them as an artifact is measuring the presence of a heading.
///
/// The last two are longer than `SHORT_BAR`, so a placeholder screen cannot
/// degenerate into a length threshold; the checkbox and the template comment are
/// the empty-bullet defect in the shape PR templates actually produce.
///
/// This table is deliberately **not** the whole must-fail set. Copying it into
/// the implementation as an exact-match list satisfies these fifteen and
/// nothing else — see `derived_deferrals`, `PHRASE_DEFERRALS` and
/// `UNICODE_BLANKS`.
const PLACEHOLDERS: &[&str] = &[
    "TBD",
    "tbd",
    "N/A",
    "n/a",
    "TODO",
    "todo",
    "-",
    "- ",
    "*",
    "...",
    "   \n   \n",
    "- [ ] ",
    "<!-- what problem does this solve? -->",
    "TBD - will fill this in before merge",
    "TODO: write the acceptance criteria here",
];

/// The prompt block a real pull request template ships, written the way
/// GitHub's own template documentation writes it: an HTML comment spread over
/// several lines, with the prompt on its own line between the delimiters.
///
/// # Why the multi-line form has to be pinned separately
///
/// Every HTML-comment fixture in this file used to open and close on ONE line.
/// So the only rule the suite required was a line-local predicate —
///
///     let t = line.trim();
///     (t.starts_with("<!--") && t.ends_with("-->")) || ..
///
/// — which is the obvious thing to write once `is_content` is a per-line
/// function, and `a_section_mixing_a_deferral_with_real_content_is_judged_on_the_real_content`
/// forces `is_content` to be per-line by deciding the `any(substantive)` rule.
/// Handed a completely unfilled template whose prompts are multi-line, the
/// inner prompt line starts with neither delimiter and ends with neither, so
/// both sections read as carrying real content and the body certifies whole.
/// That is the pasted-template false green this file's module docs call the
/// whole ballgame, on the single commonest unfilled body there is, with all of
/// the rest of this suite green while it ships.
///
/// Both halves are pinned, as with every other must-fail token (module-doc
/// property 9): a section whose whole content is the prompt block is no
/// artifact, and the same block left above or below the author's own text
/// erases nothing. The passing mirror is what stops the fix degenerating into
/// "reject any section containing `<!--`", or "reject any section holding a
/// line that opens a comment it does not close" — either of which fails the
/// commonest FILLED-IN template body there is.
const MULTILINE_PROMPT_PROBLEM: &str =
    "<!--\nWhat problem does this solve? Why does it matter?\n-->";

/// The done-when half of the same template. Neither prompt uses any of the
/// vocabulary the failure message is judged on beyond the word this file
/// already subtracts from the body, so both blocks are run through both
/// sections.
const MULTILINE_PROMPT_BAR: &str = "<!--\nHow will a reviewer check this is done?\n-->";

/// Deferral stems. These are never enumerated as finished strings: the tests
/// multiply them out by trailing punctuation, letter case and bullet wrapping,
/// so the hundreds of must-fail sections they produce appear nowhere in this
/// file as literals an implementation could copy. Normalising before comparing
/// is the cheapest way through, and normalising is what "the measurement is the
/// content" asks for.
const DEFERRAL_STEMS: &[&str] = &[
    "tbd",
    "tba",
    "n/a",
    "na",
    "todo",
    "to do",
    "wip",
    "xxx",
    "???",
    "-",
    "_",
    "[ ]",
    "[x]",
    "- [x]",
    "todo(jason)",
];

/// Deferrals that share no prefix with any entry in `PLACEHOLDERS`, so a table
/// copied from that constant cannot reach them. Each is a real thing authors
/// write in a done-when section instead of an acceptance bar.
///
/// The third and fourth pin a product decision as much as a technical one: the
/// artifact lives on the change under review, so a pointer to somewhere else is
/// not the artifact. That is listed in open_questions for a human to veto.
const PHRASE_DEFERRALS: &[&str] = &[
    "see the linked issue",
    "same as above",
    "will fill this in later",
    "as discussed in standup",
];

/// A section whose whole content is a reference to somewhere else.
///
/// The artifact lives on the change under review: the reviewer, the auditor and
/// the scorecard all read this body, and none of them follows the link. Hoisted
/// out of `a_pointer_to_somewhere_else_is_not_the_artifact` so the inline marker
/// family can run the same table through the marker line itself — `## Done when:
/// #4192` is the pasted-template false green in the one marker shape whose
/// must-fail mirrors were never filled in. Listed in open_questions as a
/// decision a human can veto.
const POINTERS: &[&str] = &[
    "https://example.invalid/issues/4192",
    "See https://example.invalid/issues/4192",
    "#4192",
    "See #4192",
    "- https://example.invalid/issues/4192",
    "example.invalid/issues/4192",
];

/// The done-when half of the single-line template prompt.
///
/// `PLACEHOLDERS` carries the problem-flavoured one
/// (`"<!-- what problem does this solve? -->"`); this is its counterpart, so the
/// inline family runs the prompt an author actually leaves on a `## Done when:`
/// line rather than only the one from the other section.
const SINGLE_LINE_PROMPT_BAR: &str = "<!-- how will a reviewer check this is done? -->";

/// Sections that are blank to a reader and non-blank to `trim()`. U+200B and
/// U+FEFF are not `char::is_whitespace`, so `!s.trim().is_empty()` reads them
/// as substance; U+00A0, U+2003 and U+3000 arrive whenever anyone pastes out of
/// a document editor.
const UNICODE_BLANKS: &[&str] = &[
    "\u{00a0}",
    "\u{200b}",
    "\u{feff}",
    "\u{00a0}\u{00a0}\n\u{00a0}",
    "\u{2003}\u{200b}\n\u{feff}",
    "\u{3000}",
];

/// A Korean problem statement. Short in characters, long in bytes.
const KO_PROBLEM: &str =
    "머지 큐가 조회되지 않은 카나리를 통과로 처리해서, 측정된 적 없는 변경이 승인된다.";

/// A Korean acceptance bar, written as checkable criteria.
const KO_BAR: &str = "- 조회되지 않은 카나리는 NotMeasured 로 보고하고 머지 큐 진입을 막는다\n\
     - 스코어카드가 둘 중 무엇이었는지 이름을 밝힌다";

/// The two words of each heading, in the markdown an author actually wraps them
/// in. Every entry is the *same words* — case, depth, a trailing colon and a
/// bold label are formatting, not vocabulary, and a gate that recognises only
/// one of them rejects nearly every real pull request. Synonyms are deliberately
/// absent: which words announce a section is left open, per the module docs.
const DONE_WHEN_MARKERS: &[&str] = &[
    "## Done when",
    "## Done When",
    "## done when",
    "### Done when",
    "# Done when",
    "## Done when:",
    "**Done when**",
];

const PROBLEM_MARKERS: &[&str] = &[
    "## Problem",
    "## problem",
    "### Problem",
    "# Problem",
    "## Problem:",
    "**Problem**",
];

/// Ordinary whitespace around a marker line, which every marker fixture used to
/// be free of.
///
/// # Why this exists
///
/// Every literal in `DONE_WHEN_MARKERS` and `PROBLEM_MARKERS` is exactly
/// terminated, and `as_eol` only ever appends `\r`, so before this constant no
/// body anywhere in this file that had to PASS carried `## Done when   ` or
/// `**Done when** `. That is the same defect class the marker family closes for
/// `\r`, left open for the character next to it — and the file's own CRLF
/// commentary steers an implementer straight at the narrow fix
/// (`trim_end_matches('\r')`) rather than at a full trim. Review verified it
/// against a working reference:
///
/// ```text
/// let l = line.trim_start().trim_end_matches('\r');
/// if let Some(rest) = l.strip_prefix("**") {
///     return rest.strip_suffix("**");          // false for "**Done when** "
/// }
/// Some(l.trim_start_matches('#').trim_start().trim_end_matches(':'))
/// ```
///
/// That handles `\r`, passes all 84 marker fixtures under both line endings,
/// and then reports BOTH artifacts missing from a complete, well-written body
/// whose author left a space after the heading — or who used markdown's
/// two-trailing-spaces line-break idiom, which is invisible in every editor
/// there is. The accusation it produces is not invisible.
///
/// Leading whitespace is here for the same reason and is markdown in its own
/// right: an ATX heading may be indented up to three spaces.
const MARKER_PADDINGS: [fn(&str) -> String; 3] = [
    |m| format!("{m}  "),
    |m| format!("  {m}"),
    |m| format!("  {m}  "),
];

/// Headings an author writes for a third section, at every depth and weight
/// markdown allows. A body with three sections is the commonest filled-in
/// template there is, and until this suite carried one no fixture falsified the
/// gate's section-boundary logic in either direction.
///
/// Used for the *passing* family only: a complete artifact followed by any of
/// these is still a complete artifact, whether the gate reads the third heading
/// as a boundary or reads straight past it.
const THIRD_SECTION_HEADERS: &[&str] = &[
    "# Testing",
    "## Testing",
    "### Testing",
    "**Testing**",
    "Testing:",
];

/// The subset of `THIRD_SECTION_HEADERS` that must TERMINATE the section above
/// it, so that an empty `## Done when` cannot swallow the testing notes.
///
/// # Why `"Testing:"` is not in this list any more
///
/// A previous revision required a bare colon-terminated line, followed by a
/// blank line, to end the section above it — and separately required a
/// colon-terminated *lead-in* line to be ordinary writing. The only structural
/// difference between the two is the blank line, so the rule the suite forced
/// was "a colon-terminated line followed by a blank line is a heading". That
/// rule then reports a missing acceptance bar for
///
/// ```text
/// ## Done when
///
/// Acceptance criteria:
///
/// - p99 < 5ms
/// ```
///
/// which is one of the commonest shapes a done-when takes. The two demands are
/// irreconcilable: those two bodies are structurally identical, and only the
/// English tells them apart. So the suite decides, rather than leaving the
/// implementer to guess which half to satisfy:
///
///   * a colon-terminated line is **never** a section boundary. It is ordinary
///     technical writing, markdown says nothing else about it, and
///     `a_short_colon_terminated_lead_in_line_is_writing_not_a_section_boundary`
///     now pins the blank-line form as passing in both sections.
///   * a bold-only line **is** a heading, blank line or no blank line. This
///     suite already treats one as a heading in the other direction — `**Done
///     when**` and `**Problem**` are markers that open a section — so a
///     bold-only line that opens a *different* topic closes the one above it.
///     `a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it`
///     pins the consequence, which is the half of this decision that costs an
///     author something.
///
/// Both halves are listed in open_questions. The veto for the first is putting
/// `"Testing:"` back and pinning the lead-in family as failing; the veto for
/// the second is dropping `"**Testing**"` from this list and pinning the bold
/// lead-in as passing. Whichever a human picks, the pair has to stay
/// consistent, which is what `assert_the_boundary_families_state_one_consistent_rule`
/// asserts.
///
/// # Why `"### Testing"` is not in this list any more either
///
/// The third boundary decision, and it is the same shape as the first. This
/// list used to require `# Testing`, `## Testing` AND `### Testing` to
/// terminate an empty `## Done when`, at every depth including deeper than the
/// marker itself, so the only rule that satisfied the family was "any ATX
/// heading ends the section above it". No fixture anywhere in the file put a
/// heading INSIDE a section, above that section's content, so the cost of that
/// rule was never written down: it reports a missing acceptance bar for
///
/// ```text
/// ## Done when
///
/// ### Criteria
///
/// - p99 < 5ms
/// - the scorecard names the canary it queried
/// ```
///
/// a body that genuinely states both artifacts, under an ordinary sub-heading,
/// rejected. That is the fabricated accusation this file names as equal in
/// severity to a false green, and — like `Acceptance criteria:` above a list of
/// bullets — it is structurally identical to the shape the boundary rule exists
/// to catch. Only the English tells `### Criteria` above a bar apart from
/// `### Testing` above testing notes.
///
/// So the suite decides, the same way and in the same direction as the colon
/// case, rather than leaving an implementer to meet it as a live defect:
///
///   * a heading DEEPER than the marker that opened the section is nested
///     content, not a boundary. `a_heading_deeper_than_the_marker_is_content_inside_the_section`
///     pins the bar under `### Criteria` as passing, and pins the emptied mirror
///     as failing, so the widened reading cannot itself become a fail-open.
///   * a heading at the marker's own depth or shallower is a SIBLING section and
///     still terminates, which is why `# Testing` and `## Testing` stay.
///   * a heading the gate recognises as a MARKER terminates whatever depth it
///     sits at. That is not new and not a choice: the marker family already pins
///     `# Problem` with nothing under it above `## Done when` as missing its
///     problem statement, and `**Problem**` carries no depth at all.
///
/// What that decision costs is a false green, and it is stated in the module
/// docs' "What this suite does NOT close" rather than left to be inferred from
/// here: an empty `## Done when` above `### Testing` and real testing notes is
/// no longer pinned, and the natural implementation certifies it. The veto is
/// putting `"### Testing"` back into this list and flipping
/// `a_heading_deeper_than_the_marker_is_content_inside_the_section`'s passing
/// fixtures to `expect_missing`.
const BOUNDARY_HEADERS: &[&str] = &["# Testing", "## Testing", "**Testing**"];

/// What a third section says. It reports what the author did; it states neither
/// what is wrong nor how anyone checks the change is done, so counting it as
/// either artifact is the boundary defect and nothing else.
const THIRD_SECTION_BODY: &str = "Ran `cargo test --all` locally on macOS and again on the CI \
     runner, and re-ran the canary integration suite twice.";

/// PR bodies authored in the GitHub web UI arrive over the webhook with CRLF,
/// because that is what an HTML textarea submits. Nothing in this repository
/// normalises line endings between the payload and the guard layer, so the
/// fixtures are built over both and the verdict must not depend on which.
#[derive(Clone, Copy, Debug)]
enum Eol {
    Lf,
    Crlf,
}

impl Eol {
    fn seq(self) -> &'static str {
        match self {
            Eol::Lf => "\n",
            Eol::Crlf => "\r\n",
        }
    }
}

/// Both line endings, so a family can be written once and run over each.
const BOTH_EOLS: [Eol; 2] = [Eol::Lf, Eol::Crlf];

/// `body`, re-terminated for `eol`.
///
/// The fixtures below are written in LF and rewritten here rather than
/// threading a separator through every `format!`, which is how the CRLF twins
/// in `awkward_bodies` are built too. The two are the same string: none of
/// these fixtures contains a `\r` of its own.
fn as_eol(body: &str, eol: Eol) -> String {
    match eol {
        Eol::Lf => body.to_string(),
        Eol::Crlf => body.replace('\n', "\r\n"),
    }
}

/// `body` with its final line terminator removed, asserting that it had one.
///
/// # Why the passing side needs this shape
///
/// Every body this suite required to PASS ended with `\n`, without exception —
/// `body_with`, `problem_only`, `bar_only`, `long_body_with_bar_at_the_end` and
/// every inline `format!` in the marker, boundary and CRLF families all
/// terminated their final section. The unterminated shape appeared only in
/// `awkward_bodies`, which is entirely on the failing side, and that fixture's
/// own comment states the fact that makes the asymmetry fatal: GitHub bodies
/// routinely have no trailing newline.
///
/// This is the defect this file already names for the checkbox, the pointer and
/// the template comment (module-doc property 9): a shape pinned only among
/// must-fail inputs can be rejected on sight, and the rejection is invisible.
/// Review verified it against a working reference:
///
/// ```text
/// fn inline_after_marker<'a>(body: &'a str, at: usize, marker: &str) -> Option<&'a str> {
///     let rest = &body[at + marker.len()..];
///     let nl = rest.find('\n')?;      // None when the marker line ends the body
///     Some(&rest[..nl])
/// }
/// ```
///
/// A `?` on `find('\n')` is an entirely ordinary way to read the line a marker
/// sits on. It is green across all 84 marker fixtures and both inline-colon
/// families, because each of them ends `…{INLINE_BAR}\n` — and it reports a
/// missing acceptance bar for the shape the GitHub API returns for the majority
/// of pull request bodies.
fn unterminated(body: &str) -> String {
    let stripped = body.strip_suffix('\n').unwrap_or(body);
    let stripped = stripped.strip_suffix('\r').unwrap_or(stripped);
    assert_ne!(
        stripped, body,
        "fixture invariant: this body was already unterminated, so stripping its \
         terminator is not the shape this family means to run"
    );
    assert!(
        !stripped.ends_with('\n') && !stripped.ends_with('\r'),
        "fixture invariant: the body must end in something other than a line \
         terminator, or the family stops testing the unterminated shape. Got {stripped:?}"
    );
    stripped.to_string()
}

/// The marker spelling these fixtures commit to. See the module docs: the
/// behaviour under test is what sits under the headings, never the headings.
fn body_with_eol(problem: &str, done_when: &str, eol: Eol) -> String {
    let n = eol.seq();
    format!("## Problem{n}{n}{problem}{n}{n}## Done when{n}{n}{done_when}{n}")
}

fn body_with(problem: &str, done_when: &str) -> String {
    body_with_eol(problem, done_when, Eol::Lf)
}

fn problem_only_eol(problem: &str, eol: Eol) -> String {
    let n = eol.seq();
    format!("## Problem{n}{n}{problem}{n}")
}

fn problem_only(problem: &str) -> String {
    problem_only_eol(problem, Eol::Lf)
}

fn bar_only(done_when: &str) -> String {
    format!("## Done when\n\n{done_when}\n")
}

/// A third section, headed the way `header` heads it.
fn third_section(header: &str) -> String {
    format!("{header}\n\n{THIRD_SECTION_BODY}\n")
}

/// Four paragraphs of genuine problem analysis and not one word about what done
/// looks like. Used on the *failing* side so that the longest body in the whole
/// suite is one that must fail.
fn long_prose() -> String {
    format!("{PROBLEM}\n\n{PROBLEM}\n\n{PROBLEM}\n\n{PROBLEM}")
}

/// The mirror: a long, real acceptance bar, used where the problem statement is
/// the thing that is missing.
fn long_bar() -> String {
    format!("{BAR}\n\n{BAR}\n\n{BAR}\n\n{BAR}")
}

/// Two kilobytes of genuine background, over thirty-odd lines, saying nothing
/// about what done looks like.
///
/// # Why this exists
///
/// Every body in this file that had to PASS used to be short, and every long
/// body was on the failing side. The module docs called that closed — "the
/// failing set brackets the passing set at both ends, so no threshold on total
/// length can separate them" — and bracketing does close off a *threshold*. It
/// does not close off a *truncation*, and truncation is the direction that
/// produces a fabricated accusation rather than a false green.
///
/// Review verified the hole against a working implementation: wrapping its entry
/// point in `let pr_body = &pr_body[..450.min(pr_body.len())]` passed every
/// behavioural test in this file, and so did `pr_body.lines().take(18)`. Four
/// hundred bytes and sixteen lines were caught; four hundred and fifty and
/// eighteen were not, because no must-pass body anywhere put its acceptance bar
/// further in than that. A bounded regex, a `.take(n)`, a `&body[..N]` "safety"
/// clamp and an extractor that stops after the first two sections are all
/// ordinary ways to write this gate, and every one of them would have shipped
/// telling authors of long, careful pull requests that they wrote no bar.
///
/// So the passing and failing sets now overlap in length at BOTH ends: this
/// background carries a real bar at the far end of it and must pass, and the
/// same background with that section emptied must still fail. It is written out
/// at three magnitudes — see `LONG_BODY_SCALES`, which is what stops the overlap
/// being a window a larger clamp can be written above. It contains none of the
/// vocabulary the failure message is judged on — see
/// `assert_the_content_fixtures_carry_none_of_the_message_vocabulary`.
const LONG_BACKGROUND_LINES: &[&str] = &[
    "The merge queue admits a change as soon as every gate reports an acceptable",
    "status, and nine of those gates read their evidence from the rollout",
    "controller rather than from the change itself.",
    "",
    "The controller answers a poll from a cache whenever the upstream store is",
    "slow, and the cache has no notion of staleness. A poll that times out is",
    "served the last document the controller happened to hold, which on a quiet",
    "afternoon can be several hours old.",
    "",
    "What that has cost us across the last three release trains:",
    "",
    "- eleven changes were admitted against a canary window that had already closed",
    "- four of those eleven were rolled back inside the same hour",
    "- the scorecard showed nine green gates for every one of the four",
    "",
    "The rollback that started this was a checkout latency regression. The canary",
    "window had closed forty minutes before the change was queued, so the P99 the",
    "gate compared against was the P99 of the change before it, and the two",
    "changes touched the same code path.",
    "",
    "Background on the cache, for anyone who has not read the controller:",
    "",
    "The controller was written when the upstream store was in-process and a poll",
    "could not fail. The cache was added during the migration to the shared store,",
    "as a way to keep the dashboard responsive while the store warmed up. Nobody",
    "revisited it once the gates began reading from the same endpoint.",
    "",
    "What I have already ruled out, so nobody repeats the work:",
    "",
    "- the upstream store is healthy; its own latency has not moved in six weeks",
    "- the timeout is 250ms and has been since the endpoint was introduced",
    "- raising the timeout to 2s moves the failure rate but not the staleness",
    "- the controller records no metric for how old a served document is",
    "",
    "So the fix has to be on the reading side rather than the serving side: a gate",
    "that cannot tell how old its evidence is must not report a verdict at all.",
];

/// GitHub's documented maximum pull request body length, in CHARACTERS.
///
/// The guard layer cannot be handed a body longer than this, so a clamp above it
/// cannot truncate anything. That is what turns the long-body family from a
/// widening into a close: see `LONG_BODY_SCALES`.
///
/// It also bounds the LINE count, because a body cannot hold more lines than it
/// holds characters — every line costs at least its own terminator. That is the
/// bound the tall fixture has to clear, and the one the previous revision
/// claimed to have closed without measuring: `LONG_BACKGROUND_LINES` is
/// thirty-eight lines, so even x64 is about twenty-five hundred, and review
/// confirmed that `pr_body.lines().take(3000)` and `.take(5000)` both left all
/// forty-one tests green while telling every author of a long, line-heavy pull
/// request that they wrote no acceptance bar.
const GITHUB_MAX_BODY_CHARS: usize = 65_536;

/// The same bound in BYTES, which is the unit a Rust clamp is written in.
///
/// `&pr_body[..N]`, `pr_body.len()` and `String::len()` are all byte counts, and
/// a character in GitHub's limit may be up to four bytes of UTF-8 — the Korean
/// fixtures in this file are three bytes each. So a body inside GitHub's
/// character limit can be four times `GITHUB_MAX_BODY_CHARS` bytes long, and a
/// byte clamp is only above every body the guard layer can receive if it is
/// above THIS. Review measured the residual: with the passing side topping out
/// around a hundred and thirty kilobytes, a byte clamp at 200,000 passed all
/// forty-one tests.
///
/// The previous revision's invariant compared `String::len()` — bytes — against
/// `GITHUB_MAX_BODY_CHARS` — characters. It happened to hold because
/// `LONG_BACKGROUND_LINES` is pure ASCII, so the two units coincided, but it
/// read as a stronger claim than it made.
const GITHUB_MAX_BODY_BYTES: usize = GITHUB_MAX_BODY_CHARS * 4;

/// How many times over the long background is repeated, for the family that
/// brackets the passing set from below.
///
/// # Why one magnitude was not enough
///
/// The previous revision ran this family at ONE size — two kilobytes, with the
/// acceptance bar at byte 1864 on line 39 — and the module docs conceded the
/// consequence: "a clamp written INSIDE `judge` with a limit above that window
/// survives this file". Review measured it against a working reference
/// implementation rather than reasoning about it. Inserting
///
/// ```text
/// let pr_body = &pr_body[..2100.min(pr_body.len())];
/// ```
///
/// at the top of `missing_artifacts` gave `test result: ok. 40 passed; 0
/// failed`, and so did `for line in pr_body.lines().take(50)`. Both left the
/// whole repository green. Each is the one-line "safety clamp" an engineer
/// writes without thinking, each leaves `judge` perfectly correct on every
/// fixture in this file, and each tells every author of a pull request body
/// over about two kilobytes or fifty lines that they wrote no acceptance bar.
/// Real bodies exceed two kilobytes routinely. This file already rejects the
/// byte-identical mutation one line higher — `truncated_argument` refuses
/// `judge(&pr_body[..2000.min(pr_body.len())])` at the call site — so pinning
/// only one magnitude was the file's own standard applied inconsistently one
/// layer down.
///
/// The docs framed the remedy as a product decision ("a stated maximum body
/// size"). It is not. No test anywhere in this file requires a body to fail on
/// account of its length, so lengthening the passing side cannot collide with
/// anything the specification already demands, and the emptied mirror grows
/// alongside it at every magnitude so the fail-open direction stays pinned.
/// That makes it fixture work.
///
/// Three magnitudes an order of magnitude apart, each with its mirror, so no
/// clamp at any fixed BYTE count below a hundred and thirty kilobytes can
/// satisfy the family: it must sit above the largest to pass the passing side
/// and below the smallest to fail the failing side.
///
/// # What these three do NOT close, and what does
///
/// The previous revision's docs said "no clamp at any FIXED byte count or line
/// count". The byte half was true up to a point and the LINE half was false, and
/// review measured both. `LONG_BACKGROUND_LINES` is thirty-eight lines, so even
/// x64 is roughly two and a half thousand lines — while a GitHub pull request
/// body may hold 65,536 characters and therefore up to 65,536 lines. Any line
/// clamp between the two survived the whole file:
///
/// ```text
/// pr_body.lines().take(200)   -> test result: FAILED. 40 passed; 1 failed
/// pr_body.lines().take(3000)  -> test result: ok. 41 passed; 0 failed
/// pr_body.lines().take(5000)  -> test result: ok. 41 passed; 0 failed
/// ```
///
/// `for line in pr_body.lines().take(5000)` is exactly the one-line safety clamp
/// this file's own commentary says an engineer writes without thinking. The byte
/// direction had a smaller residual for the same reason in the other unit: the
/// limit is 65,536 CHARACTERS, so a body of four-byte characters reaches 262,144
/// bytes, and a byte clamp at 200,000 also passed all forty-one.
///
/// Repeating this background is the wrong tool for either gap — sixty-five
/// thousand lines of it would be a megabyte and a half of prose. So the family
/// gains one more member built the other way round, out of short single-token
/// lines: see `TALL_BACKGROUND_LINES`. Between them the passing side now clears
/// `GITHUB_MAX_BODY_BYTES` in bytes AND `GITHUB_MAX_BODY_CHARS` in lines, which
/// is every body the guard layer can ever be handed, in both units. A clamp that
/// survives this file cannot truncate anything.
const LONG_BODY_SCALES: [usize; 3] = [1, 8, 64];

/// How many short lines the tall background is written over.
///
/// Above `GITHUB_MAX_BODY_CHARS` on purpose, and that is the whole argument: a
/// body cannot hold more lines than characters, so a line clamp that passes this
/// fixture sits above every body GitHub can deliver and truncates nothing. It is
/// a floor to clear, not a size chosen for its own sake.
const TALL_BACKGROUND_LINES: usize = 66_000;

/// The rotating content of `tall_background`, one short line each.
///
/// Real content — no deferral, no placeholder, no pointer, nothing blank — so
/// the section that carries it reads as substantive and the tall fixture's
/// mirror is missing its acceptance bar and nothing else. Short, because the
/// point of this fixture is a line count that outruns its byte count. And none
/// of the vocabulary the failure message is judged on, like every other content
/// fixture here — `assert_the_content_fixtures_carry_none_of_the_message_vocabulary`
/// pins that.
const TALL_BACKGROUND_STEMS: &[&str] = &[
    "- ring {} held",
    "- ring {} was served from cache",
    "- ring {} polled and timed out",
    "- ring {} rolled back inside the hour",
    "- ring {} queued against a closed window",
    "- ring {} read a stale document",
    "- ring {} showed nine green gates",
];

/// `TALL_BACKGROUND_LINES` short lines of genuine background, saying nothing
/// about what done looks like.
///
/// The line-count half of the long-body family. `long_background_at` grows the
/// byte count fastest; this grows the line count fastest, and the two together
/// leave no fixed clamp in either unit that can pass the passing side and fail
/// the failing side.
fn tall_background() -> String {
    (0..TALL_BACKGROUND_LINES)
        .map(|i| {
            TALL_BACKGROUND_STEMS[i % TALL_BACKGROUND_STEMS.len()].replace("{}", &i.to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The long background written `times` over, paragraph by paragraph.
fn long_background_at(times: usize) -> String {
    assert!(
        times > 0,
        "fixture invariant: the background must be written at least once"
    );
    let once = LONG_BACKGROUND_LINES.join("\n");
    (0..times)
        .map(|_| once.clone())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn long_background() -> String {
    long_background_at(1)
}

/// `times` copies of the long background with `done_when` written at the far
/// end of them.
///
/// Passed a real bar this is a body that must pass and is longer than every body
/// in this file that must fail; passed `""` it is the same body with the section
/// emptied, and must still fail.
fn long_body_with_bar_at_the_end(times: usize, done_when: &str) -> String {
    body_around(&long_background_at(times), done_when)
}

/// A body whose problem section is `background` and whose done-when section is
/// `done_when` — a real bar, or `""` for the emptied mirror.
fn body_around(background: &str, done_when: &str) -> String {
    format!("## Problem\n\n{background}\n\n## Done when\n\n{done_when}\n")
}

/// `stem` in lower case, upper case and sentence case — the three shapes a
/// human actually types a deferral in.
fn case_shapes(stem: &str) -> Vec<String> {
    let lower = stem.to_lowercase();
    let upper = stem.to_uppercase();
    let sentence = {
        let mut cs = lower.chars();
        match cs.next() {
            Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
            None => String::new(),
        }
    };
    vec![lower, upper, sentence]
}

/// Every deferral stem crossed with trailing punctuation, letter case and the
/// bullet wrappers a markdown section arrives in.
///
/// Derived rather than listed on purpose: an implementation that satisfies this
/// set by enumeration has to enumerate several hundred strings it cannot read
/// off this file, which is more work than normalising and comparing.
fn multiply(stems: &[&str]) -> Vec<String> {
    let trailers = ["", ".", "!", "?", ":", "...", " -"];
    let wrappers: [fn(&str) -> String; 4] = [
        |s| s.to_string(),
        |s| format!("- {s}"),
        |s| format!("* {s}"),
        |s| format!("  {s}  "),
    ];

    let mut out: BTreeSet<String> = BTreeSet::new();
    for stem in stems {
        for shape in case_shapes(stem) {
            for trailer in trailers {
                let token = format!("{shape}{trailer}");
                for wrap in wrappers {
                    out.insert(wrap(&token));
                }
            }
        }
    }
    out.into_iter().collect()
}

fn derived_deferrals() -> Vec<String> {
    multiply(DEFERRAL_STEMS)
}

/// The phrase deferrals under the same multiplication.
///
/// Listing the four raw literals and nothing else left the whole family
/// satisfiable by `PHRASES.contains(&section.trim())` — four strings copied
/// straight off the constant. That gate then passes a done-when reading
/// `"See the linked issue."` or `"- see the linked issues"`, which is the
/// sentence-case-and-a-full-stop spelling a human actually types. Multiplying
/// them out makes copying the table strictly more work than normalising.
fn derived_phrase_deferrals() -> Vec<String> {
    multiply(PHRASE_DEFERRALS)
}

/// Asserts the gate blocked, and returns the message it blocked with.
///
/// `Failed` specifically: `Warning` and `NotMeasured` both certify, and
/// `Errored` would claim the gate tried to read something and could not.
#[track_caller]
fn expect_failed(status: &GateStatus, context: &str) -> String {
    match status {
        GateStatus::Failed(msg) => msg.clone(),
        other => panic!(
            "{context}: expected Failed, got {other:?}. Absence of Product's bar is the \
             defect itself — reporting it any other way lets the change certify, and \
             quality sign-off is then signing off on nothing.",
        ),
    }
}

/// The variant only, with no message. Used where two inputs must reach the same
/// verdict but may legitimately quote different text back at the author.
fn variant(status: &GateStatus) -> &'static str {
    match status {
        GateStatus::Passed => "Passed",
        GateStatus::AutoUpdated => "AutoUpdated",
        GateStatus::Warning(_) => "Warning",
        GateStatus::Failed(_) => "Failed",
        GateStatus::Errored(_) => "Errored",
        GateStatus::NotMeasured { .. } => "NotMeasured",
    }
}

fn names_the_bar(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("done-when") || m.contains("done when") || m.contains("acceptance")
}

fn names_the_problem(msg: &str) -> bool {
    msg.to_lowercase().contains("problem")
}

fn names(artifact: Artifact, msg: &str) -> bool {
    match artifact {
        Artifact::WrittenProblem => names_the_problem(msg),
        Artifact::DoneWhenBar => names_the_bar(msg),
    }
}

/// `msg` with every non-blank line of `body` subtracted from it.
///
/// What the gate says **on its own account**. Quoting the offending section
/// back at the author is legal and helpful — the module docs are explicit that
/// the `judge`/`missing_artifacts` split exists so the tests need not forbid it
/// — so the body's own text is removed before the message's vocabulary is
/// judged, and only the remainder is held to the rule.
///
/// Without this subtraction the message contract was satisfiable by a single
/// constant that names both artifacts every time:
///
///     GateStatus::Failed(format!(
///         "The change does not carry the Product artifact (a written problem and a
///          done-when acceptance bar). Body: {pr_body:?}"))
///
/// Every positive containment assertion holds, `expect_missing` only pins the
/// measurement set, and the three messages differ from one another because the
/// three *bodies* differ — the echoed body does the distinguishing, not the
/// measurement. An author whose bar is missing then reads a comment accusing
/// them of also not writing a problem statement they did write.
///
/// Longest lines first, so a line that contains a shorter one is removed whole,
/// and each removal leaves a space behind so subtracting a `-` cannot join two
/// words into vocabulary that was never written.
fn message_residue(msg: &str, body: &str) -> String {
    let mut lines: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines.sort_by_key(|l| std::cmp::Reverse(l.len()));

    let mut residue = msg.to_string();
    for line in lines {
        residue = residue.replace(line, " ");
    }
    residue
}

/// The measurement, normalised for comparison.
///
/// Render order is presentation, so this sorts before comparing. Duplication is
/// not presentation: an artifact reported twice renders as "a written problem
/// statement and a written problem statement" in the author-facing message, so
/// the raw vector is checked for it rather than quietly deduplicated.
#[track_caller]
fn missing(body: &str) -> Vec<Artifact> {
    let raw = product_bar::missing_artifacts(body);
    let mut got = raw.clone();
    got.sort();
    got.dedup();
    assert_eq!(
        got.len(),
        raw.len(),
        "the measurement named the same missing artifact more than once ({raw:?}); the \
         message rendered from it then repeats itself back at the author. body={body:?}"
    );
    got
}

/// The change produced both artifacts: `judge` passes and the measurement finds
/// nothing missing. Asserting both keeps the verdict and the message rendered
/// from one measurement rather than two disagreeing ones.
#[track_caller]
fn expect_passed(body: &str, context: &str) {
    let status = product_bar::judge(body);
    assert_eq!(
        status,
        GateStatus::Passed,
        "{context}: this change produced the Product artifact; failing it is a \
         fabricated accusation, which is the same defect as a false green pointed \
         the other way. body={body:?}"
    );
    assert!(
        missing(body).is_empty(),
        "{context}: judge() passed the change while missing_artifacts() still reports \
         {:?} absent. The verdict and the message must be rendered from one \
         measurement, or the scorecard and the comment contradict each other",
        missing(body)
    );
}

/// The change did not produce these artifacts, and only these.
///
/// Asserts the measurement exactly (so naming an artifact the author actually
/// wrote is caught as a fabricated accusation, without banning any word from the
/// prose), asserts the verdict is `Failed` (not `Warning`, `NotMeasured` or
/// `Errored`), and asserts the message names each missing artifact so the author
/// can act on it without reading the gate's source. Returns the message.
#[track_caller]
fn expect_missing(body: &str, expected: &[Artifact], context: &str) -> String {
    let mut want = expected.to_vec();
    want.sort();
    assert_eq!(
        missing(body),
        want,
        "{context}: the gate measured the wrong set of missing artifacts. Naming an \
         artifact the author did write tells them to write the thing they already \
         wrote; failing to name one they did not write hides the work. body={body:?}"
    );
    assert_failed_naming(body, &want, context)
}

/// The change is missing *at least* these artifacts.
///
/// Used where pinning the set exactly would decide something the specification
/// leaves open. A body of unheaded prose states no acceptance bar, so the bar is
/// missing beyond argument; whether that same prose also counts as the written
/// problem is a marker-recognition choice this suite deliberately leaves to the
/// implementer, and pinning it either way would be pinning the marker format.
#[track_caller]
fn expect_at_least_missing(body: &str, expected: &[Artifact], context: &str) -> String {
    let got = missing(body);
    for artifact in expected {
        assert!(
            got.contains(artifact),
            "{context}: the gate did not report the missing {artifact:?}. It reported \
             {got:?}. body={body:?}"
        );
    }
    assert_failed_naming(body, &got, context)
}

/// The shared tail of both: `Failed`, measured, unacceptable, and a message that
/// names every artifact the measurement found missing.
#[track_caller]
fn assert_failed_naming(body: &str, want: &[Artifact], context: &str) -> String {
    assert!(
        !want.is_empty(),
        "{context}: this is the failing side, so the measurement must report at least \
         one missing artifact; use expect_passed otherwise. body={body:?}"
    );

    let status = product_bar::judge(body);
    assert!(
        status.is_measured(),
        "{context}: the gate read the change and found {want:?} missing; that is a \
         measurement, and recording it as NotMeasured hides the defect behind \
         honest-looking bookkeeping. body={body:?}"
    );
    assert!(
        !status.is_acceptable(),
        "{context}: an acceptable status certifies, and quality cannot sign off \
         without Product's bar. body={body:?}"
    );
    let msg = expect_failed(&status, context);
    for artifact in want {
        assert!(
            names(*artifact, &msg),
            "{context}: the message must name the missing {artifact:?} so the author \
             can act on it without reading the gate's source; got {msg:?}"
        );
    }

    // And the negative, on what the gate said ON ITS OWN ACCOUNT. The positive
    // above is asserted on the whole message, so a gate may name the missing
    // artifact by quoting the heading it found empty. The negative is asserted
    // on the residue — the message with the body's own lines subtracted — so
    // quoting stays legal while naming an artifact the author DID write does
    // not. That is what stops one constant string that lists both artifacts,
    // plus an echo of the body, from satisfying this whole file: for a change
    // whose problem statement is present and whose bar is not, the residue must
    // not accuse the author over the problem statement they wrote.
    //
    // See open_questions: this forbids a message that reports the artifact that
    // IS present ("your problem statement is here, your done-when is not"),
    // which is helpful prose, and a human may prefer to pay that price the
    // other way.
    let residue = message_residue(&msg, body);
    for artifact in [Artifact::WrittenProblem, Artifact::DoneWhenBar] {
        if want.contains(&artifact) {
            continue;
        }
        assert!(
            !names(artifact, &residue),
            "{context}: the gate did not find {artifact:?} missing, and the author did \
             write it, but the message names it anyway on its own account — telling \
             them to go and write the thing they already wrote. Quoting the offending \
             section is legal and is subtracted before this check; this is the \
             message minus the body. Missing: {want:?}. Message: {msg:?}. Residue: \
             {residue:?}"
        );
    }

    msg
}

/// The failure-message vocabulary must not be smuggled in from the fixtures.
///
/// `message_residue` subtracts the body's own lines before the negative naming
/// rule is applied, so a fixture that itself said "problem" or "acceptance"
/// would silently exempt the gate from that rule. None of the content fixtures
/// does — this is what pins it, and it is asserted from inside a test rather
/// than standing alone so it is never green before the gate exists.
#[track_caller]
fn assert_the_content_fixtures_carry_none_of_the_message_vocabulary() {
    let background = long_background();
    for (name, fixture) in [
        ("PROBLEM", PROBLEM),
        ("BAR", BAR),
        ("MULTILINE_BAR", MULTILINE_BAR),
        ("SHORT_PROBLEM", SHORT_PROBLEM),
        ("SHORT_BAR", SHORT_BAR),
        ("INLINE_PROBLEM", INLINE_PROBLEM),
        ("INLINE_BAR", INLINE_BAR),
        ("KO_PROBLEM", KO_PROBLEM),
        ("KO_BAR", KO_BAR),
        ("THIRD_SECTION_BODY", THIRD_SECTION_BODY),
        ("LONG_BACKGROUND_LINES", background.as_str()),
        ("SHORT_REAL_CONTENT[0]", SHORT_REAL_CONTENT[0]),
        ("SHORT_REAL_CONTENT[1]", SHORT_REAL_CONTENT[1]),
        ("SHORT_REAL_CONTENT[2]", SHORT_REAL_CONTENT[2]),
        ("SHORT_REAL_CONTENT[3]", SHORT_REAL_CONTENT[3]),
    ] {
        assert!(
            !names_the_problem(fixture) && !names_the_bar(fixture),
            "fixture invariant: {name} must contain none of the vocabulary the failure \
             message is judged on, or subtracting it from the message exempts the gate \
             from the rule that message is held to. Fixture: {fixture:?}"
        );
    }

    // The tall background is checked stem by stem rather than whole: it is
    // sixty-six thousand lines, and its content is these seven strings.
    for stem in TALL_BACKGROUND_STEMS {
        assert!(
            !names_the_problem(stem) && !names_the_bar(stem),
            "fixture invariant: TALL_BACKGROUND_STEMS must contain none of the \
             vocabulary the failure message is judged on. Stem: {stem:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The measurement: what passes
// ---------------------------------------------------------------------------

#[test]
fn a_change_carrying_a_written_problem_and_a_done_when_bar_passes() {
    expect_passed(
        &body_with(PROBLEM, BAR),
        "a written problem and a done-when bar",
    );
}

#[test]
fn a_bar_written_as_measurable_criteria_passes() {
    // The bar is far more often a list than a sentence. A gate that only
    // accepts prose would push authors back to writing nothing.
    expect_passed(
        &body_with(PROBLEM, MULTILINE_BAR),
        "an acceptance bar expressed as checkable criteria is the artifact, not a \
         lesser form of it",
    );
}

#[test]
fn a_one_line_problem_and_a_one_line_bar_pass() {
    // A small change that has still done Product's job. `SHORT_BAR` is eleven
    // bytes — shorter than four of the placeholders in PLACEHOLDERS that must
    // fail — so no minimum length used INSTEAD OF a content check can admit this
    // and reject those. The gate has to discriminate on what the words say.
    //
    // That is not the whole of it, and this test used to be written as if it
    // were. A length floor bolted ON TOP OF a correct content check is a
    // different mistake, and `SHORT_BAR` does not falsify it: nine characters
    // and three words was the shortest content this file required to pass, so
    // `core.chars().count() >= 9` was green everywhere.
    // `content_passes_however_few_characters_and_words_it_takes` is what closes
    // that, with content of one and two words and no space in it at all.
    assert!(
        SHORT_BAR.len() < "TODO: write the acceptance criteria here".len(),
        "fixture invariant: the legitimate short bar must be shorter than the \
         longest placeholder, or this test stops forcing a content check"
    );

    expect_passed(
        &body_with(SHORT_PROBLEM, SHORT_BAR),
        "a one-line bet and a one-line, checkable bar are the artifact; rejecting \
         them because they are short measures effort and accuses an author who did \
         the job",
    );
}

#[test]
fn a_korean_problem_and_bar_pass() {
    // This corpus already carries Korean (src/compliance_guard/statutes.rs and
    // its siblings), so the gate will be handed non-ASCII bodies. Hangul is
    // three bytes per character: a byte-length rule and a character-length rule
    // disagree about this fixture, and the suite refuses to let either stand in
    // for the measurement.
    let body = body_with(KO_PROBLEM, KO_BAR);
    assert_ne!(
        body.len(),
        body.chars().count(),
        "fixture invariant: this body must have more bytes than characters, or it \
         stops separating a byte heuristic from a character heuristic"
    );

    expect_passed(
        &body,
        "a written problem and a done-when bar are the artifact in any language; \
         failing this accuses every author who does not write in English",
    );
}

#[test]
fn the_two_sections_may_be_written_in_either_order() {
    // Order is presentation. An author who states the bar first has produced
    // both artifacts, and a section extractor that assumes the problem comes
    // first either mis-slices this body or panics on it — see
    // `judge_returns_a_verdict_for_any_body_and_never_panics`.
    expect_passed(&body_with(PROBLEM, BAR), "the problem stated first");
    expect_passed(
        &format!("## Done when\n\n{BAR}\n\n## Problem\n\n{PROBLEM}\n"),
        "the done-when stated first",
    );
}

#[test]
fn the_same_two_words_are_the_marker_however_the_author_formats_them() {
    // The defect this closes, reproduced twice by review against a real
    // implementation: a suite whose every passing body used the byte strings
    // "## Problem" and "## Done when" admitted a gate that matched exactly
    // those two byte strings. Combined with the (correct) rule that a body
    // carrying no bar fails closed, that gate rejects "## Done When", "###
    // Done when", "## Done when:" and "**Done when**" — and this repository
    // ships no PULL_REQUEST_TEMPLATE forcing one spelling, so once wired into
    // seal() it withholds certification from essentially every real change.
    // Blocking everyone is not a safe direction to be wrong in; it is the
    // fabricated accusation at full incidence.
    //
    // Every marker below is the SAME TWO WORDS. Case, heading depth, a trailing
    // colon and a bold label are how markdown is written, not what it says.
    // Synonyms are not pinned here — which words announce a section stays open.
    //
    // Run over both line endings, because the two are not the same test. Every
    // marker here that a gate recognises with `ends_with` — `**Done when**`,
    // `## Done when:` — is defeated by the trailing `\r` a browser-submitted
    // body carries, and `body.split('\n')` instead of `body.lines()` is an
    // entirely ordinary way to write the extractor. Under LF alone this whole
    // cross-product is green for a gate that rejects the same complete artifact
    // the moment it is typed into the GitHub web UI.
    for pad in MARKER_PADDINGS {
        let padded = pad("## Done when");
        assert_ne!(
            padded, "## Done when",
            "fixture invariant: a padding must actually change the marker line, or the \
             whitespace family is the plain family run twice"
        );
        assert_eq!(
            padded.trim(),
            "## Done when",
            "fixture invariant: a padding must add whitespace and nothing else, or this \
             family stops being about whitespace"
        );
    }

    for eol in BOTH_EOLS {
        for problem_marker in PROBLEM_MARKERS {
            for done_when_marker in DONE_WHEN_MARKERS {
                expect_passed(
                    &as_eol(
                        &format!("{problem_marker}\n\n{PROBLEM}\n\n{done_when_marker}\n\n{BAR}\n"),
                        eol,
                    ),
                    &format!(
                        "a complete Product artifact under {problem_marker:?} and \
                         {done_when_marker:?}, {eol:?}"
                    ),
                );
            }
        }

        // THE SAME MARKERS WITH ORDINARY WHITESPACE ON THE LINE. See
        // `MARKER_PADDINGS`: every marker literal above is exactly terminated
        // and `as_eol` only ever appends `\r`, so a trailing space — markdown's
        // own line-break idiom, and what anyone who lines up their headings
        // types — was pinned nowhere on the passing side. A `heading_text` that
        // trims `\r` and nothing else passes all 84 fixtures above and then
        // reports BOTH artifacts missing from a complete, well-written body.
        for pad in MARKER_PADDINGS {
            for problem_marker in PROBLEM_MARKERS {
                for done_when_marker in DONE_WHEN_MARKERS {
                    let (p, d) = (pad(problem_marker), pad(done_when_marker));
                    expect_passed(
                        &as_eol(&format!("{p}\n\n{PROBLEM}\n\n{d}\n\n{BAR}\n"), eol),
                        &format!("a complete Product artifact under {p:?} and {d:?}, {eol:?}"),
                    );
                }
            }

            // The empty and deferred mirrors under each padding, so widening
            // the recognition cannot itself become a fail-open.
            for done_when_marker in DONE_WHEN_MARKERS {
                let d = pad(done_when_marker);
                expect_missing(
                    &as_eol(&format!("## Problem\n\n{PROBLEM}\n\n{d}\n\n"), eol),
                    &[Artifact::DoneWhenBar],
                    &format!("{d:?} with nothing under it, {eol:?}"),
                );
                expect_missing(
                    &as_eol(&format!("## Problem\n\n{PROBLEM}\n\n{d}\nTBD\n"), eol),
                    &[Artifact::DoneWhenBar],
                    &format!("a deferral on the line directly under {d:?}, {eol:?}"),
                );
            }
            for problem_marker in PROBLEM_MARKERS {
                let p = pad(problem_marker);
                expect_missing(
                    &as_eol(&format!("{p}\n\n\n## Done when\n\n{BAR}\n"), eol),
                    &[Artifact::WrittenProblem],
                    &format!("{p:?} with nothing under it, {eol:?}"),
                );
            }
        }

        // THE SAME MARKERS WITH NO BLANK LINE UNDER THEM. Until this family
        // existed, every passing body in the whole file — without exception —
        // put a blank line between the marker and its content, so two ordinary
        // markdown shapes were unpinned and the boundary rule the file forces
        // pushed the implementer straight into rejecting them:
        //
        //     **Done when**
        //     - p99 < 5ms
        //
        // Under `is_bold_only(line) && next_is_blank` the marker is not
        // recognised as a heading at all, so the gate reports BOTH artifacts
        // missing from a body that carries both. A list that starts on the line
        // after its bold label is not an exotic input; it is what an author who
        // does not double-space writes.
        for problem_marker in PROBLEM_MARKERS {
            for done_when_marker in DONE_WHEN_MARKERS {
                expect_passed(
                    &as_eol(
                        &format!(
                            "{problem_marker}\n{PROBLEM}\n\n{done_when_marker}\n{MULTILINE_BAR}\n"
                        ),
                        eol,
                    ),
                    &format!(
                        "a complete Product artifact whose content starts on the line \
                         directly under {problem_marker:?} and {done_when_marker:?}, {eol:?}"
                    ),
                );
            }
        }

        // THE COLON-TERMINATED MARKERS WITH THEIR CONTENT ON THE SAME LINE.
        // `"## Done when:"` is in DONE_WHEN_MARKERS, so an implementer strips
        // the colon and matches the heading text against the two words — an
        // `==`-shaped match, which `## Done when: p99 < 5ms` defeats. The
        // marker is then unrecognised and the bar is reported missing from a
        // change that stated one on the heading's own line.
        for problem_marker in PROBLEM_MARKERS.iter().filter(|m| m.ends_with(':')) {
            for done_when_marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
                expect_passed(
                    &as_eol(
                        &format!(
                            "{problem_marker} {INLINE_PROBLEM}\n\n{done_when_marker} {INLINE_BAR}\n"
                        ),
                        eol,
                    ),
                    &format!(
                        "both artifacts written on their own marker lines, \
                         {problem_marker:?} and {done_when_marker:?}, {eol:?}"
                    ),
                );
            }
        }
        // And each of them inline beside an ordinarily-headed counterpart, so
        // the recognition is not pinned only when both sections take the same
        // shape.
        for done_when_marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_passed(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker} {INLINE_BAR}\n"),
                    eol,
                ),
                &format!("a bar written on the {done_when_marker:?} line itself, {eol:?}"),
            );
        }
        for problem_marker in PROBLEM_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_passed(
                &as_eol(
                    &format!("{problem_marker} {INLINE_PROBLEM}\n\n## Done when\n\n{BAR}\n"),
                    eol,
                ),
                &format!("a problem written on the {problem_marker:?} line itself, {eol:?}"),
            );
        }

        // AND THE SAME WHITESPACE AFTER THE COLON. `## Done when:  p99 < 5ms`
        // is the inline family's version of the trailing-space hole above: an
        // extractor that takes the marker line's remainder verbatim and then
        // compares it, or one that splits on a single space, reads the bar as
        // absent from a line that states one.
        for done_when_marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_passed(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker}  {INLINE_BAR}\n"),
                    eol,
                ),
                &format!("a bar two spaces after {done_when_marker:?}, {eol:?}"),
            );
            expect_missing(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker}  TBD\n"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!("a deferral two spaces after {done_when_marker:?}, {eol:?}"),
            );
        }
        for problem_marker in PROBLEM_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_passed(
                &as_eol(
                    &format!("{problem_marker}  {INLINE_PROBLEM}\n\n## Done when\n\n{BAR}\n"),
                    eol,
                ),
                &format!("a problem two spaces after {problem_marker:?}, {eol:?}"),
            );
        }

        // The mirrors for both widenings, so neither can fail open. A marker
        // whose content starts on the next line still has to be judged on that
        // content, and a marker with a deferral on its own line is a deferral.
        for done_when_marker in DONE_WHEN_MARKERS {
            expect_missing(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker}\nTBD\n"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!("a deferral on the line directly under {done_when_marker:?}, {eol:?}"),
            );
        }
        for done_when_marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_missing(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker} TBD\n"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!("a deferral written on the {done_when_marker:?} line itself, {eol:?}"),
            );
        }

        // The mirror, so widening recognition cannot itself become a fail-open:
        // an empty section under any of those spellings is still an empty
        // section.
        for done_when_marker in DONE_WHEN_MARKERS {
            expect_missing(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker}\n\n"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!("{done_when_marker:?} with nothing under it, {eol:?}"),
            );
        }
        for problem_marker in PROBLEM_MARKERS {
            expect_missing(
                &as_eol(
                    &format!("{problem_marker}\n\n\n## Done when\n\n{BAR}\n"),
                    eol,
                ),
                &[Artifact::WrittenProblem],
                &format!("{problem_marker:?} with nothing under it, {eol:?}"),
            );
        }
    }
}

#[test]
fn a_body_that_does_not_end_in_a_newline_is_still_the_artifact() {
    // The shape the GitHub API returns for the majority of pull request bodies,
    // and until this test the whole passing side of this file was terminated —
    // `body_with`, `bar_only`, `long_body_with_bar_at_the_end` and every inline
    // `format!` in the marker, boundary and CRLF families. The unterminated
    // shape appeared only among must-fail inputs, in `awkward_bodies`, whose own
    // comment says GitHub bodies routinely have no trailing newline. A shape
    // pinned only on the failing side can be rejected on sight and the rejection
    // is invisible — module-doc property (9), applied to the line ending.
    //
    // `let nl = rest.find('\n')?;` is an entirely ordinary way to read the line
    // a marker sits on, and it returns None exactly when the marker line ends
    // the body. See `unterminated`.
    for eol in BOTH_EOLS {
        expect_passed(
            &unterminated(&as_eol(&body_with(PROBLEM, BAR), eol)),
            &format!("a complete artifact, problem first, with no trailing newline, {eol:?}"),
        );
        expect_passed(
            &unterminated(&as_eol(&body_with(PROBLEM, MULTILINE_BAR), eol)),
            &format!(
                "a complete artifact whose last line is the last criterion of a \
                 multi-line bar, with no trailing newline, {eol:?}"
            ),
        );
        expect_passed(
            &unterminated(&as_eol(
                &format!("## Done when\n\n{BAR}\n\n## Problem\n\n{PROBLEM}\n"),
                eol,
            )),
            &format!("a complete artifact, done-when first, with no trailing newline, {eol:?}"),
        );

        // The inline form, where the marker line itself is the last line of the
        // body and there is no `\n` after it at all.
        for done_when_marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_passed(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker} {INLINE_BAR}"),
                    eol,
                ),
                &format!(
                    "a bar written on the {done_when_marker:?} line, which is the last \
                     line of the body, {eol:?}"
                ),
            );
        }

        // The mirrors: the same bodies with the last section emptied or
        // deferred, still unterminated, so widening the passing side here
        // cannot itself become a fail-open. A marker that ends the body has
        // nothing under it, and that is exactly what an empty heading is.
        expect_missing(
            &as_eol(&format!("## Problem\n\n{PROBLEM}\n\n## Done when"), eol),
            &[Artifact::DoneWhenBar],
            &format!("a done-when marker as the unterminated last line of the body, {eol:?}"),
        );
        expect_missing(
            &as_eol(&format!("## Done when\n\n{BAR}\n\n## Problem"), eol),
            &[Artifact::WrittenProblem],
            &format!("a problem marker as the unterminated last line of the body, {eol:?}"),
        );
        for done_when_marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_missing(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker} TBD"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!(
                    "a deferral on the {done_when_marker:?} line, which is the last \
                     line of the body, {eol:?}"
                ),
            );
        }
    }
}

#[test]
fn a_third_section_after_the_artifacts_does_not_hide_them() {
    // The counterpart to `a_third_section_does_not_hide_an_empty_one`. Without
    // this half, the fix for that one degenerates into "ignore everything after
    // the marker" — which reads a bar of zero length and fails every filled-in
    // template instead. The boundary has to be pinned in both directions or the
    // slicing behaviour stays unspecified in both.
    // Both line endings: `**Testing**` and `Testing:` are recognised by an
    // `ends_with` test, which a trailing `\r` defeats, so under CRLF a gate can
    // stop seeing the boundary entirely — and then a complete artifact whose
    // bar is followed by testing notes is judged on a section it mis-sliced.
    for eol in BOTH_EOLS {
        for header in THIRD_SECTION_HEADERS {
            let third = third_section(header);

            expect_passed(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n## Done when\n\n{BAR}\n\n{third}"),
                    eol,
                ),
                &format!("a complete artifact followed by a section headed {header:?}, {eol:?}"),
            );

            // A multi-line bar followed by a third section: an extractor that
            // takes only the first line after the marker still sees a bar here,
            // but one that takes nothing does not.
            expect_passed(
                &as_eol(
                    &format!(
                        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n{MULTILINE_BAR}\n\n{third}"
                    ),
                    eol,
                ),
                &format!("a three-line bar followed by a section headed {header:?}, {eol:?}"),
            );

            // A third section wedged BETWEEN the two artifacts.
            expect_passed(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{third}\n## Done when\n\n{BAR}\n"),
                    eol,
                ),
                &format!("a section headed {header:?} between the problem and the bar, {eol:?}"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The measurement: what fails
// ---------------------------------------------------------------------------

#[test]
fn a_third_section_does_not_hide_an_empty_one() {
    // The headline gap in the previous revision, reproduced independently by
    // both reviewers: no fixture in the file had a third section, so an
    // extractor that ran the done-when to the next occurrence of "\n## " was
    // never falsified. It returned Passed for a body whose done-when heading is
    // empty and whose next section is `### Testing`, `**Testing**` or
    // `# Testing`, because none of those is "\n## " — swallowing the testing
    // notes as the acceptance bar.
    //
    // That is the pasted-template defect in the exact shape real templates
    // produce: Problem / Done when / Testing, with the middle one skipped. The
    // author wrote nothing under the heading; what a later section happens to
    // say is not their acceptance bar, however many words it is.
    // Under BOTH line endings, because LF alone leaves this whole family green
    // for a gate that fails open on the shape GitHub's web UI actually submits.
    // `str::lines()` strips a trailing `\r` and `.trim()` eats it, so a gate
    // that finds its boundary with `ends_with("**")` or `ends_with(':')` on
    // `body.split('\n')` sees `**Testing**\r` as ordinary prose — and then
    // swallows the testing notes as the acceptance bar of an empty
    // `## Done when`. That is the headline defect of this file, certified, in
    // the exact line endings a browser produces.
    //
    // `BOUNDARY_HEADERS`, not `THIRD_SECTION_HEADERS`, and two entries are
    // deliberately absent from it. `"Testing:"` is no longer required to end a
    // section, because the rule that makes it end one also rejects
    // `Acceptance criteria:` above a real list of bullets; `"### Testing"` is no
    // longer required either, because the rule that makes a heading DEEPER than
    // the marker end the section also rejects a real bar written under
    // `### Criteria`. Both are the same trade and both are decided in the same
    // direction. See BOUNDARY_HEADERS' own docs for the decisions and their
    // vetoes, the module docs' "What this suite does NOT close" for what each
    // one costs, and `assert_the_boundary_families_state_one_consistent_rule`
    // for the invariant that stops the families drifting apart.
    for eol in BOTH_EOLS {
        for header in BOUNDARY_HEADERS {
            let third = third_section(header);

            for filler in ["", "   ", "TBD", "- [ ]"] {
                expect_missing(
                    &as_eol(
                        &format!("## Problem\n\n{PROBLEM}\n\n## Done when\n\n{filler}\n\n{third}"),
                        eol,
                    ),
                    &[Artifact::DoneWhenBar],
                    &format!(
                        "a done-when section holding only {filler:?}, followed by a section \
                         headed {header:?}, {eol:?}"
                    ),
                );

                // The mirror, so the problem-side extractor is pinned the same
                // way.
                expect_missing(
                    &as_eol(
                        &format!("## Problem\n\n{filler}\n\n{third}\n## Done when\n\n{BAR}\n"),
                        eol,
                    ),
                    &[Artifact::WrittenProblem],
                    &format!(
                        "a problem section holding only {filler:?}, followed by a section \
                         headed {header:?}, {eol:?}"
                    ),
                );

                // Both empty, with the third section carrying all the prose in
                // the body. Neither artifact exists; the body is not short.
                expect_missing(
                    &as_eol(
                        &format!("## Problem\n\n{filler}\n\n## Done when\n\n{filler}\n\n{third}"),
                        eol,
                    ),
                    &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
                    &format!(
                        "both sections holding only {filler:?}, with a section headed \
                         {header:?} carrying the only prose in the body, {eol:?}"
                    ),
                );
            }
        }
    }
}

#[test]
fn a_heading_deeper_than_the_marker_is_content_inside_the_section() {
    // THE THIRD BOUNDARY DECISION, and the half of it that costs the gate
    // something. `BOUNDARY_HEADERS` used to carry `### Testing`, so an empty
    // `## Done when` had to be terminated by a heading DEEPER than the marker
    // itself — and no fixture anywhere in this file ever put a heading inside a
    // section, above that section's content. The only rule satisfying the family
    // was "any ATX heading ends the section above it", and its unwritten cost is
    // the body below: a real, checkable, three-item acceptance bar under an
    // ordinary sub-heading, reported missing.
    //
    // That is this file's own definition of a fabricated accusation, and it is
    // structurally identical to the `Acceptance criteria:` collision the suite
    // already met and decided — two demands only the English can tell apart. It
    // is decided here in the same direction, so an implementer meets it as a
    // decision rather than as a live production defect: a heading deeper than
    // the marker is nested content.
    //
    // BOTH HALVES, or the widened reading becomes a fail-open of its own. The
    // sub-heading is not itself content — an emptied section under one is still
    // empty, however the emptying is spelled — and a heading at the marker's own
    // depth or shallower still terminates, which `a_third_section_does_not_hide_an_empty_one`
    // pins over `# Testing` and `## Testing`.
    //
    // The vocabulary stays open, which is the reason the sub-headings below are
    // safe to write: a gate generous enough to read `### Criteria` as a
    // done-when marker of its own reaches the same verdict on every fixture here
    // — the criteria section carries the bar in the passing half and carries
    // nothing in the failing half. Nothing here obliges the gate to recognise
    // the word, and nothing here punishes it for doing so.
    assert_the_boundary_families_state_one_consistent_rule();

    for eol in BOTH_EOLS {
        for header in ["### Criteria", "#### Criteria"] {
            expect_passed(
                &as_eol(
                    &format!(
                        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n{header}\n\n{MULTILINE_BAR}\n"
                    ),
                    eol,
                ),
                &format!(
                    "a real acceptance bar under the sub-heading {header:?} inside the \
                     done-when section, {eol:?}"
                ),
            );
        }
        for header in ["### Background", "#### Background"] {
            expect_passed(
                &as_eol(
                    &format!("## Problem\n\n{header}\n\n{PROBLEM}\n\n## Done when\n\n{BAR}\n"),
                    eol,
                ),
                &format!(
                    "a real problem statement under the sub-heading {header:?} inside the \
                     problem section, {eol:?}"
                ),
            );
        }

        // THE MIRROR. A sub-heading is a heading and not content, so a section
        // that holds one and nothing else is still an empty section — otherwise
        // this decision hands every pasted template a way to certify by adding
        // one `###` line.
        for filler in ["", "   ", "TBD", "- [ ]"] {
            expect_missing(
                &as_eol(
                    &format!(
                        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n### Criteria\n\n{filler}\n"
                    ),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!(
                    "a done-when section holding the sub-heading \"### Criteria\" over \
                     only {filler:?}, {eol:?}"
                ),
            );
            expect_missing(
                &as_eol(
                    &format!("## Problem\n\n### Background\n\n{filler}\n\n## Done when\n\n{BAR}\n"),
                    eol,
                ),
                &[Artifact::WrittenProblem],
                &format!(
                    "a problem section holding the sub-heading \"### Background\" over \
                     only {filler:?}, {eol:?}"
                ),
            );
        }
    }
}

#[test]
fn a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured() {
    expect_missing(
        "",
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        "a change with no problem statement and no bar",
    );
}

#[test]
fn a_real_pull_request_body_with_no_bar_fails_closed() {
    // THE HEADLINE CASE. Every other failing fixture in this file is built from
    // this file's own heading template, which is the one input shape where an
    // implementation has no temptation to fail open. These are the bodies
    // people actually submit: ordinary prose with no headings at all, a
    // template nobody filled in, and a template filled in beautifully under
    // headings that are not the Product artifact. A gate that answers
    // `NotMeasured` here — "the body carries no section I recognise" — is
    // acceptable to `is_certified_ready`, so it certifies the majority of real
    // pull requests while the scorecard names a Product gate that measured
    // nothing. That is precisely the false green this seat exists to prevent.
    //
    // None of these bodies states an acceptance criterion in any spelling, so
    // none of them collides with the gate's freedom over the marker format: the
    // point is that no bar exists, not that no heading exists. For the same
    // reason all of them but the last two are pinned with
    // `expect_at_least_missing` — whether unheaded prose, or a `## Summary`,
    // also counts as the written problem is a recognition choice this suite
    // leaves open, while the absence of the bar is not open at all.
    let at_least: Vec<(&str, String)> = vec![
        (
            "plain prose with no headings — the commonest real body there is",
            "Refactors the canary poller onto the shared HTTP client so the retry budget \
             is configured in one place instead of three. No behaviour change is intended."
                .to_string(),
        ),
        (
            "a one-line description, the shape most drive-by fixes carry",
            "Bumps the tracing subscriber to 0.3.19.".to_string(),
        ),
        (
            "an emoji-and-link body, which defers the artifact to somewhere else",
            "🚀 see https://example.invalid/issues/4192".to_string(),
        ),
    ];

    // A FULLY-FILLED TEMPLATE WHOSE HEADINGS ARE REAL AND ARE NOT THE PRODUCT
    // ARTIFACT. Everything else on this test's failing side is header-less —
    // plain prose, a one-liner, an emoji and a link, an unfilled comment
    // template, whitespace — and `THIRD_SECTION_BODY` is pinned as not-an-
    // artifact only in bodies that ALSO carry a done-when marker with nothing
    // under it. So no fixture anywhere required a body with real, well-written,
    // non-Product headings and no done-when marker in any spelling to fail.
    //
    // The module docs deliberately leave the marker vocabulary open and invite
    // generosity — "an implementation that also recognises `## Acceptance
    // criteria`, `## Why`, a YAML block or unheaded prose passes unchanged" —
    // which steers an implementer straight at
    //
    //     matches!(t, "done when" | "acceptance criteria" | "acceptance"
    //                | "test plan" | "testing" | "validation" | "verification")
    //
    // and a test plan is not an acceptance bar: it says what the author ran,
    // not what done looks like. Handed the bodies below, that gate reports the
    // bar PRESENT and certifies a very large fraction of real pull requests
    // that state no acceptance bar at all, with all of this suite green.
    //
    // These sit in `at_least`, not in `exactly_both`, so the same freedom stays
    // open over the PROBLEM half: whether `## Summary` prose counts as the
    // written problem is a recognition choice this suite leaves to the
    // implementer. The absence of the bar is not open.
    //
    // This is the decision `a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it`
    // already made in prose — "no reading of 'Testing', 'Rollout' or 'Notes' is
    // a synonym for the bet or the bar" — made enforceable when the done-when
    // marker is ABSENT rather than only when it is present and empty.
    let real_headings_no_bar: Vec<(&str, String)> = vec![
        (
            "a well-written template stating a summary and a test plan, and no \
             acceptance bar in any spelling",
            "## Summary\n\nRefactors the canary poller onto the shared HTTP client so \
             the retry budget is configured in one place instead of three.\n\n\
             ## Test plan\n\n- `cargo test --all`\n- manual smoke on staging\n"
                .to_string(),
        ),
        (
            "the same shape under `## Testing`, the other spelling every template \
             ships",
            format!(
                "## Summary\n\nBumps the tracing subscriber to 0.3.19 and drops the \
                 vendored fork it was pinned to.\n\n## Testing\n\n{THIRD_SECTION_BODY}\n"
            ),
        ),
    ];

    for (context, body) in &real_headings_no_bar {
        let lowered = body.to_lowercase();
        assert!(
            !lowered.contains("done when")
                && !lowered.contains("done-when")
                && !lowered.contains("acceptance"),
            "fixture invariant: {context} must state no acceptance bar in any \
             spelling, or it stops pinning the absence of the ARTIFACT and starts \
             pinning the absence of a heading this file happens to know. body={body:?}"
        );
        assert!(
            body.matches("\n## ").count() >= 1 && body.starts_with("## "),
            "fixture invariant: {context} must carry two real headings over real \
             content, or it stops separating 'this body states no acceptance bar' \
             from 'this body has no headings'. body={body:?}"
        );
        assert!(
            body.lines().filter(|l| !l.trim().is_empty()).count() >= 4,
            "fixture invariant: {context} must be a filled-in template, not an empty \
             one; an empty heading is already pinned elsewhere. body={body:?}"
        );
    }

    // These two carry nothing at all, under any reading, so the set is exact.
    let exactly_both: Vec<(&str, String)> = vec![
        (
            "an unfilled pull request template: HTML comments and nothing else",
            "<!-- Describe your change -->\n\n<!-- Done when? -->\n".to_string(),
        ),
        (
            "whitespace only, which is what an author who deleted the template leaves",
            "   \n\t\n  \r\n  ".to_string(),
        ),
    ];

    for (context, body) in at_least
        .iter()
        .chain(real_headings_no_bar.iter())
        .chain(exactly_both.iter())
    {
        assert!(
            !body.contains("Done when") || body.starts_with("<!--"),
            "fixture invariant: {context} must not carry this file's heading over real \
             content, or it stops testing the marker-less shape"
        );
    }

    for (context, body) in at_least.iter().chain(real_headings_no_bar.iter()) {
        expect_at_least_missing(body, &[Artifact::DoneWhenBar], context);
    }
    for (context, body) in &exactly_both {
        expect_missing(
            body,
            &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
            context,
        );
    }
}

#[test]
fn a_problem_statement_with_no_acceptance_bar_fails_however_long_the_prose() {
    // Guards the shallow check "the body is long, so it must say something".
    // This body is four paragraphs of genuine problem analysis and contains no
    // statement of what done looks like.
    expect_missing(
        &problem_only(&long_prose()),
        &[Artifact::DoneWhenBar],
        "a long problem statement with no bar",
    );
}

#[test]
fn an_acceptance_bar_with_no_written_problem_fails_however_long_the_bar() {
    // The artifact is "written problem + done-when". A bar with no bet behind
    // it cannot be judged: there is nothing to say whether the bar is the right
    // bar. The long variant is the mirror of the test above — a substantial
    // done-when cannot carry an absent problem statement.
    for bar in [BAR.to_string(), long_bar()] {
        expect_missing(
            &bar_only(&bar),
            &[Artifact::WrittenProblem],
            "a bar with no written problem",
        );
    }
}

#[test]
fn a_heading_with_nothing_under_it_fails_however_much_the_other_section_says() {
    // The two long cases are half the reason a global length threshold cannot
    // satisfy this suite: `long_prose()` under an empty done-when is longer than
    // every short passing body, and it must fail. The other half — a passing
    // body longer than every failing one, so the sets overlap at that end too
    // and a gate reading only a prefix of the change is falsified — is
    // `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact`. Neither is
    // sufficient alone: bracketing from this side rules out a threshold and says
    // nothing about a truncation.
    let cases: Vec<(String, Vec<Artifact>, &str)> = vec![
        (
            body_with(PROBLEM, ""),
            vec![Artifact::DoneWhenBar],
            "the done-when heading is present with nothing under it",
        ),
        (
            body_with("", BAR),
            vec![Artifact::WrittenProblem],
            "the problem heading is present with nothing under it",
        ),
        (
            body_with("", ""),
            vec![Artifact::WrittenProblem, Artifact::DoneWhenBar],
            "both headings present, both empty — a pasted template is not the artifact",
        ),
        (
            body_with(&long_prose(), ""),
            vec![Artifact::DoneWhenBar],
            "four paragraphs of problem analysis above an empty done-when heading; \
             length is not a bar",
        ),
        (
            body_with("", &long_bar()),
            vec![Artifact::WrittenProblem],
            "a long done-when above an empty problem heading; a bar with no bet \
             cannot be judged",
        ),
    ];

    let longest_failing = cases.iter().map(|(b, _, _)| b.len()).max().unwrap_or(0);
    assert!(
        longest_failing > body_with(PROBLEM, BAR).len(),
        "fixture invariant: some body that must fail has to be longer than every \
         body that must pass, or total length alone still separates the two sets"
    );

    for (body, expected, context) in &cases {
        expect_missing(body, expected, context);
    }
}

#[test]
fn a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact() {
    // The other half of the length property, and the half the file argued for
    // and did not have. The test above brackets the passing set from ABOVE: some
    // body that must fail is longer than every body that must pass, so no
    // threshold on total length can separate the sets. That closes off a
    // threshold and nothing else.
    //
    // It does not close off a TRUNCATION, and truncation is the direction that
    // produces a fabricated accusation rather than a false green. Review
    // verified it by mutation against a working implementation: wrapping the
    // entry point in `let pr_body = &pr_body[..450.min(pr_body.len())]` passed
    // every behavioural test in this file, and so did
    // `pr_body.lines().take(18)`. Four hundred bytes and sixteen lines were
    // caught; four hundred and fifty and eighteen were not, because no must-pass
    // body anywhere put its acceptance bar further in than that. A bounded
    // regex, a `.take(n)`, a `&body[..N]` "safety" clamp and an extractor that
    // stops after the first two sections are all ordinary ways to write this
    // gate, and every one of them ships a comment telling the author of a long,
    // careful pull request that they wrote no bar.
    //
    // So this pins the bracket from BELOW: a body that must pass, longer than
    // every body in the file that must fail, with its bar at the far end of a
    // long stretch of background. Its mirror — the same body with that one
    // section emptied — must still fail, so widening the passing side this way
    // cannot itself become a fail-open.
    //
    // AT THREE MAGNITUDES, which is what the previous revision lacked. Pinning
    // ONE size closes every clamp below it and no clamp above it, and the
    // module docs conceded as much. Review measured the survivor rather than
    // reasoning about it: `let pr_body = &pr_body[..2100.min(pr_body.len())];`
    // written at the top of `missing_artifacts` — and `pr_body.lines().take(50)`
    // — each passed all forty tests in this file and left the whole repository
    // green, while telling every author of a body over two kilobytes or fifty
    // lines that they wrote no acceptance bar. `truncated_argument` already
    // rejects the byte-identical mutation one line higher, at the call site, so
    // one magnitude was this file's own standard applied inconsistently one
    // layer down.
    //
    // A clamp at any fixed byte count or line count now has to sit above the
    // largest fixture to pass the passing side and below the smallest to fail
    // the failing side. No number does both.
    //
    // AND IN BOTH UNITS, which is what the previous revision claimed and did not
    // measure. `LONG_BACKGROUND_LINES` is thirty-eight lines, so even x64 is
    // about two and a half thousand — while a GitHub body may hold 65,536
    // characters and therefore up to 65,536 LINES. Review inserted a line clamp
    // at the top of `missing_artifacts` against a working reference gate:
    //
    //     pr_body.lines().take(200)   -> FAILED. 40 passed; 1 failed
    //     pr_body.lines().take(3000)  -> ok. 41 passed; 0 failed
    //     pr_body.lines().take(5000)  -> ok. 41 passed; 0 failed
    //
    // The byte direction had the same gap in the other unit: the limit is 65,536
    // CHARACTERS, and a character may be four bytes, so a body inside GitHub's
    // limit reaches 262,144 bytes and a byte clamp at 200,000 also passed all
    // forty-one. Repeating this prose background is the wrong tool for either —
    // sixty-five thousand lines of it is a megabyte and a half. So the family
    // gains one more member built the other way round, out of short single-token
    // lines: enough of them to clear `GITHUB_MAX_BODY_CHARS` in LINES, and
    // enough bytes with it to clear `GITHUB_MAX_BODY_BYTES`. See
    // `TALL_BACKGROUND_LINES`.
    let mut fixtures: Vec<(String, String, String)> = LONG_BODY_SCALES
        .iter()
        .map(|&times| {
            (
                format!("x{times} of prose background"),
                long_body_with_bar_at_the_end(times, MULTILINE_BAR),
                long_body_with_bar_at_the_end(times, ""),
            )
        })
        .collect();
    let tall = tall_background();
    fixtures.push((
        format!("{TALL_BACKGROUND_LINES} short lines of background"),
        body_around(&tall, MULTILINE_BAR),
        body_around(&tall, ""),
    ));

    // The fixture invariants first, so they are exercised rather than stranded
    // behind the measurement.
    let longest_failing = [
        body_with(&long_prose(), ""),
        body_with("", &long_bar()),
        body_with(&long_prose(), "TODO: write the acceptance criteria here"),
        problem_only(&long_prose()),
        bar_only(&long_bar()),
    ]
    .iter()
    .map(String::len)
    .max()
    .unwrap();
    assert!(
        LONG_BACKGROUND_LINES.len() > 30,
        "fixture invariant: the background must be spread over more than thirty lines, \
         or a `.take(n)` extractor is still unfalsified at the smallest magnitude; got \
         {}",
        LONG_BACKGROUND_LINES.len()
    );

    let mut marker_bytes: Vec<usize> = Vec::new();
    let mut marker_lines: Vec<usize> = Vec::new();
    for (name, passing, mirror) in &fixtures {
        assert!(
            passing.len() > longest_failing && mirror.len() > longest_failing,
            "fixture invariant: the long body of {name} ({} bytes) has to be longer \
             than every body that must fail on its own account ({longest_failing} \
             bytes), or the two sets do not overlap in length at this end and a gate \
             that reads only a prefix of the change is unfalsifiable",
            passing.len()
        );

        let marker_byte = passing
            .find("## Done when")
            .expect("fixture invariant: the long body must carry a done-when marker");
        let marker_line = passing
            .lines()
            .position(|l| l.trim() == "## Done when")
            .expect("fixture invariant: the done-when marker must be on a line of its own");
        marker_bytes.push(marker_byte);
        marker_lines.push(marker_line);
    }

    let (shallowest_byte, deepest_byte) = (
        *marker_bytes.iter().min().unwrap(),
        *marker_bytes.iter().max().unwrap(),
    );
    let (shallowest_line, deepest_line) = (
        *marker_lines.iter().min().unwrap(),
        *marker_lines.iter().max().unwrap(),
    );
    assert!(
        shallowest_byte > 1500 && shallowest_line > 25,
        "fixture invariant: even the smallest of these must put the acceptance bar deep \
         enough into the body that a gate reading a short prefix cannot see the marker \
         at all; got byte {shallowest_byte}, line {shallowest_line}"
    );
    assert!(
        deepest_byte > shallowest_byte * 10 && deepest_line > shallowest_line * 10,
        "fixture invariant: the deepest acceptance bar must sit at least an order of \
         magnitude past the shallowest one, in BOTH units, or a clamp between the two \
         still passes every fixture here while truncating real pull request bodies. \
         Bytes {shallowest_byte} -> {deepest_byte}; lines {shallowest_line} -> \
         {deepest_line}"
    );
    let longest_passing = fixtures.iter().map(|(_, p, _)| p.len()).max().unwrap();
    let longest_failing_anywhere = fixtures
        .iter()
        .map(|(_, _, m)| m.len())
        .chain(std::iter::once(longest_failing))
        .max()
        .unwrap();
    assert!(
        longest_passing > longest_failing_anywhere,
        "fixture invariant: the longest body that must PASS ({longest_passing} bytes) \
         has to be longer than every body in this file that must fail \
         ({longest_failing_anywhere} bytes), or a threshold on total length still \
         separates the two sets"
    );

    // THE TWO ABSOLUTE BOUNDS, one per unit, which is what makes this family a
    // CLOSE rather than a widening. A clamp low enough to be a real defect fails
    // one of these fixtures; a clamp high enough to survive them is above every
    // body the guard layer can ever be handed, and truncates nothing.
    //
    // In BYTES against `GITHUB_MAX_BODY_BYTES` and not against
    // `GITHUB_MAX_BODY_CHARS`: `String::len()` counts bytes, GitHub's limit
    // counts characters, and a character may be four bytes. The previous
    // revision compared the two directly and read as a stronger claim than it
    // made.
    assert!(
        longest_passing > GITHUB_MAX_BODY_BYTES,
        "fixture invariant: the longest body that must PASS ({longest_passing} bytes) \
         has to exceed the longest body GitHub can deliver, in BYTES \
         ({GITHUB_MAX_BODY_BYTES} = {GITHUB_MAX_BODY_CHARS} characters at up to four \
         bytes each). Anything less leaves a byte clamp above the fixtures and below \
         a real body — review measured one at 200,000 that passed the whole file"
    );

    // And in LINES, which the previous revision claimed and did not have. A body
    // cannot hold more lines than characters, so clearing
    // `GITHUB_MAX_BODY_CHARS` lines puts every fixed line clamp that survives
    // this family above every body GitHub can deliver.
    let most_lines = fixtures
        .iter()
        .map(|(_, p, _)| p.lines().count())
        .max()
        .unwrap();
    assert!(
        most_lines > GITHUB_MAX_BODY_CHARS,
        "fixture invariant: the passing side has to reach more LINES ({most_lines}) \
         than a GitHub pull request body can hold characters \
         ({GITHUB_MAX_BODY_CHARS}), or a line clamp between the tallest fixture and \
         the real bound survives this file and silently truncates real bodies. Review \
         measured `pr_body.lines().take(3000)` and `.take(5000)` passing all \
         forty-one tests against a working gate"
    );

    for (name, passing, mirror) in &fixtures {
        for eol in BOTH_EOLS {
            expect_passed(
                &as_eol(passing, eol),
                &format!(
                    "{name}, with a real acceptance bar at the end of it ({} bytes, {} \
                     lines); reading only the opening of a change and reporting the \
                     rest absent is a fabricated accusation aimed squarely at the \
                     authors who wrote the most, {eol:?}",
                    passing.len(),
                    passing.lines().count()
                ),
            );
            expect_missing(
                &as_eol(mirror, eol),
                &[Artifact::DoneWhenBar],
                &format!(
                    "the same {name} with the done-when section emptied; length is not \
                     the measurement in this direction either, {eol:?}"
                ),
            );
        }
    }
}

/// Runs one must-fail section through both positions and both-at-once.
///
/// Both positions matter: a gate that screens the done-when for substance and
/// settles for "non-empty" on the bet certifies half a template paste.
#[track_caller]
fn assert_placeholder_fails_in_both_sections(placeholder: &str, family: &str) {
    for problem in [PROBLEM, SHORT_PROBLEM] {
        expect_missing(
            &body_with(problem, placeholder),
            &[Artifact::DoneWhenBar],
            &format!("{family}: the done-when section contains only {placeholder:?}"),
        );
    }

    for bar in [BAR, SHORT_BAR] {
        expect_missing(
            &body_with(placeholder, bar),
            &[Artifact::WrittenProblem],
            &format!("{family}: the problem section contains only {placeholder:?}"),
        );
    }

    expect_missing(
        &body_with(placeholder, placeholder),
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        &format!("{family}: both sections contain only {placeholder:?}"),
    );
}

#[test]
fn a_placeholder_fails_in_either_section_however_much_the_other_one_says() {
    // The enumerated table, run through both positions against a short and a
    // normal counterpart. On its own this is copyable; the two tests below are
    // what make copying it insufficient.
    for placeholder in PLACEHOLDERS {
        assert_placeholder_fails_in_both_sections(placeholder, "enumerated placeholder");
    }

    // The long counterparts, so a length threshold cannot separate the sets.
    for placeholder in PLACEHOLDERS {
        expect_missing(
            &body_with(&long_prose(), placeholder),
            &[Artifact::DoneWhenBar],
            &format!("four paragraphs of problem above the placeholder {placeholder:?}"),
        );
        expect_missing(
            &body_with(placeholder, &long_bar()),
            &[Artifact::WrittenProblem],
            &format!("the placeholder {placeholder:?} above four paragraphs of bar"),
        );
    }
}

/// The first line of a fixture, which is all a marker line can carry.
fn first_line(token: &str) -> String {
    token.lines().next().unwrap_or("").to_string()
}

/// One shape of each stem, rotating the wrapper and the trailing punctuation so
/// the sample is not the same wrapper every time.
///
/// Every stem is represented, which a slice off the front of `multiply` would
/// not be: `multiply` sorts, so the first N entries are all the leading-space
/// wrapper of whichever stems sort first.
fn one_shape_per_stem(stems: &[&str]) -> Vec<String> {
    stems
        .iter()
        .enumerate()
        .map(|(i, stem)| {
            let shapes = multiply(&[stem]);
            shapes[i % shapes.len()].clone()
        })
        .collect()
}

/// One representative of every must-fail family, in the single-line shape a
/// marker line is able to carry.
///
/// The multi-line members (`"   \n   \n"`, the two-line `UNICODE_BLANKS`, the
/// multi-line template prompts) contribute their first line: what a marker line
/// can hold is one line, so that is the shape of them this family can pin.
fn single_line_must_fail_tokens() -> Vec<(String, &'static str)> {
    fn push(out: &mut Vec<(String, &'static str)>, token: String, family: &'static str) {
        if !out.iter().any(|(t, _)| *t == token) {
            out.push((token, family));
        }
    }

    let mut out: Vec<(String, &'static str)> = Vec::new();
    for placeholder in PLACEHOLDERS {
        push(
            &mut out,
            first_line(placeholder),
            "an enumerated placeholder",
        );
    }
    push(
        &mut out,
        SINGLE_LINE_PROMPT_BAR.to_string(),
        "a single-line template prompt",
    );
    for token in one_shape_per_stem(DEFERRAL_STEMS) {
        push(&mut out, first_line(&token), "a derived deferral");
    }
    for token in one_shape_per_stem(PHRASE_DEFERRALS) {
        push(&mut out, first_line(&token), "a derived phrase deferral");
    }
    for blank in UNICODE_BLANKS {
        push(
            &mut out,
            first_line(blank),
            "a section blank only to a reader",
        );
    }
    for pointer in POINTERS {
        push(&mut out, first_line(pointer), "a pointer to somewhere else");
    }
    out
}

/// Runs one single-line must-fail token through the two marker shapes whose
/// content does NOT sit under a blank line: written on the marker's own line
/// after the colon, and written on the line directly below the marker.
///
/// `index` rotates the spelling of the section that SURVIVES. `message_residue`
/// subtracts the body's own lines from the message before the negative naming
/// rule is applied, so pinning every one of these against a byte-identical
/// `## Problem` / `## Done when` counterpart would let one constant message
/// spelling those two literals be subtracted out of the residue for exactly the
/// bodies where the rule bites — the hole
/// `the_message_holds_its_ground_however_the_surviving_section_is_headed` was
/// written for. No constant can embed thirteen spellings.
#[track_caller]
fn assert_token_fails_on_and_under_the_marker(index: usize, token: &str, family: &str) {
    assert!(
        !token.contains('\n'),
        "fixture invariant: {family} {token:?} must be one line, or it cannot be \
         written on a marker line at all and this family pins nothing about the \
         inline shape"
    );

    let surviving_problem = PROBLEM_MARKERS[index % PROBLEM_MARKERS.len()];
    let surviving_done_when = DONE_WHEN_MARKERS[index % DONE_WHEN_MARKERS.len()];

    for eol in BOTH_EOLS {
        // ON THE MARKER LINE, after the colon.
        for marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_missing(
                &as_eol(
                    &format!("{surviving_problem}\n\n{PROBLEM}\n\n{marker} {token}\n"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!("{family}: {token:?} written on the {marker:?} line itself, {eol:?}"),
            );
        }
        for marker in PROBLEM_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_missing(
                &as_eol(
                    &format!("{marker} {token}\n\n{surviving_done_when}\n\n{BAR}\n"),
                    eol,
                ),
                &[Artifact::WrittenProblem],
                &format!("{family}: {token:?} written on the {marker:?} line itself, {eol:?}"),
            );
        }

        // AND ON THE LINE DIRECTLY UNDER IT, with no blank line between. The
        // done-when half of this shape was pinned for exactly one token (`TBD`)
        // and the problem half for none at all.
        for marker in DONE_WHEN_MARKERS {
            expect_missing(
                &as_eol(
                    &format!("{surviving_problem}\n\n{PROBLEM}\n\n{marker}\n{token}\n"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!("{family}: {token:?} on the line directly under {marker:?}, {eol:?}"),
            );
        }
        for marker in PROBLEM_MARKERS {
            expect_missing(
                &as_eol(
                    &format!("{marker}\n{token}\n\n{surviving_done_when}\n\n{BAR}\n"),
                    eol,
                ),
                &[Artifact::WrittenProblem],
                &format!("{family}: {token:?} on the line directly under {marker:?}, {eol:?}"),
            );
        }
    }
}

/// The passing mirror of the same two shapes: real content on the marker line,
/// and real content on the line directly under it.
#[track_caller]
fn assert_content_passes_on_and_under_the_marker(index: usize, content: &str, family: &str) {
    assert!(
        !names_the_problem(content) && !names_the_bar(content),
        "fixture invariant: {family} must carry none of the vocabulary the failure \
         message is judged on, or subtracting it from the message exempts the gate \
         from the rule that message is held to. Content: {content:?}"
    );

    let counterpart_problem = PROBLEM_MARKERS[index % PROBLEM_MARKERS.len()];
    let counterpart_done_when = DONE_WHEN_MARKERS[index % DONE_WHEN_MARKERS.len()];

    for eol in BOTH_EOLS {
        for marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_passed(
                &as_eol(
                    &format!("{counterpart_problem}\n\n{PROBLEM}\n\n{marker} {content}\n"),
                    eol,
                ),
                &format!("{family}: {content:?} written on the {marker:?} line itself, {eol:?}"),
            );
        }
        for marker in PROBLEM_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_passed(
                &as_eol(
                    &format!("{marker} {content}\n\n{counterpart_done_when}\n\n{BAR}\n"),
                    eol,
                ),
                &format!("{family}: {content:?} written on the {marker:?} line itself, {eol:?}"),
            );
        }
        for marker in DONE_WHEN_MARKERS {
            expect_passed(
                &as_eol(
                    &format!("{counterpart_problem}\n\n{PROBLEM}\n\n{marker}\n{content}\n"),
                    eol,
                ),
                &format!("{family}: {content:?} on the line directly under {marker:?}, {eol:?}"),
            );
        }
        for marker in PROBLEM_MARKERS {
            expect_passed(
                &as_eol(
                    &format!("{marker}\n{content}\n\n{counterpart_done_when}\n\n{BAR}\n"),
                    eol,
                ),
                &format!("{family}: {content:?} on the line directly under {marker:?}, {eol:?}"),
            );
        }
    }
}

#[test]
fn a_deferral_on_the_marker_line_itself_fails_in_both_sections() {
    // MODULE-DOC PROPERTY 3 — "both sections are held to one standard" — applied
    // where the passing side was most widened and the failing side was left
    // almost empty.
    //
    // The inline colon form (`## Problem: <text>`) was pinned as PASSING in four
    // places and had NO must-fail mirror anywhere in the file. The inline
    // done-when form had exactly one must-fail token, `TBD`, out of
    // `PLACEHOLDERS`, `derived_deferrals()`, `derived_phrase_deferrals()`,
    // `UNICODE_BLANKS`, the pointers and both HTML-comment prompt shapes. The
    // same asymmetry held for the marker-with-no-blank-line shape:
    // `{done_when_marker}\nTBD` was pinned, `{problem_marker}\nTBD` was not.
    //
    // The inline form is a separate code path — the content lives on the marker
    // line rather than under it — and once it is written separately the cheapest
    // predicate satisfying every other fixture in the file is:
    //
    //     if let Some((_, rest)) = heading_line.split_once(':') {
    //         if !rest.trim().is_empty() {
    //             present = true;          // no deferral check at all
    //         }
    //     }
    //
    // That gate is green across the whole suite and then certifies the bet for
    // `## Problem: TBD`, `## Problem: N/A`, `## Problem: ...` and
    // `## Problem: <!-- what problem does this solve? -->`, and certifies the bar
    // for `## Done when: <!-- how will a reviewer check this is done? -->`,
    // `## Done when: \u{200b}`, `## Done when: see the linked issue` and
    // `## Done when: #4192`. The last of those is the pasted-template false green
    // the module docs call the whole ballgame, reached through the one marker
    // shape whose mirrors were never filled in.
    let tokens = single_line_must_fail_tokens();

    // The fixture invariants first, so they are exercised rather than stranded
    // behind the measurement.
    assert!(
        tokens.len() > 30,
        "fixture invariant: this family must run more than a handful of tokens, or \
         an inline predicate can satisfy it by screening the two or three it names; \
         got {}",
        tokens.len()
    );
    for family in [
        "an enumerated placeholder",
        "a single-line template prompt",
        "a derived deferral",
        "a derived phrase deferral",
        "a section blank only to a reader",
        "a pointer to somewhere else",
    ] {
        assert!(
            tokens.iter().any(|(_, f)| *f == family),
            "fixture invariant: every must-fail family must reach the inline marker \
             position, or the one it misses is the hole. Missing: {family:?}"
        );
    }
    for marker_table in [PROBLEM_MARKERS, DONE_WHEN_MARKERS] {
        assert!(
            marker_table.iter().any(|m| m.ends_with(':')),
            "fixture invariant: each marker table must carry a colon-terminated \
             spelling, or the inline half of this test runs over nothing at all"
        );
    }

    for (index, (token, family)) in tokens.iter().enumerate() {
        assert_token_fails_on_and_under_the_marker(index, token, family);
    }

    // AND THE PASSING MIRRORS, kept and widened, so this cannot swing the other
    // way into rejecting `## Done when: p99 < 5ms`. Terse content, content that
    // merely begins with a deferral stem, and content that cites a pointer are
    // all real bars written on a heading's own line, and every one of them must
    // still pass.
    for (index, content) in [
        INLINE_BAR,
        "p99 stays under 5ms for two consecutive canary windows",
        "no 5xx",
        "- Navigation completes in under 200ms",
        "the retry budget behaves the same as above for the plaintext listener",
        "the checkout panel at https://grafana.invalid/d/canary stays under 5ms",
    ]
    .iter()
    .enumerate()
    {
        assert_content_passes_on_and_under_the_marker(
            index,
            content,
            "real content on the marker line",
        );
    }
}

#[test]
fn an_unfilled_template_whose_prompts_are_multi_line_comments_fails_closed() {
    // The single commonest unfilled pull request body there is, written in the
    // form GitHub's own template documentation uses and the form most real
    // templates ship: the prompt sits on its own line between the delimiters.
    //
    // See MULTILINE_PROMPT_PROBLEM for what this closes. A line-local
    // `starts_with("<!--") && ends_with("-->")` predicate — the obvious thing
    // to write once `is_content` is per-line, which the `any(substantive)` rule
    // makes it — reads the inner prompt line as real content and certifies this
    // body whole, with every other test in this file green.
    //
    // The fixture invariants first, so they are exercised rather than stranded
    // behind the `todo!()` while the gate is unwritten.
    for (label, block) in [
        ("the problem prompt", MULTILINE_PROMPT_PROBLEM),
        ("the done-when prompt", MULTILINE_PROMPT_BAR),
    ] {
        assert!(
            block.lines().count() > 2,
            "fixture invariant: {label} must span more than two lines, or it stops \
             being the multi-line shape this test exists for and collapses back onto \
             the single-line form already pinned in PLACEHOLDERS"
        );
        assert!(
            block
                .lines()
                .any(|l| !l.trim().starts_with("<!--") && !l.trim().ends_with("-->")),
            "fixture invariant: {label} must carry a line that neither opens nor \
             closes a comment, or a line-local delimiter predicate still reads the \
             whole block as a comment and this test pins nothing new"
        );
    }

    // Over both line endings, because the closing delimiter is exactly the kind
    // of `ends_with` test a trailing `\r` defeats, and this body reaches the
    // guard layer from the GitHub web UI with CRLF.
    for eol in BOTH_EOLS {
        expect_missing(
            &as_eol(
                &body_with(MULTILINE_PROMPT_PROBLEM, MULTILINE_PROMPT_BAR),
                eol,
            ),
            &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
            &format!(
                "a template nobody filled in, its prompts written as multi-line HTML \
                 comments, {eol:?}"
            ),
        );
    }

    // The same body with no headings at all, which is what a template leading
    // with its prompts produces before the author types anything.
    for eol in BOTH_EOLS {
        expect_missing(
            &as_eol(
                &format!("{MULTILINE_PROMPT_PROBLEM}\n\n{MULTILINE_PROMPT_BAR}\n"),
                eol,
            ),
            &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
            &format!("multi-line template prompts and nothing else, {eol:?}"),
        );
    }

    // And each block judged in the bet position as well as the bar position, so
    // an implementation cannot screen one section for substance and settle for
    // "non-empty" on the other — module-doc property 3, applied to the token
    // this test exists for.
    for block in [MULTILINE_PROMPT_PROBLEM, MULTILINE_PROMPT_BAR] {
        assert_placeholder_fails_in_both_sections(block, "a multi-line template prompt");
    }
}

#[test]
fn a_deferral_fails_however_it_is_capitalised_punctuated_or_bulleted() {
    // The defect this kills, verified against a real implementation: a gate
    // whose substance check is `!PLACEHOLDERS.contains(section)` passes every
    // enumerated test above and then returns Passed for a done-when section
    // reading "TBD.", "tbd!", "N/A.", "Todo:", "WIP", "- [x]" or "xxx". A
    // hardcoded table is a complete implementation only while the must-fail set
    // is enumerable, so this one is generated.
    let derived = derived_deferrals();
    let novel = derived
        .iter()
        .filter(|d| !PLACEHOLDERS.contains(&d.as_str()))
        .count();
    assert!(
        novel > 200,
        "fixture invariant: the derived family must contain far more strings than \
         PLACEHOLDERS enumerates, or copying that table into the gate is still a \
         complete implementation; got {novel} novel of {} derived",
        derived.len()
    );

    for placeholder in &derived {
        assert_placeholder_fails_in_both_sections(placeholder, "derived deferral");
    }
}

#[test]
fn a_deferral_phrase_fails_even_though_it_shares_no_prefix_with_the_table() {
    // The prefix-disjointness invariant holds of the raw phrases: nothing in
    // PLACEHOLDERS reaches them by accident. It is asserted on the raw form
    // only, because the bullet wrappers below deliberately give the derived
    // forms the same two-character prefix as `"- "`.
    for phrase in PHRASE_DEFERRALS {
        for enumerated in PLACEHOLDERS {
            let n = enumerated.len().min(phrase.len()).min(3);
            assert_ne!(
                phrase.to_lowercase()[..n],
                enumerated.to_lowercase()[..n],
                "fixture invariant: {phrase:?} must share no prefix with the enumerated \
                 placeholder {enumerated:?}, or the table reaches it by accident"
            );
        }
    }

    // Multiplied out, exactly as `DEFERRAL_STEMS` is. Listing four lowercase,
    // unpunctuated, unbulleted literals made this family satisfiable by
    // `PHRASES.contains(&section.trim())` — four strings copied off the
    // constant — and that gate then returns Passed for `"See the linked
    // issue."`, `"Same as above!"` and `"- see the linked issues"`. The English
    // sentence a human types is not the literal a table holds.
    let derived = derived_phrase_deferrals();
    let novel = derived
        .iter()
        .filter(|d| !PHRASE_DEFERRALS.contains(&d.as_str()))
        .count();
    assert!(
        novel > 200,
        "fixture invariant: the derived phrase family must contain far more strings \
         than PHRASE_DEFERRALS enumerates, or copying that table into the gate is \
         still a complete implementation; got {novel} novel of {} derived",
        derived.len()
    );

    for phrase in &derived {
        assert_placeholder_fails_in_both_sections(phrase, "derived deferral phrase");
    }
}

#[test]
fn a_pointer_to_somewhere_else_is_not_the_artifact() {
    // The product decision the phrase deferrals were standing in for, stated on
    // the shape it actually takes. "The artifact lives on the change under
    // review" means a section whose entire content is a reference to another
    // place has not produced it: the reviewer, the auditor and the scorecard
    // all read this body, and none of them follows the link.
    //
    // Pinned separately from the phrases because a bare URL and a bare issue
    // reference share no English with any of them, and because they are the
    // commonest form of this defect by a wide margin — an author who defers
    // pastes a link far more often than they write a sentence about deferring.
    //
    // Listed in open_questions as a decision a human can veto: a shop that
    // accepts "the bar is in the linked issue" wants this family deleted, not
    // weakened.
    for pointer in POINTERS {
        assert_placeholder_fails_in_both_sections(pointer, "a pointer to somewhere else");
    }

    // THE MIRROR, and the reason this family had none until now. Every other
    // must-fail family in this file is paired with one: `UNICODE_BLANKS` with
    // real prose carrying an NBSP, a ZWSP and a BOM; `DEFERRAL_STEMS` with
    // `REAL_CONTENT_WITH_A_DEFERRAL_PREFIX`. The pointers got nothing, and no
    // passing fixture anywhere in the file contained `http`, a bare domain or a
    // `#1234` issue reference. So
    //
    //     !(line.contains("http") || ISSUE_RE.is_match(line)) && …
    //
    // — reject any line CONTAINING a pointer, rather than a line that IS one —
    // passed the whole suite, and then reported a missing acceptance bar for a
    // bar that cites the dashboard it will be checked on, and a missing problem
    // for a body that opens `Fixes #4192:`. Both are ordinary writing; a
    // done-when that names the panel you will read is a BETTER bar, not a
    // deferred one.
    //
    // The same hole existed for `PHRASE_DEFERRALS`, where `section.contains("same
    // as above")` satisfied the derived family and then killed a bar saying the
    // retry budget behaves the same as above for the plaintext listener.
    //
    // The rule the pair states — a section whose WHOLE content is a pointer has
    // not produced the artifact; a pointer beside real content does not erase
    // the content — is the only rule that satisfies both halves.
    for content in [
        "- the checkout p99 panel at https://grafana.invalid/d/canary shows under 5ms for two \
         windows",
        "Fixes #4192: the canary gate rebuilds its verdict from `passed`, which is true for a \
         canary nobody queried.",
        "- the retry budget behaves the same as above for the plaintext listener",
        "- the example.invalid/d/canary panel stays under 5ms for two consecutive windows",
    ] {
        assert_real_content_passes_in_both_sections(content, "real content citing a pointer");
    }
}

#[test]
fn a_section_that_is_blank_only_to_a_reader_fails() {
    // U+200B and U+FEFF are not `char::is_whitespace`, so a section holding one
    // survives `trim()` and reads as substance to the cheapest possible check
    // while rendering as an empty heading on GitHub.
    assert!(
        UNICODE_BLANKS
            .iter()
            .any(|b| !b.trim().is_empty() && b.chars().all(|c| !c.is_alphanumeric())),
        "fixture invariant: at least one of these must survive trim() while carrying \
         no readable content, or the family stops separating `trim().is_empty()` from \
         a real substance check"
    );

    for blank in UNICODE_BLANKS {
        assert_placeholder_fails_in_both_sections(blank, "invisible section");
    }

    // THE MIRROR, and without it this family is satisfied by rejecting any
    // section that contains one of these characters at all:
    //
    //     if section.chars().any(|c| matches!(c, '\u{200b}' | '\u{feff}' | …)) {
    //         return false;
    //     }
    //
    // That gate passes every assertion above and then reports both artifacts
    // missing from a complete, well-written one whose only sin is a
    // non-breaking space between two words — which is what every body pasted
    // out of Notion, Google Docs or Confluence carries, and what a leading BOM
    // adds to a body pasted out of a file. High incidence, and a fabricated
    // accusation is the same defect as a false green pointed the other way.
    //
    // The rule these two halves state together is: strip the invisible
    // characters, then judge what is left. Not: reject on sight.
    let nbsp_problem = "Checkout p99 regressed to 40ms\u{00a0}after the cache change, and the \
                        rollout path is now certified against a measurement that never happened.";
    let zwsp_bar = "- p99 under 5ms on the\u{200b} checkout path\n\
                    - the scorecard names the unqueried canary";
    expect_passed(
        &body_with(nbsp_problem, zwsp_bar),
        "a complete artifact carrying a non-breaking space and a zero-width space \
         inside real prose; an invisible character next to real content does not \
         erase the content",
    );

    let bom_problem = format!("\u{feff}{PROBLEM}");
    let bom_bar = format!("\u{feff}{BAR}");
    expect_passed(
        &body_with(&bom_problem, &bom_bar),
        "a complete artifact whose sections open with a byte order mark, which is what \
         a body pasted out of a file carries",
    );

    // And the ideographic and em spaces, which arrive from the same editors.
    expect_passed(
        &body_with(
            &PROBLEM.replacen(' ', "\u{2003}", 1),
            &BAR.replacen(' ', "\u{3000}", 1),
        ),
        "a complete artifact carrying an em space and an ideographic space inside real \
         prose",
    );
}

#[test]
fn a_section_mixing_a_deferral_with_real_content_is_judged_on_the_real_content() {
    // THE DECISION THIS TEST SETTLES. Until it existed, no fixture anywhere in
    // this file had a section whose lines disagreed: every passing section led
    // with real content on every line, and every failing section was a deferral
    // on every line. So `substantive(section)` could legally collapse to
    // `substantive(first non-blank line)` — and the mirror,
    // `section.lines().any(substantive)`, was equally legal. The two disagree
    // about a partially-filled checklist, which is one of the commonest real
    // shapes there is, and the suite decided nothing while reading as though it
    // had.
    //
    // The rule is `any`: a section is the artifact if anything in it is. An
    // author who left the first checkbox blank and then wrote three checkable
    // criteria under it has done Product's job, and failing them is a
    // fabricated accusation over a stray character. The `first line` rule is
    // rejected here explicitly, not left to the implementer.
    let checkbox_then_bar = format!("- [ ]\n{MULTILINE_BAR}");
    expect_passed(
        &body_with(PROBLEM, &checkbox_then_bar),
        "a partially-filled checklist: an empty checkbox above three real, checkable \
         criteria is a bar, and reading only the first line of the section calls it a \
         template paste",
    );

    // The same thing the other way up, so the rule is not "skip the first line"
    // either.
    expect_passed(
        &body_with(PROBLEM, &format!("{MULTILINE_BAR}\n- [ ]")),
        "three real criteria with an empty checkbox left at the bottom",
    );

    // The problem-side mirror, both ways up.
    expect_passed(
        &body_with(&format!("TBD\n\n{PROBLEM}"), BAR),
        "a problem section that opens with a leftover TBD and then states the problem",
    );
    expect_passed(
        &body_with(&format!("{PROBLEM}\n\nTBD"), BAR),
        "a problem section that states the problem and then trails a leftover TBD",
    );

    // A CHECKBOX THAT CARRIES CONTENT, which is the commonest spelling of an
    // acceptance bar there is and the one every pull request template produces.
    //
    // Until this family existed, the only checkbox lines anywhere in the file
    // were EMPTY ones that had to fail: `"[ ]"`, `"[x]"` and `"- [x]"` are
    // deferral stems, `"- [ ] "` is an enumerated placeholder, `"- [ ]"` is one
    // of the fillers in `a_third_section_does_not_hide_an_empty_one`, and
    // `all_deferral` below opens with three empty boxes. Nothing anywhere
    // required a checkbox line to be read as content, so
    //
    //     !DEFERRAL_STEMS.iter().any(|s| n == *s || n.starts_with(&format!("{s} ")))
    //
    // — treating `- [ ] ` as announcing a deferral the way `TODO: ` does — was
    // green across the whole file, and then reported a missing acceptance bar
    // for `- [ ] p99 < 5ms`. That is the fabricated accusation at very high
    // incidence, which this file names as equal in severity to a false green.
    //
    // The two families together state the rule the suite means: strip the
    // checkbox marker, then judge the remainder. Not: a checkbox announces a
    // deferral. Run through both sections, so the rule is not applied to the bar
    // and forgotten on the bet.
    for checklist in [
        "- [ ] p99 < 5ms\n- [ ] the scorecard names the canary it queried",
        "- [x] p99 < 5ms\n- [ ] the scorecard names the canary it queried",
        "- [X] p99 < 5ms\n- [ ] the scorecard names the canary it queried",
    ] {
        assert_real_content_passes_in_both_sections(checklist, "a checklist carrying criteria");
    }

    // A checklist whose first box is empty and whose remaining boxes carry
    // criteria: the partially-filled template, with the marker on every line.
    expect_passed(
        &body_with(
            PROBLEM,
            "- [ ]\n- [x] p99 < 5ms\n- [ ] the scorecard names the canary it queried",
        ),
        "an empty checkbox above two checkboxes that carry real criteria",
    );

    // AN UNFILLED TEMPLATE COMMENT ABOVE REAL CONTENT, which is what a filled-in
    // template actually looks like: authors overwhelmingly leave the prompt
    // comment where it is and type underneath it.
    //
    // `"<!-- what problem does this solve? -->"` is pinned in `PLACEHOLDERS` and
    // in the `exactly_both` body of `a_real_pull_request_body_with_no_bar_fails_closed`
    // as a WHOLE section, and nowhere as a line beside content. So a section-level
    // reject — `!s.contains("<!--") && s.lines().any(is_content)` — passed every
    // fixture in this file, including every mixed-content case above (which mix
    // checkboxes and `TBD` and never a comment), and then reported BOTH artifacts
    // missing from the commonest filled-in template body there is.
    //
    // The `any` rule has to cover an HTML comment exactly the way it covers a
    // leftover `TBD`: strip the line that is not content, judge what is left.
    expect_passed(
        &body_with(
            &format!("<!-- what problem does this solve? -->\n{PROBLEM}"),
            &format!("<!-- how will we know this is done? -->\n{MULTILINE_BAR}"),
        ),
        "a filled-in template whose prompt comments were left above the content the \
         author typed under them",
    );
    expect_passed(
        &body_with(
            &format!("{PROBLEM}\n<!-- what problem does this solve? -->"),
            &format!("{MULTILINE_BAR}\n<!-- how will we know this is done? -->"),
        ),
        "the same template comments left trailing below the content instead of above \
         it",
    );
    expect_passed(
        &body_with(
            &format!("<!-- what problem does this solve? -->\n\n{PROBLEM}"),
            &format!("<!-- how will we know this is done? -->\n\n{MULTILINE_BAR}"),
        ),
        "the same template comments with a blank line between the prompt and the \
         content",
    );

    // THE SAME PAIR FOR THE MULTI-LINE PROMPT BLOCK, which is the form GitHub's
    // own template documentation writes and the form most real templates ship.
    // `an_unfilled_template_whose_prompts_are_multi_line_comments_fails_closed`
    // pins the failing half; without this half the fix for it degenerates into
    // "reject any section containing `<!--`", or "reject any section holding a
    // line that opens a comment and does not close it" — and either of those
    // reports both artifacts missing from the commonest FILLED-IN template body
    // there is, which is the fabricated accusation this file names as equal in
    // severity to a false green. Above the author's text and below it, the same
    // two positions the single-line form is pinned in.
    expect_passed(
        &body_with(
            &format!("{MULTILINE_PROMPT_PROBLEM}\n\n{PROBLEM}"),
            &format!("{MULTILINE_PROMPT_BAR}\n\n{MULTILINE_BAR}"),
        ),
        "a filled-in template whose multi-line prompt blocks were left above the \
         content the author typed under them",
    );
    expect_passed(
        &body_with(
            &format!("{PROBLEM}\n\n{MULTILINE_PROMPT_PROBLEM}"),
            &format!("{MULTILINE_BAR}\n\n{MULTILINE_PROMPT_BAR}"),
        ),
        "the same multi-line prompt blocks left trailing below the author's own \
         content instead of above it",
    );

    // And the bound on `any`, so it cannot be satisfied by a stray word: a
    // section several lines long, every line of which is a deferral, is still
    // no artifact. This is the fully-unfilled checklist, which is a template
    // paste however tall it is.
    let all_deferral = "- [ ]\n- [ ]\n- [ ]\n\nTBD\n\nN/A\n\nWIP\n\n???";
    assert!(
        all_deferral
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
            > 4,
        "fixture invariant: this section has to be several lines long, or it stops \
         separating `any(substantive)` from `substantive(whole section)`"
    );
    assert_placeholder_fails_in_both_sections(all_deferral, "an unfilled checklist");
}

#[test]
fn a_short_colon_terminated_lead_in_line_is_writing_not_a_section_boundary() {
    // A colon-terminated line is ordinary technical writing, in both the shapes
    // it is written in: with its content on the next line, and with a blank
    // line between the two.
    //
    // The previous revision pinned only the first. That was pinning the half of
    // the rule that was convenient. `"Testing:"` was in the boundary family, so
    // a bare colon-terminated line followed by a blank line had to END a
    // section — and no fixture anywhere put such a line, followed by a blank
    // line, INSIDE one. The implementation the suite demanded was therefore
    //
    //     line.starts_with('#') || ((line.ends_with(':') || is_bold_only(line))
    //         && next_is_blank)
    //
    // which passes every test in the file and then reports a missing acceptance
    // bar for
    //
    //     ## Done when
    //
    //     Acceptance criteria:
    //
    //     - p99 < 5ms
    //     - the scorecard names the unqueried canary
    //
    // one of the two commonest shapes a done-when takes. High incidence,
    // unfalsified: the fabricated accusation this file names as equal in
    // severity to a false green.
    //
    // The two demands cannot both be met — `Acceptance criteria:` above bullets
    // and `Testing:` above prose are structurally identical, and only the
    // English tells them apart — so the suite decides instead of leaving it to
    // whichever side the implementer guesses. `"Testing:"` is out of
    // `BOUNDARY_HEADERS`; a colon-terminated line is never a boundary; both
    // spacings pass, in both sections, under both line endings. The veto is
    // stated in BOUNDARY_HEADERS' docs and in open_questions.
    let lead_in_bar = format!("Acceptance:\n{MULTILINE_BAR}");
    let lead_in_problem = format!("Today:\n{PROBLEM}");
    let spaced_lead_in_bar = format!("Acceptance criteria:\n\n{MULTILINE_BAR}");
    let spaced_lead_in_problem = format!("Today:\n\n{PROBLEM}");

    for eol in BOTH_EOLS {
        expect_passed(
            &as_eol(&body_with(PROBLEM, &lead_in_bar), eol),
            &format!("a bar introduced by the lead-in line \"Acceptance:\", {eol:?}"),
        );
        expect_passed(
            &as_eol(&body_with(&lead_in_problem, BAR), eol),
            &format!("a problem introduced by the lead-in line \"Today:\", {eol:?}"),
        );
        expect_passed(
            &as_eol(&body_with(&lead_in_problem, &lead_in_bar), eol),
            &format!("both sections introduced by a colon-terminated lead-in, {eol:?}"),
        );

        // THE MIRROR FIXTURES, and the reason this test was rejected: the same
        // lead-in with a BLANK LINE under it, which is the shape the boundary
        // rule above turns into a heading. Both sections, both line endings.
        expect_passed(
            &as_eol(&body_with(PROBLEM, &spaced_lead_in_bar), eol),
            &format!(
                "a bar introduced by \"Acceptance criteria:\" and a blank line above its \
                 bullets, {eol:?}"
            ),
        );
        expect_passed(
            &as_eol(&body_with(&spaced_lead_in_problem, BAR), eol),
            &format!(
                "a problem introduced by \"Today:\" and a blank line above its prose, \
                 {eol:?}"
            ),
        );
        expect_passed(
            &as_eol(
                &body_with(&spaced_lead_in_problem, &spaced_lead_in_bar),
                eol,
            ),
            &format!(
                "both sections introduced by a colon-terminated lead-in and a blank \
                 line, {eol:?}"
            ),
        );
    }

    // The mirror in the other direction, so this cannot be satisfied by reading
    // a colon-terminated line as content: a lead-in with nothing to lead in to
    // is still an empty section, whichever spacing follows it.
    //
    // IN BOTH POSITIONS. This half used to be pinned only in the done-when
    // section, which is module-doc property 3 — "both sections are held to one
    // standard" — broken by the file that states it: a gate that read a bare
    // lead-in as content in the PROBLEM section satisfied the whole suite.
    for (label, lead_in) in [
        ("with nothing to lead in to", "Acceptance:".to_string()),
        (
            "with a blank line and nothing else",
            "Acceptance criteria:\n\n".to_string(),
        ),
    ] {
        expect_missing(
            &body_with(PROBLEM, &lead_in),
            &[Artifact::DoneWhenBar],
            &format!("a done-when section holding a lead-in line {label}"),
        );
    }
    for (label, lead_in) in [
        ("with nothing to lead in to", "Background:".to_string()),
        (
            "with a blank line and nothing else",
            "Today:\n\n".to_string(),
        ),
    ] {
        expect_missing(
            &body_with(&lead_in, BAR),
            &[Artifact::WrittenProblem],
            &format!("a problem section holding a lead-in line {label}"),
        );
    }
}

#[test]
fn a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it() {
    // The other half of the boundary decision, pinned so the implementer does
    // not have to guess it either — and pinned in the direction that costs an
    // author something, which is why it is a decision rather than a detail.
    //
    // `**Done when**` and `**Problem**` are markers in this file: a bold-only
    // line OPENS a section. It follows that a bold-only line naming a different
    // topic CLOSES the one above it, and `BOUNDARY_HEADERS` keeps `"**Testing**"`
    // for exactly that reason — an empty `## Done when` above `**Testing**` must
    // not swallow the testing notes, which is this file's headline defect.
    //
    // The cost: a bold sub-label inside a done-when section starts a new
    // section, so the bar above it is empty. That is the consequence of the
    // rule, so it is pinned here rather than left for an author to discover.
    // The veto is in BOUNDARY_HEADERS' docs: drop `"**Testing**"` from that
    // list and flip these two fixtures to `expect_passed`, which re-opens the
    // bold-third-section fail-open in exchange.
    //
    // WHY THE SUB-LABELS ARE `**Testing**`, `**Rollout**` AND `**Notes**`, AND
    // WHY WHAT SITS UNDER THEM IS TESTING PROSE.
    //
    // The previous revision wrote this test with `**Acceptance criteria**` above
    // a real three-item bar, and `**Background**` above a real problem
    // statement. Both bodies genuinely state both artifacts — `MULTILINE_BAR` is
    // three checkable criteria and `PROBLEM` is a written problem — so the test
    // contradicted this file's own promise that "no test here requires a body
    // that genuinely states both artifacts to fail", and it quietly closed the
    // marker vocabulary the module docs leave open: `acceptance criteria` is the
    // first synonym those docs name as free to recognise, so an implementer who
    // took the promise at face value, wrote a more forgiving and entirely
    // correct gate, and was told by a settled specification test that it was
    // broken. Editing the spec mid-implementation is the one thing this
    // project's method forbids, so the collision is removed rather than
    // documented.
    //
    // The boundary decision is unchanged and still pinned in the direction that
    // costs an author something: the labels below name a DIFFERENT topic (no
    // reading of "Testing", "Rollout" or "Notes" is a synonym for the bet or the
    // bar), and what sits under them is testing prose, not an acceptance bar. If
    // a bold-only line does not end the section above it, the done-when section
    // is that label plus real prose and this body passes; if it does, the bar is
    // empty and it fails. The discrimination is intact and the vocabulary
    // freedom survives.
    //
    // `**Rollout**` and `**Notes**` are outside `BOUNDARY_HEADERS` on purpose:
    // the rule is that ANY bold-only line is a heading, not that the four
    // enumerated ones are.
    assert_the_boundary_families_state_one_consistent_rule();

    for eol in BOTH_EOLS {
        for label in ["**Testing**", "**Rollout**"] {
            expect_missing(
                &as_eol(
                    &format!(
                        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n{label}\n\n{THIRD_SECTION_BODY}\n"
                    ),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!(
                    "the bold-only sub-label {label:?} under an otherwise empty done-when \
                     heading, {eol:?}"
                ),
            );
        }
        for label in ["**Testing**", "**Notes**"] {
            expect_missing(
                &as_eol(
                    &format!(
                        "## Problem\n\n{label}\n\n{THIRD_SECTION_BODY}\n\n## Done when\n\n{BAR}\n"
                    ),
                    eol,
                ),
                &[Artifact::WrittenProblem],
                &format!(
                    "the bold-only sub-label {label:?} under an otherwise empty problem \
                     heading, {eol:?}"
                ),
            );
        }
    }
}

/// The two boundary families have to state one rule between them.
///
/// Not a `#[test]` of its own: nothing here touches the gate, so standing alone
/// it would be green from the moment it was written, and a test that has never
/// been observed failing publishes assurance it has not earned. It runs first
/// inside `a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it`,
/// which is red on the absent measurement like everything else here — the same
/// arrangement as `assert_the_wiring_parsers_read_a_real_wiring`.
#[track_caller]
fn assert_the_boundary_families_state_one_consistent_rule() {
    // The two boundary families are the one place in this file where a passing
    // fixture and a failing fixture are told apart by a rule rather than by
    // their content, so the relationship between them is asserted rather than
    // left implicit. A later edit that puts `"Testing:"` back into
    // `BOUNDARY_HEADERS` without also flipping the colon lead-in fixtures
    // re-creates the contradiction that got the previous revision rejected —
    // two demands no implementation can satisfy at once, which an implementer
    // discovers as an unwinnable test run rather than as a decision.
    for header in BOUNDARY_HEADERS {
        assert!(
            THIRD_SECTION_HEADERS.contains(header),
            "{header:?} must terminate a section AND be one of the third-section \
             headings the passing family runs, or the two families are testing \
             different things"
        );
    }
    assert!(
        !BOUNDARY_HEADERS.contains(&"Testing:"),
        "a colon-terminated line is pinned as ordinary writing by \
         a_short_colon_terminated_lead_in_line_is_writing_not_a_section_boundary, so \
         requiring one to terminate a section here demands two incompatible things \
         of the same rule. Flip that test's blank-line fixtures to expect_missing \
         before putting this entry back"
    );
    assert!(
        BOUNDARY_HEADERS.contains(&"**Testing**"),
        "a bold-only line is pinned as a heading by \
         a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it, and by \
         **Done when** and **Problem** being markers. Dropping it here without \
         flipping that test re-opens the bold-third-section fail-open"
    );
    assert!(
        !BOUNDARY_HEADERS.contains(&"### Testing"),
        "a heading DEEPER than the marker that opened the section is pinned as nested \
         content by a_heading_deeper_than_the_marker_is_content_inside_the_section, so \
         requiring one to terminate a section here demands two incompatible things of \
         the same rule: `### Criteria` above a real bar and `### Testing` above testing \
         notes are structurally identical and only the English tells them apart. Flip \
         that test's expect_passed fixtures to expect_missing before putting this \
         entry back"
    );
    for sibling in ["# Testing", "## Testing"] {
        assert!(
            BOUNDARY_HEADERS.contains(&sibling),
            "{sibling:?} sits at the marker's own depth or shallower, which makes it a \
             SIBLING section rather than nested content, and an empty `## Done when` \
             above one must not swallow it — that is this file's headline defect. \
             Dropping it here without flipping \
             a_third_section_does_not_hide_an_empty_one guts the boundary rule to \
             nothing while leaving both families green"
        );
    }
}

#[test]
fn the_marker_is_a_heading_line_not_a_phrase_anywhere_in_the_prose() {
    // Until this test existed, no passing fixture in the file contained the
    // words "problem" or "done when" anywhere except on a marker line. The two
    // occurrences inside content were both on the failing side, where the
    // verdict is already forced by an empty counterpart section. So the suite
    // could not tell a marker predicate anchored to a whole heading line apart
    // from one built on `contains` — and `contains` is the cheapest way to
    // satisfy the seven-by-six cross-product the marker test demands:
    //
    //     fn is_done_when_marker(l: &str) -> bool {
    //         normalise_heading(l).contains("done when")
    //     }
    //
    // Both directions of that mistake are pinned below, because a `contains`
    // predicate is wrong twice over depending on which match it takes.
    //
    // The two last-match fixtures are built and checked here, above the loop and
    // above the first measurement, so their invariant is exercised rather than
    // stranded behind a `todo!()`.
    //
    // NOTE FOR THE NEXT EDITOR — the one place in this file where the residue
    // mechanism rests on a coincidence rather than on an asserted invariant.
    // `assert_the_content_fixtures_carry_none_of_the_message_vocabulary` and
    // `assert_real_content_passes_in_both_sections` both forbid a fixture from
    // carrying the words the failure message is judged on, because
    // `message_residue` subtracts the body's lines before the negative naming
    // rule is applied. The two must-fail bodies below deliberately BREAK that:
    // their whole point is prose containing "done when" and "problem". They are
    // exempt only because `assert_failed_naming` skips the negative rule for an
    // artifact that IS in `want`, and in both bodies the artifact whose words
    // appear in the prose is the missing one. Swap which section is empty, or
    // add a third fixture here where the prose names the SURVIVING artifact, and
    // the residue rule quietly stops biting for that fixture.
    //
    // The marker-bearing prose sentence is deliberately NOT the last line of
    // either body, and that detail is the whole test. In the revision before this
    // one it was, in both fixtures — so the spurious section a `contains`
    // predicate opens after it was empty either way, and the verdict came out
    // identical whether the line was read as a marker or as prose. Review
    // reproduced it: the any-over-sections form of
    // `normalise(l).contains("done when")` passed this test and every other
    // behavioural test in the file while certifying an empty `## Done when` whose
    // words were supplied by a later `## Rollout` paragraph. A further line of
    // ordinary content under each prose sentence is what makes the spurious
    // section non-empty, so a `contains` predicate now reports the artifact
    // present here and this test goes red on it.
    let empty_bar_under_rollout_prose = format!(
        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n\n\
         ## Rollout\n\nThe rollout is done when the canary reports two clean \
         windows in a row.\n\
         We will keep the old path behind a flag until then.\n"
    );
    let empty_problem_under_notes_prose = format!(
        "## Problem\n\n\n## Done when\n\n{BAR}\n\n\
         ## Notes\n\nThe problem was introduced by the cache change last \
         quarter, and this only reports it.\n\
         The queue has carried the same defect since the rollout path was \
         split in two.\n"
    );
    assert_the_marker_prose_is_followed_by_content(
        &empty_bar_under_rollout_prose,
        "is done when the canary",
    );
    assert_the_marker_prose_is_followed_by_content(
        &empty_problem_under_notes_prose,
        "The problem was introduced",
    );

    for eol in BOTH_EOLS {
        // FIRST MATCH WINS: an ordinary summary paragraph mentioning the
        // problem, above the real sections. The prose line becomes the marker,
        // its section runs to the `## Problem` heading and is therefore empty,
        // and the gate reports a missing written problem on a change that wrote
        // one — an entirely ordinary body, rejected.
        expect_passed(
            &as_eol(
                &format!(
                    "## Summary\n\nThis addresses a problem in the canary path.\n\n\
                     ## Problem\n\n{PROBLEM}\n\n## Done when\n\n{BAR}\n"
                ),
                eol,
            ),
            &format!(
                "a summary paragraph mentioning the problem above a real problem and a \
                 real bar, {eol:?}"
            ),
        );
        expect_passed(
            &as_eol(
                &format!(
                    "This addresses a problem in the canary path.\n\n\
                     ## Problem\n\n{PROBLEM}\n\n## Done when\n\n{BAR}\n"
                ),
                eol,
            ),
            &format!(
                "an unheaded opening paragraph mentioning the problem above a real \
                 problem and a real bar, {eol:?}"
            ),
        );

        // LAST MATCH WINS: the fail-open twin. A later section's prose contains
        // the words "done when", and the `## Done when` section above it is
        // empty. The prose sentence is a statement about the rollout, not this
        // change's acceptance bar, and reading it as one certifies a template
        // paste. The two bodies and their invariant are above the loop.
        expect_missing(
            &as_eol(&empty_bar_under_rollout_prose, eol),
            &[Artifact::DoneWhenBar],
            &format!(
                "an empty done-when section above a later paragraph whose prose \
                 contains the marker words, {eol:?}"
            ),
        );
        expect_missing(
            &as_eol(&empty_problem_under_notes_prose, eol),
            &[Artifact::WrittenProblem],
            &format!(
                "an empty problem section above a later paragraph whose prose contains \
                 the marker word, {eol:?}"
            ),
        );
    }
}

/// The fixture invariant that makes the last-match-wins pair load-bearing.
///
/// A `contains` marker predicate opens a spurious section at the prose line that
/// carries the marker words. If that line is the last line of the body, the
/// spurious section is empty and the wrong gate reaches the right verdict by
/// accident — which is exactly how the previous revision of this test failed to
/// falsify the defect its own comment named. So: the marker-bearing sentence
/// must exist in the body, and at least one line of ordinary content must follow
/// it before the body ends.
#[track_caller]
fn assert_the_marker_prose_is_followed_by_content(body: &str, marker_prose: &str) {
    let lines: Vec<&str> = body.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.contains(marker_prose))
        .unwrap_or_else(|| {
            panic!(
                "fixture invariant: {marker_prose:?} must appear in the body, or this \
                 fixture pins nothing about a phrase-matching marker. body={body:?}"
            )
        });
    let followed_by_content = lines[at + 1..]
        .iter()
        .any(|l| !l.trim().is_empty() && !l.trim().starts_with('#'));
    assert!(
        followed_by_content,
        "fixture invariant: at least one line of ordinary content must follow the \
         marker-bearing prose {marker_prose:?}, or the spurious section a `contains` \
         predicate opens there is empty and the wrong gate reaches the right verdict \
         by accident. body={body:?}"
    );
}

/// Runs one section of real content through both positions and both-at-once.
///
/// The mirror of `assert_placeholder_fails_in_both_sections`, and used for the
/// same reason: a rule applied to the bar and not to the bet is half a gate.
#[track_caller]
fn assert_real_content_passes_in_both_sections(content: &str, family: &str) {
    // The message-vocabulary invariant, applied where the content actually
    // enters rather than to a hand-kept list beside it.
    // `assert_the_content_fixtures_carry_none_of_the_message_vocabulary` names
    // fifteen constants and misses every fixture written inline at a call site
    // — the checklists, the pointer mirrors,
    // `REAL_CONTENT_WITH_A_DEFERRAL_PREFIX`. All of those pass through here, so
    // asserting it here cannot silently exempt one: a fixture that itself said
    // "problem" or "acceptance" would be subtracted from the failure message by
    // `message_residue` and would exempt the gate from the naming rule it is
    // held to.
    assert!(
        !names_the_problem(content) && !names_the_bar(content),
        "fixture invariant: {family} must carry none of the vocabulary the failure \
         message is judged on, or subtracting it from the message exempts the gate \
         from the rule that message is held to. Content: {content:?}"
    );

    expect_passed(
        &body_with(PROBLEM, content),
        &format!("{family}: the done-when section holds {content:?}"),
    );
    expect_passed(
        &body_with(content, BAR),
        &format!("{family}: the problem section holds {content:?}"),
    );
    expect_passed(
        &body_with(content, content),
        &format!("{family}: both sections hold {content:?}"),
    );
}

#[test]
fn real_content_that_merely_begins_with_a_deferral_stem_passes() {
    // The bound on the deferral vocabulary, and the reason `PLACEHOLDERS` can
    // demand that `"TBD - will fill this in before merge"` and `"TODO: write
    // the acceptance criteria here"` fail without that demand costing an author
    // their acceptance bar.
    //
    // Neither of those two normalises to anything in a stem table, and
    // `derived_deferrals()` forbids an enumeration, so the cheapest
    // implementation satisfying both is a prefix test on the normalised line:
    //
    //     STEMS.iter().any(|s| normalised.starts_with(s))
    //
    // with `na`, `tbd`, `todo`, `wip`, `xxx` in the table. That passes every
    // other fixture in this file — `"Today:"` misses `"todo"` by one character
    // — and then reports a missing bar for `- Navigation completes in under
    // 200ms`, a missing problem for a section opening `Native TLS …`, and kills
    // anything starting `Wipe` or `NAT`. Nothing pinned a legitimate section
    // whose first word merely begins with a stem.
    //
    // The last fixture is the same defect at token level rather than at
    // character level: `TODO comments are removed from src/pre_merge_guard/`
    // opens with a deferral token used as an ordinary word. What separates it
    // from the two placeholders above — which must still fail — is the
    // separator after the token, not the token itself.
    // The fixture invariant that makes this family load-bearing, asserted
    // first so it is exercised rather than stranded behind the measurement:
    // the normalised form of every one of these really does start with a
    // deferral stem, so the prefix rule really is falsified here.
    for content in REAL_CONTENT_WITH_A_DEFERRAL_PREFIX {
        let normalised = content.trim_start_matches(['-', '*', ' ']).to_lowercase();
        assert!(
            DEFERRAL_STEMS
                .iter()
                .any(|s| normalised.starts_with(&s.to_lowercase())),
            "fixture invariant: {content:?} must normalise to something that STARTS \
             WITH a deferral stem, or it stops separating a prefix rule from a token \
             rule and this whole family is decoration. Normalised: {normalised:?}"
        );
    }

    for content in REAL_CONTENT_WITH_A_DEFERRAL_PREFIX {
        assert_real_content_passes_in_both_sections(content, "real content, deferral prefix");
    }

    // And the mirror, so widening the vocabulary this way cannot fail open: the
    // two placeholders whose separator announces a deferral still fail, in both
    // sections, however much the other one says.
    for placeholder in [
        "TODO: write the acceptance criteria here",
        "TBD - will fill this in before merge",
    ] {
        assert_placeholder_fails_in_both_sections(
            placeholder,
            "a deferral announced by its separator",
        );
    }
}

#[test]
fn content_passes_however_few_characters_and_words_it_takes() {
    // The bound on "substantive" from below, and the mirror of
    // `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact`.
    //
    // `SHORT_BAR` used to be the shortest thing this file required to pass:
    // "- p99 < 5ms", nine characters and three words once normalised. Nothing
    // between one and eight characters, and nothing of one or two words, was
    // required to pass anywhere. The module docs called that closed because a
    // minimum length admitting `SHORT_BAR` also admits the thirty-nine-byte
    // placeholder — true of a length rule used INSTEAD OF a content check, and
    // false of a length floor bolted ON TOP OF one. Review verified it:
    // appending `core.chars().count() >= 9` to an otherwise-correct content
    // predicate passed every behavioural test in this file, and so did "the line
    // must contain a space" and "the line must have at least three tokens".
    //
    // A floor is not a hypothetical mistake. It is the natural thing to reach
    // for once the placeholder family is in front of you, and it rejects the
    // terse bars that the best-run teams write — the ones with a number in them.
    //
    // The invariants first, so the family cannot quietly stop being short.
    let longest_failing = PLACEHOLDERS
        .iter()
        .chain(PHRASE_DEFERRALS.iter())
        .copied()
        .max_by_key(|p| p.chars().count())
        .expect("fixture invariant: the must-fail vocabulary must not be empty");
    for content in SHORT_REAL_CONTENT {
        assert!(
            content.chars().count() < longest_failing.chars().count()
                && content.split_whitespace().count() < longest_failing.split_whitespace().count(),
            "fixture invariant: {content:?} ({} chars, {} words) must be shorter than \
             the longest string this file requires to FAIL, {longest_failing:?} ({} \
             chars, {} words), in characters AND in words — otherwise a length floor \
             bolted on top of a content check still separates the two sets and \
             \"the measurement is the content, not the length\" is asserted in prose \
             only",
            content.chars().count(),
            content.split_whitespace().count(),
            longest_failing.chars().count(),
            longest_failing.split_whitespace().count(),
        );
    }

    let shortest_passing = SHORT_REAL_CONTENT
        .iter()
        .map(|c| c.chars().count())
        .min()
        .unwrap_or(0);
    let shortest_failing = PLACEHOLDERS
        .iter()
        .map(|p| p.trim().chars().count())
        .min()
        .unwrap_or(0);
    assert!(
        shortest_failing < shortest_passing,
        "fixture invariant: something that must FAIL has to be shorter than everything \
         that must pass ({shortest_failing} vs {shortest_passing} chars), or the two \
         sets are separable by length from below as well and this family only pins the \
         ceiling"
    );

    // One of these has no space in it at all; one is five characters of Hangul
    // in thirteen bytes. A floor in characters, in bytes, in words or in "does
    // it contain a space" rejects at least one of them.
    for content in SHORT_REAL_CONTENT {
        assert_real_content_passes_in_both_sections(content, "content written tersely");
    }

    // And the mirror, in the same length band, so widening the passing side
    // downwards cannot fail open: the deferrals that are this short still fail.
    for placeholder in ["TBD", "n/a", "...", "- [ ] "] {
        assert_placeholder_fails_in_both_sections(
            placeholder,
            "a deferral as short as the content beside it",
        );
    }
}

// THE TITLE IS NOT AN INPUT, AND NO TEST HERE CLAIMS TO PIN THAT.
//
// A previous revision carried `the_bet_and_the_bar_are_written_on_the_change_not
// _left_to_its_title`, whose name stated a fact its body could not falsify:
// `judge` takes the body alone, so there is no parameter through which a title
// could reach the gate and the compiler already enforces it. Its two assertions
// were byte-identical to
// `a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured` and to a case
// inside `an_acceptance_bar_with_no_written_problem_fails_however_long_the_bar`,
// so it could not fail for its own reason and published assurance it had not
// earned — which is this file's own stated standard, applied to this file. The
// claim lives in the module docs, where it is a decision and not a measurement.

#[test]
fn three_shapes_of_absence_produce_three_distinct_messages() {
    // What this test checks, exactly: that the three shapes of absence do not
    // share one message. One constant string — "the written problem and the
    // done-when acceptance bar are missing" — satisfies every positive
    // containment assertion in this file, and would tell an author that a gate
    // failed and nothing else.
    //
    // What it deliberately does NOT check, despite an earlier name that claimed
    // it: that each message names *only* the artifact that was missing. There
    // is no non-brittle way to assert that at the level of prose. A correct and
    // helpful implementation may legitimately write "your problem statement is
    // here, your done-when bar is not", or quote the offending section back at
    // the author — and a raw vocabulary ban turns both of those red. The
    // do-not-falsely-accuse property is therefore enforced one level down,
    // where it *is* mechanically checkable: on the measurement, by
    // `expect_missing` pinning the set exactly, and on the message minus the
    // body, by `assert_failed_naming`'s residue rule. See open_questions.
    //
    // Distinctness is asserted on the RESIDUE, not on the raw message. Asserted
    // raw, this test claimed something it did not check: the three bodies are
    // three different strings, so a gate rendering one constant message with
    // the body quoted after it produces three different messages, and the echo
    // does all the distinguishing. The measurement has to be what differs, so
    // the body is subtracted first.
    assert_the_content_fixtures_carry_none_of_the_message_vocabulary();

    let bodies = ["".to_string(), problem_only(PROBLEM), bar_only(BAR)];
    let both = expect_missing(
        &bodies[0],
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        "nothing written at all",
    );
    let no_bar = expect_missing(
        &bodies[1],
        &[Artifact::DoneWhenBar],
        "problem written, bar missing",
    );
    let no_problem = expect_missing(
        &bodies[2],
        &[Artifact::WrittenProblem],
        "bar written, problem missing",
    );

    let both_said = message_residue(&both, &bodies[0]);
    let no_bar_said = message_residue(&no_bar, &bodies[1]);
    let no_problem_said = message_residue(&no_problem, &bodies[2]);

    assert_ne!(
        both_said, no_bar_said,
        "three different absences cannot share one message; an author has to be \
         able to tell from it which artifact to go and write. These two differ only \
         by the body quoted back at them: {both:?} vs {no_bar:?}"
    );
    assert_ne!(
        both_said, no_problem_said,
        "same, for the missing problem statement: {both:?} vs {no_problem:?}"
    );
    assert_ne!(
        no_bar_said, no_problem_said,
        "the missing-bar message and the missing-problem message must differ in what \
         the GATE says, not merely in the section it quoted: {no_bar:?} vs \
         {no_problem:?}"
    );
}

#[test]
fn the_message_holds_its_ground_however_the_surviving_section_is_headed() {
    // THE HOLE THE RESIDUE RULE DID NOT ACTUALLY CLOSE.
    //
    // `assert_failed_naming` subtracts the body's own non-blank lines from the
    // message before asking whether the gate named an artifact the author did
    // write. That is what makes quoting the offending section legal. But every
    // `expect_missing` call in this file that had one artifact PRESENT wrote
    // that present section under the byte-identical heading `## Problem` or
    // `## Done when` — the marker cross-product varies the spelling only on the
    // MISSING side. So a constant that spells the two artifacts using those two
    // literals was subtracted out of the residue for exactly the bodies where
    // the negative rule applies:
    //
    //     GateStatus::Failed(
    //         "This change does not carry the Product artifact. Add a `## Problem` \
    //          section stating the bet and a `## Done when` section stating the \
    //          acceptance bar.".to_string())
    //
    // returned whenever `missing_artifacts` is non-empty. The positive holds
    // ("## Problem" lowercases to contain `problem`; "## Done when" contains
    // `done when`). The negative holds because the present section's heading
    // line is in the body and is therefore subtracted. And
    // `three_shapes_of_absence_produce_three_distinct_messages` passes, because
    // the three residues differ purely by WHICH heading got subtracted, not by
    // anything the gate measured. The author whose problem statement is present
    // is still told to go and write one — the exact defect the residue mechanism
    // was introduced to prevent.
    //
    // Varying the surviving section's spelling closes it. A constant cannot
    // embed thirteen heading spellings, so subtracting the body can no longer
    // subtract the message, and the residue really does hold the gate to what it
    // said on its own account.
    assert_the_content_fixtures_carry_none_of_the_message_vocabulary();

    for problem_marker in PROBLEM_MARKERS {
        expect_missing(
            &format!("{problem_marker}\n\n{PROBLEM}\n\n## Done when\n\nTBD\n"),
            &[Artifact::DoneWhenBar],
            &format!(
                "a written problem headed {problem_marker:?} above a deferred done-when: \
                 the message must name the missing bar without accusing the author over \
                 the problem statement they wrote"
            ),
        );
    }

    for done_when_marker in DONE_WHEN_MARKERS {
        expect_missing(
            &format!("## Problem\n\nTBD\n\n{done_when_marker}\n\n{BAR}\n"),
            &[Artifact::WrittenProblem],
            &format!(
                "a real bar headed {done_when_marker:?} below a deferred problem section: \
                 the message must name the missing problem without accusing the author \
                 over the bar they wrote"
            ),
        );
    }

    // And the same for the two heading depths crossed, so the surviving heading
    // is never the one the missing heading is spelled with either — a constant
    // that embedded `### Problem` instead would otherwise be subtracted by the
    // loops above.
    for problem_marker in PROBLEM_MARKERS {
        for done_when_marker in DONE_WHEN_MARKERS {
            expect_missing(
                &format!("{problem_marker}\n\n{PROBLEM}\n\n{done_when_marker}\n\nTBD\n"),
                &[Artifact::DoneWhenBar],
                &format!(
                    "a written problem headed {problem_marker:?} above a done-when headed \
                     {done_when_marker:?} that defers"
                ),
            );
            expect_missing(
                &format!("{problem_marker}\n\nTBD\n\n{done_when_marker}\n\n{BAR}\n"),
                &[Artifact::WrittenProblem],
                &format!(
                    "a real bar headed {done_when_marker:?} below a problem headed \
                     {problem_marker:?} that defers"
                ),
            );
        }
    }
}

#[test]
fn a_korean_problem_with_no_bar_fails_naming_the_missing_bar() {
    expect_missing(
        &problem_only(KO_PROBLEM),
        &[Artifact::DoneWhenBar],
        "a Korean problem statement with no bar",
    );
}

/// Bodies shaped to break a hurried section extractor. Every one of them is
/// missing at least one artifact, so every one of them must be `Failed`.
fn awkward_bodies() -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = vec![
        ("an empty body", String::new()),
        ("one Korean word, no headings", "카나리".to_string()),
        (
            "a Korean problem with no bar",
            "## Problem\n\n카나리 게이트가 잘못된 판정을 만든다\n".to_string(),
        ),
        (
            "a one-syllable problem and a TBD bar",
            "## Problem\n\n한\n\n## Done when\n\nTBD\n".to_string(),
        ),
        (
            "an emoji sentence, no headings",
            "🚀 배포가 못 된다".to_string(),
        ),
        (
            "mixed scripts above an empty done-when",
            "## Problem\n\n한국어 problem 混合 テキスト\n\n## Done when\n\n\n".to_string(),
        ),
        ("bare carriage returns", "\r\r\r".to_string()),
        // Every byte index from 0 to 47 in this body is either inside
        // "## Problem\n\n" or inside a three-byte Hangul syllable, so any
        // fixed-offset slice of it that is not a character boundary panics.
        (
            "a problem of twelve three-byte syllables and no bar",
            format!("## Problem\n\n{}", "가".repeat(12)),
        ),
        // The reversed order. `&body[find("## Problem") + len .. find("## Done when")]`
        // panics here with "byte range starts at 39 but ends at 0", because the
        // done-when marker is now before the problem marker.
        (
            "the done-when first, with an empty problem section",
            "## Done when\n\n- p99 < 5ms\n\n## Problem\n".to_string(),
        ),
        (
            "the done-when first, with a whitespace-only problem section",
            "## Done when\n\n- p99 < 5ms\n\n## Problem\n\n   \n".to_string(),
        ),
        // A body whose last characters are a marker with no trailing newline.
        // GitHub bodies routinely have none, and
        // `&body[find(marker) + marker.len() + 2 ..]` panics with "start byte
        // index 51 is out of bounds for string of length 49".
        (
            "a problem statement and a trailing done-when marker, no newline",
            "## Problem\n\nCheckout p99 regressed.\n\n## Done when".to_string(),
        ),
        (
            "a bar and a trailing problem marker, no newline",
            "## Done when\n\n- p99 < 5ms\n\n## Problem".to_string(),
        ),
        (
            "nothing but the problem marker, no newline",
            "## Problem".to_string(),
        ),
        (
            "nothing but the done-when marker, no newline",
            "## Done when".to_string(),
        ),
        // The same marker twice: the first occurrence empty, the second filled.
        // The other artifact is absent outright, so the verdict is unambiguous
        // whichever occurrence the gate reads.
        (
            "the problem marker twice, first empty, and no bar anywhere",
            "## Problem\n\n\n## Problem\n\nCheckout p99 regressed to 40ms.\n".to_string(),
        ),
        (
            "the done-when marker twice, first empty, and no problem anywhere",
            "## Done when\n\n\n## Done when\n\n- p99 < 5ms\n".to_string(),
        ),
    ];

    // The same fixtures as a browser submits them. A gate anchored on "\n##"
    // mis-slices every one of these.
    let crlf: Vec<(&'static str, String)> = out
        .iter()
        .map(|(name, body)| (*name, body.replace('\n', "\r\n")))
        .collect();
    out.extend(crlf);
    out
}

#[test]
fn judge_returns_a_verdict_for_any_body_and_never_panics() {
    // A panic inside `judge` is not a Failed gate: it unwinds
    // `evaluate_pre_merge_gates` and takes the whole review with it. The
    // obvious way to write one is to quote an excerpt of the body back at the
    // author — `&pr_body[..40]` — which is a byte index, and byte 40 lands
    // inside a character in several of the bodies below. The other two shapes
    // are a marker order the extractor did not expect and a marker with nothing
    // after it.
    //
    // The assertion is `Failed`, not merely "returned": none of these bodies
    // carries both artifacts, so answering `NotMeasured` for any of them
    // certifies a change that produced no acceptance bar.
    for (context, body) in awkward_bodies() {
        assert!(
            !missing(&body).is_empty(),
            "{context}: this fixture is on the failing side, so the measurement must \
             report at least one missing artifact; body={body:?}"
        );
        expect_failed(&product_bar::judge(&body), context);
    }
}

#[test]
fn the_verdict_is_the_same_whether_the_body_uses_lf_or_crlf() {
    // GitHub's web UI submits textarea content with CRLF, and nothing between
    // the webhook payload and the guard layer normalises it. A gate anchored on
    // `"## Done when\n"` fails essentially every human-authored pull request
    // while a suite built only from `\n` stays green.
    /// A fixture built over whichever line ending it is handed.
    type BodyBuilder = Box<dyn Fn(Eol) -> String>;

    let cases: Vec<(&str, BodyBuilder)> = vec![
        (
            "a written problem and a done-when bar",
            Box::new(|eol| body_with_eol(PROBLEM, BAR, eol)),
        ),
        (
            "a problem statement with no bar",
            Box::new(|eol| problem_only_eol(PROBLEM, eol)),
        ),
        (
            "a done-when heading with nothing under it",
            Box::new(|eol| body_with_eol(PROBLEM, "", eol)),
        ),
        // The three cases above are all `##`-headed with no third section,
        // which is the one shape where CRLF is harmless: `str::lines()` strips
        // the `\r` and `.trim()` eats it, so even a hand-rolled `#` branch
        // survives. The two below are the shapes that actually separate a gate
        // that handles CRLF from one that does not.
        (
            "an empty done-when above a bold third section",
            Box::new(|eol| {
                as_eol(
                    &format!(
                        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n\n**Testing**\n\n{THIRD_SECTION_BODY}\n"
                    ),
                    eol,
                )
            }),
        ),
        (
            "a complete artifact under bold labels rather than hashes",
            Box::new(|eol| {
                as_eol(
                    &format!("**Problem**\n\n{PROBLEM}\n\n**Done when**\n\n{BAR}\n"),
                    eol,
                )
            }),
        ),
    ];

    for (name, build) in &cases {
        let lf = product_bar::judge(&build(Eol::Lf));
        let crlf = product_bar::judge(&build(Eol::Crlf));
        assert_eq!(
            variant(&lf),
            variant(&crlf),
            "{name}: the same change reached a different verdict because its line \
             endings came from a browser rather than an editor. lf={lf:?} crlf={crlf:?}"
        );
        assert_eq!(
            missing(&build(Eol::Lf)),
            missing(&build(Eol::Crlf)),
            "{name}: the gate found different artifacts missing under CRLF than under LF"
        );
    }

    // "Failed under both line endings" would satisfy the loop above, so the
    // absolute verdicts are pinned too — in both directions, on the two shapes
    // that separate a CRLF-aware gate from a CRLF-blind one.
    expect_passed(
        &body_with_eol(PROBLEM, BAR, Eol::Crlf),
        "a complete Product artifact typed into the GitHub web UI; rejecting it \
         blocks the majority of real pull requests",
    );

    // A complete artifact under bold labels, submitted from a browser. A gate
    // that recognises `**Done when**` with `ends_with("**")` over
    // `body.split('\n')` sees `**Done when**\r`, recognises nothing, and
    // reports both artifacts missing from a body that carries both.
    expect_passed(
        &as_eol(
            &format!("**Problem**\n\n{PROBLEM}\n\n**Done when**\n\n{BAR}\n"),
            Eol::Crlf,
        ),
        "a complete Product artifact under bold labels, typed into the GitHub web UI",
    );

    // The mirror, and the one that fails open: an empty `## Done when` whose
    // next section is `**Testing**`. The same CRLF-blind gate misses the
    // boundary and certifies the testing notes as the acceptance bar.
    expect_missing(
        &as_eol(
            &format!(
                "## Problem\n\n{PROBLEM}\n\n## Done when\n\n\n**Testing**\n\n{THIRD_SECTION_BODY}\n"
            ),
            Eol::Crlf,
        ),
        &[Artifact::DoneWhenBar],
        "an empty done-when above a bold third section, typed into the GitHub web UI",
    );
}

#[test]
fn the_verdict_depends_on_nothing_but_the_change_it_was_handed() {
    // The gate runs inside a review that also runs dozens of other gates, and
    // this suite runs in parallel. A verdict that depends on anything other
    // than the two strings it was handed is a flake nothing here could
    // attribute — and product_bar.rs's own doc comment promises it makes no
    // network or filesystem call, which until now nothing pinned. A gate that
    // loaded its deferral vocabulary from a config file would satisfy every
    // behavioural assertion in this file and still be non-deterministic.
    //
    // The behavioural half first, so this test is red on the unimplemented
    // measurement rather than on the source scan.
    let bodies = [
        body_with(PROBLEM, BAR),
        problem_only(PROBLEM),
        bar_only(BAR),
        body_with(PROBLEM, "TBD"),
        String::new(),
    ];
    for body in &bodies {
        let first = product_bar::judge(body);
        let second = product_bar::judge(body);
        assert_eq!(
            first, second,
            "two calls on the same change disagreed: {first:?} then {second:?}. \
             body={body:?}"
        );
        assert_eq!(
            missing(body),
            missing(body),
            "the measurement is not stable across calls; body={body:?}"
        );
    }

    // The static half. Sanctioned source inspection, same idiom as the wiring
    // tests below; it lives inside this test rather than beside it because on
    // its own — against a module that is still `todo!()` — it would be a test
    // born green, and a test that has never been observed failing publishes
    // assurance it has not earned.
    // The previous revision of this half scanned for eight literal prefixes,
    // and the idiomatic spellings of the very I/O it forbade walked straight
    // past it: `use std::{env, fs};` contains neither "std::env" nor "std::fs",
    // and a grouped import is how anyone actually brings in two std modules.
    // `env!`, `option_env!` and `File::open` were not covered at all. A guard
    // that misreads what it guards is worse than none.
    //
    // So this asserts the shape of the import list instead, through
    // `impure_import` — a denylist of *effects*, not a whitelist of spellings.
    // See that function's docs for why the whitelist had to go: three rounds
    // running it turned a correct implementation red over an import with no
    // effect behind it (`regex`, then the `unicode_*` crates, then `std::mem`),
    // which is a guard misreading what it guards and, worse, a settled
    // specification test demanding to be edited mid-implementation.
    //
    // `use std::{cmp, fmt};` is how anyone brings in two std modules. The group
    // is expanded into its members and each member judged on its own, so the ban
    // on `std::{` is gone: it forbade a spelling, not an effect. `pub use` is
    // matched too — it re-exports exactly as far as `use` reaches, so leaving it
    // unmatched was a hole.
    //
    // The rule is only as good as the parser that feeds it, so the parser is
    // exercised before it is trusted — including the grouped form that used to
    // be banned outright and the `pub use` form that used to walk past.
    for (line, want) in [
        ("use regex::Regex;", vec!["regex::Regex"]),
        ("pub use regex::Regex;", vec!["regex::Regex"]),
        ("use std::{cmp, fmt};", vec!["std::cmp", "std::fmt"]),
        (
            "use std::{collections::BTreeSet, fmt::Write};",
            vec!["std::collections::BTreeSet", "std::fmt::Write"],
        ),
        ("use std::{env, fs};", vec!["std::env", "std::fs"]),
        (
            "use std::collections::{self, BTreeMap};",
            vec!["std::collections", "std::collections::BTreeMap"],
        ),
        ("use super::GateStatus;", vec!["super::GateStatus"]),
        ("let x = 1;", vec![]),
        ("    // use std::fs;", vec![]),
    ] {
        assert_eq!(
            imported_paths(line),
            want.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "imported_paths({line:?}) misread the import; an allowlist fed by a \
             parser that cannot read a grouped or re-exported import either bans a \
             correct implementation or lets the filesystem through"
        );
    }

    // And the effect rule itself, exercised on both sides before it is trusted.
    // The pure half of this table is not decoration: every one of these is an
    // import an ordinary line-splitting, string-trimming implementation reaches
    // for, and the whitelist this replaces rejected nine of them.
    for (path, impure) in [
        ("std::mem", false),
        ("std::char", false),
        ("std::ops::Range", false),
        ("std::convert::identity", false),
        ("std::num::NonZeroUsize", false),
        ("std::slice", false),
        ("std::hash::Hash", false),
        ("std::vec::Vec", false),
        ("std::iter::once", false),
        ("std::sync::Arc", false),
        ("std::sync::LazyLock", false),
        ("std::collections::BTreeMap", false),
        ("std::borrow::Cow", false),
        ("std::cmp::Reverse", false),
        ("std::fmt::Write", false),
        ("std::str::FromStr", false),
        ("core::fmt", false),
        ("alloc::string::String", false),
        ("super::GateStatus", false),
        ("crate::pre_merge_guard::GateStatus", false),
        ("self::inner", false),
        ("regex::Regex", false),
        ("unicode_segmentation::UnicodeSegmentation", false),
        ("std::fs", true),
        ("std::fs::File", true),
        ("std::env", true),
        ("std::env::var", true),
        ("std::io::Read", true),
        ("std::net::TcpStream", true),
        ("std::os::unix::fs::PermissionsExt", true),
        ("std::process::Command", true),
        ("std::time::SystemTime", true),
        ("std::thread", true),
        ("std::sync::mpsc::channel", true),
        ("reqwest::Client", true),
        ("tokio::fs", true),
        ("chrono::Utc", true),
        ("rand::random", true),
    ] {
        assert_eq!(
            impure_import(path).is_some(),
            impure,
            "impure_import({path:?}) misjudged the import. A determinism rule that \
             calls a pure module impure accuses a correct implementation and forces a \
             settled specification test to be edited mid-implementation; one that calls \
             an impure module pure lets a second source of truth for what the author \
             wrote in through the front door"
        );
    }

    // EVERY FILE THE GATE'S CODE CAN BE REACHED FROM — the module closure, not a
    // filename prefix. Scanning `product_bar.rs` alone made the whole property
    // escapable by moving the parser into a sibling module; scanning
    // `product_bar*` made it escapable by NAMING that sibling anything else,
    // which is one rename and the ordinary way a several-hundred-line parser
    // gets split out. Either way the behavioural half above stays green, because
    // it only proves two calls in one process agree and a file on disk does not
    // change between them. See `module_closure`.
    let sources = product_bar_sources();
    assert!(
        !sources.is_empty(),
        "no file matches src/pre_merge_guard/product_bar*.rs and there is no \
         src/pre_merge_guard/product_bar/ directory, so this scan has nothing to read \
         and the determinism property is vacuous. A guard that answers 'clean' without \
         looking at anything is worse than no guard"
    );
    assert!(
        sources.iter().any(|(rel, _)| {
            rel.ends_with("pre_merge_guard/product_bar.rs")
                || rel.ends_with("pre_merge_guard/product_bar/mod.rs")
        }),
        "the module this suite imports `judge` and `missing_artifacts` from is not \
         among the files this scan enumerated, so the scan is reading something other \
         than the gate. Found: {:?}",
        sources.iter().map(|(rel, _)| rel).collect::<Vec<_>>()
    );

    // The things reachable without a `use` line at all.
    //
    // Swept over source with the string literals blanked out, because this is a
    // ban on reaching for I/O and not a ban on words. `"Command"`, `"Instant"`
    // and `"::var("` are all things an author-facing failure message could
    // legitimately contain, and failing the gate for its prose would be a guard
    // misreading what it guards. The parser is exercised first, on the shapes
    // that separate a string from the code around it.
    // The last three cases are the ones that matter most, and none of them was
    // covered before: the parser toggled on any `"`, so a `'"'` char literal
    // left it stuck open, and it treated `\` as an escape inside a raw string,
    // so `r"C:\"` swallowed its own terminator. Either one blanks the entire
    // remainder of the file, which turns this sweep — the only guard that
    // catches `std::fs::read_to_string(..)` reached without a `use` line — into
    // a silent no-op. A blanking parser that can disable the guard it feeds is
    // the guard-misreads-what-it-guards failure, at its worst.
    for (line, want) in [
        (
            r#"let msg = "Command"; std::process::Command::new("git")"#,
            r#"let msg = "       "; std::process::Command::new("   ")"#,
        ),
        (
            r#"let m = "a \" Instant"; let t = Instant::now();"#,
            r#"let m = "            "; let t = Instant::now();"#,
        ),
        ("let x = 1;", "let x = 1;"),
        // A lifetime is not a char literal.
        (
            r#"fn f<'a>(s: &'a str) -> &'a str { s }"#,
            r#"fn f<'a>(s: &'a str) -> &'a str { s }"#,
        ),
        (r#"r"\\""#, r#"r"  ""#),
        (r##"r#"a \" b"#"##, r##"r#"      "#"##),
        (r#"if c == '"' {"#, "if c == ' ' {"),
        // And the whole point: the code AFTER a raw string and a char literal
        // is still visible to the sweep.
        (
            r##"let q = '"'; let raw = r"C:\"; std::fs::read_to_string(p)"##,
            r##"let q = ' '; let raw = r"   "; std::fs::read_to_string(p)"##,
        ),
    ] {
        assert_eq!(
            without_string_literals(line),
            want,
            "without_string_literals({line:?}) blanked the wrong span; a sweep fed by \
             a parser that cannot tell a message from a syscall either bans a correct \
             implementation for its prose or lets the filesystem through"
        );
    }

    // AND THE BLOCK-COMMENT STRIPPER, exercised before it is trusted for the
    // same reason as every other parser here: the sweep below bans eighteen
    // substrings, and prose is not an effect. A stripper that ate the code
    // after a comment would silence the sweep; one that ate nothing would fail
    // a correct gate for explaining itself.
    for (src_text, keep, lose) in [
        ("/* Command */\nlet x = 1;", "let x = 1;", "Command"),
        (
            "let a = 1; /* std::fs::read_to_string */ let b = 2;",
            "let b = 2;",
            "fs::",
        ),
        (
            "/* outer /* Instant */ still a comment */ let c = 3;",
            "let c = 3;",
            "Instant",
        ),
        (
            "/*\n * tokio\n */\nstd::process::Command::new(\"git\")",
            "Command::new",
            "tokio",
        ),
    ] {
        let stripped = without_block_comments(src_text);
        assert!(
            stripped.contains(keep),
            "without_block_comments({src_text:?}) swallowed the code after the \
             comment; a stripper that can blank the rest of a file silences the only \
             sweep that catches I/O reached without a `use` line. Got {stripped:?}"
        );
        assert!(
            !stripped.contains(lose),
            "without_block_comments({src_text:?}) left {lose:?} behind, so a gate that \
             explains itself in a block comment fails this test for its prose. Got \
             {stripped:?}"
        );
        assert_eq!(
            stripped.lines().count(),
            src_text.lines().count(),
            "without_block_comments must preserve line structure, or the per-line \
             import scan reads the wrong lines"
        );
    }

    // The things reachable with a `use` that `impure_import` admits — `use std;`
    // followed by a full path, or `use std::sync;` followed by `sync::mpsc` —
    // are caught here rather than by widening the import rule back into a ban on
    // spellings. `::io::` and `::net::` are spelled with both separators so that
    // an ordinary identifier ending in those two letters (`Ratio::new`) is not
    // mistaken for a syscall.
    // THE CLOSURE WALK AND THE RULE IT FEEDS, BOTH SIDES, on a fixture the real
    // tree cannot supply. The escape this replaces is one rename wide: the
    // previous revision enumerated only files whose NAME starts with
    // `product_bar`, and `impure_import` returns None for any `super::` path, so
    // a parser split into `bar_vocabulary.rs` — an ordinary name for an ordinary
    // split — was never opened, while `product_bar.rs` stayed spotless and the
    // behavioural half above stayed green, because it only proves two calls in
    // one process agree and a file on disk does not change between them.
    //
    // The clean half matters as much: a closure that reported a sibling reaching
    // for `std::fmt` would fail every correct implementation that splits its
    // parser out, which is the accusation this file forbids.
    const DELEGATING_GATE: &str = "use super::bar_vocabulary;\n\
                                   pub fn judge(pr_body: &str) -> GateStatus {\n\
                                   \x20   bar_vocabulary::judge(pr_body)\n\
                                   }\n";
    for (sibling, impure) in [
        (
            "use std::fs;\nfn deferrals() -> String { fs::read_to_string(\"d.txt\").unwrap() }\n",
            true,
        ),
        (
            "use std::fmt::Write;\nfn render(out: &mut String) { let _ = write!(out, \"x\"); }\n",
            false,
        ),
    ] {
        let mut fixture: BTreeMap<String, String> = BTreeMap::new();
        fixture.insert(
            "src/pre_merge_guard/product_bar.rs".to_string(),
            DELEGATING_GATE.to_string(),
        );
        fixture.insert(
            "src/pre_merge_guard/bar_vocabulary.rs".to_string(),
            sibling.to_string(),
        );
        let seeds = product_bar_seeds(&fixture);
        assert_eq!(
            seeds,
            vec!["src/pre_merge_guard/product_bar.rs".to_string()],
            "the seed rule must find the gate's own module and not its differently \
             named sibling, or this fixture is not exercising the closure at all"
        );
        let reached = module_closure(&fixture, &seeds);
        assert_eq!(
            reached
                .iter()
                .map(|(rel, _)| rel.as_str())
                .collect::<Vec<_>>(),
            vec![
                "src/pre_merge_guard/bar_vocabulary.rs",
                "src/pre_merge_guard/product_bar.rs",
            ],
            "the closure must follow `use super::bar_vocabulary;` to the file that \
             holds the parser. A scan that stops at the filename prefix is one rename \
             away from vacuous, and `impure_import` cannot see the delegation itself: \
             it returns None for every `super::` path, and has to"
        );
        assert_eq!(
            impurity_in(&reached).is_some(),
            impure,
            "the determinism rule misjudged a gate that delegates to a sibling \
             module. Missing the `std::fs` half lets the gate's vocabulary come off \
             disk, so in any environment where that file is absent or stale every \
             pull request is told it wrote no bar; reporting the `std::fmt` half \
             accuses a correct implementation that merely split its parser out. \
             Sibling: {sibling:?}"
        );
    }

    // And an ancestor module is not part of the closure: `use super::GateStatus;`
    // is how the gate imports its own return type, and following it to
    // `pre_merge_guard/mod.rs` would pull in the whole guard subtree and report
    // the Product seat for what `evaluator.rs` and `scanner.rs` do. See
    // `resolved_import_target`.
    let mut parent_fixture: BTreeMap<String, String> = BTreeMap::new();
    parent_fixture.insert(
        "src/pre_merge_guard/product_bar.rs".to_string(),
        "use super::GateStatus;\n".to_string(),
    );
    parent_fixture.insert(
        "src/pre_merge_guard/mod.rs".to_string(),
        "use std::process::Command;\npub mod product_bar;\n".to_string(),
    );
    let parent_seeds = product_bar_seeds(&parent_fixture);
    assert_eq!(
        module_closure(&parent_fixture, &parent_seeds)
            .iter()
            .map(|(rel, _)| rel.as_str())
            .collect::<Vec<_>>(),
        vec!["src/pre_merge_guard/product_bar.rs"],
        "the closure must stop at the gate's parent module. A gate is not \
         responsible for what its siblings do, and a settled specification test that \
         goes red when a NEIGHBOURING gate reaches for a subprocess is a guard \
         misreading what it guards"
    );

    // And now over the real tree.
    if let Some(defect) = impurity_in(&sources) {
        panic!("{defect}");
    }
}

// ---------------------------------------------------------------------------
// The corpus and the certification verdict
// ---------------------------------------------------------------------------

#[test]
fn the_product_bar_gate_joins_the_corpus_without_desynchronising_the_declared_total() {
    let report = PreMergeCertificationReport::unmeasured("fixture: nothing measured");

    let names: Vec<&str> = report
        .named_statuses()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(
        names.contains(&"product_bar_status"),
        "the new gate is a struct field that all_statuses() and named_statuses() \
         cannot see, so seal() cannot gate on it and the scorecard cannot name it. \
         That is how review_verdict_status stopped mattering. Present names: {names:?}"
    );

    assert_eq!(
        report.named_statuses().len(),
        report.all_statuses().len(),
        "the two listings must stay aligned, or a gate is reported in one and \
         invisible in the other"
    );
    assert_eq!(
        report.all_statuses().len(),
        TOTAL_GATES,
        "the corpus grew but TOTAL_GATES did not; TOTAL_GATES is published onto \
         pull requests, so every count claim it feeds is now wrong"
    );
}

#[test]
fn the_product_bar_name_is_bound_to_the_product_bar_field() {
    // Both listings are hand-written lists of seventy-odd near-identical
    // `_status` fields, so the likeliest mistake is not omission but a
    // copy-paste that pairs the new name with a neighbouring field:
    // `("product_bar_status", &self.test_suite_status)`. That passes the name
    // check above and both alignment tests in report.rs, and it makes the
    // scorecard report someone else's measurement under the Product seat's
    // name. Follows the idiom of report.rs's
    // `named_statuses_identifies_which_gates_failed`: mark one field and see
    // which name reports it.
    const PROBE: &str = "probe: the Product seat's own field, marked by this test";

    let mut report = PreMergeCertificationReport::unmeasured("fixture: nothing measured");
    report.product_bar_status = GateStatus::Failed(PROBE.to_string());

    let reporting: Vec<&str> = report
        .named_statuses()
        .into_iter()
        .filter(|(_, s)| matches!(s, GateStatus::Failed(m) if m == PROBE))
        .map(|(n, _)| n)
        .collect();
    assert_eq!(
        reporting,
        vec!["product_bar_status"],
        "exactly one name must report the field this test marked, and it must be \
         product_bar_status. An empty list means named_statuses() reads a different \
         field under that name; a different name means the Product seat's \
         measurement is published under someone else's gate"
    );

    let carried = report
        .all_statuses()
        .into_iter()
        .filter(|s| matches!(s, GateStatus::Failed(m) if m == PROBE))
        .count();
    assert_eq!(
        carried, 1,
        "all_statuses() must carry the product_bar_status field exactly once; \
         seal(), gate_counts() and recompute_unmeasured() all read that listing, so \
         a field missing from it gates nothing and a field listed twice is counted \
         twice"
    );
}

#[test]
fn the_adr_stops_publishing_the_product_seat_as_measuring_nothing() {
    // ADR-0002's own honesty law, in its own words, on the line directly above
    // the Discover roster: "Roster names are the live report fields minus the
    // `_status` suffix, so every `Today:` line can be checked mechanically
    // against `PreMergeCertificationReport`." Seat 1 reads "Today: nothing", and
    // nothing in this repository pins it. The only backstop for the corpus
    // growing at all, outside this file, is
    // `pre_merge_guard::matrix::tests::every_named_gate_has_exactly_one_label_and_vice_versa`,
    // which polices GATE_LABELS and never reads the ADR's prose. So a gate can
    // be added, wired, counted in TOTAL_GATES and rendered on every scorecard
    // while the published roster still says the seat measures nothing — the
    // published-name-versus-live-measurement hole that law exists to close,
    // pointed the other way.
    //
    // A previous revision declined to pin this on the ground that doing so would
    // be "the twenty named gates the ADR forbids". That conflates adding a GATE
    // with adding a TEST. The standing constraint is on gates; this adds an
    // assertion, and the assertion is about a document.
    //
    // The corpus half is asserted FIRST and in the same test on purpose: it is
    // what stops this going green by editing one word of a markdown file while
    // the seat still measures nothing.
    const FIELD: &str = "product_bar_status";
    let roster_name = FIELD
        .strip_suffix("_status")
        .expect("fixture invariant: the ADR's rule is the field name minus `_status`");

    let report = PreMergeCertificationReport::unmeasured("fixture: nothing measured");
    let names: Vec<&str> = report
        .named_statuses()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(
        names.contains(&FIELD),
        "the Product seat has no live gate to publish yet, so the ADR line below \
         cannot honestly name one. Live names: {names:?}"
    );

    const ADR: &str = "docs/adr/0002-agentic-roster-and-delivery-fabric.md";
    let adr = source(ADR);
    let seat = adr
        .lines()
        .find(|l| l.trim_start().starts_with("1. Product."))
        .unwrap_or_else(|| {
            panic!(
                "{ADR} no longer carries a Discover roster line beginning \"1. \
                 Product.\", so this test is not reading the roster any more and would \
                 answer without looking. Fix the test to find the new shape"
            )
        });

    assert!(
        seat.contains(roster_name),
        "{ADR} publishes the Product seat as {seat:?}, while \
         PreMergeCertificationReport now carries {FIELD:?} and blocks on it. The ADR's \
         own rule is that a roster name is the live report field minus `_status`, so \
         this line has to name {roster_name:?}. Publishing a seat as measuring \
         nothing while it measures something is the same defect as publishing a gate \
         that measures nothing, read from the other end"
    );
    assert!(
        !seat.contains("Today: nothing"),
        "{ADR} still says the Product seat measures nothing: {seat:?}"
    );
}

#[test]
fn a_missing_product_bar_withholds_certification() {
    // The ADR's consequence, at the level where it bites: "Quality sign-off
    // must fail if Product's bar is missing."
    // The TRANSITION, not the absolute pre-state. The previous revision opened
    // with `assert!(report.is_certified_ready, "sanity: …")`, which silently
    // encoded today's I1 policy that an individually `NotMeasured` gate still
    // certifies. That assertion is load-bearing — without something in its place
    // this test passes vacuously against a `seal()` that never certifies
    // anything — but it is load-bearing about the wrong thing: if a human later
    // tightens `is_certified_ready` to withhold on unmeasured gates, a plausible
    // change the report's own docs flag as deliberately decoupled, this test
    // goes red for a reason that has nothing to do with the Product seat, under
    // a message that reads as a fixture bug.
    //
    // So it asserts what this test is actually about: sealing a report whose
    // ONLY change is a failed Product bar must move certification from wherever
    // it was to withheld. That is falsifiable under either policy.
    let mut report = PreMergeCertificationReport::unmeasured("fixture: nothing measured");
    let before = report.is_certified_ready;

    report.product_bar_status =
        GateStatus::Failed("no done-when acceptance bar on the change".to_string());
    report.seal();

    assert!(
        !report.is_certified_ready,
        "a change with no acceptance bar was certified anyway; the Product gate is \
         carried on the report but not wired into the verdict"
    );
    assert!(
        before,
        "fixture invariant: the report has to be certifying BEFORE the Product gate \
         speaks, or the assertion above is satisfied by a seal() that withholds \
         certification from everything and this test measures nothing. It was not, so \
         either `unmeasured()` no longer produces a certifying report or \
         `is_certified_ready` now withholds on unmeasured gates — both are policy \
         changes elsewhere, and neither is a defect in the Product seat"
    );
}

// ---------------------------------------------------------------------------
// The wiring: the gate has to run on a real change
// ---------------------------------------------------------------------------
//
// `evaluate_pre_merge_gates` takes roughly fifty guard reports, so calling it
// from a test is not viable and nothing in the suite above can tell a perfect
// `product_bar::judge` apart from `let product_bar_status = GateStatus::Passed`.
// The existing backstop does not close it either:
// `every_computed_gate_reaches_the_report_test` only checks that a computed
// status is carried, so a literal satisfies it. The result would be a named gate
// rendered in the scorecard, counted in TOTAL_GATES, and blocking nothing —
// `review_verdict_status` reproduced one level up.
//
// This repository already sanctions source inspection for exactly this invariant
// class (`tests/evaluator_gate_ordering_test.rs` asserts on the evaluator's text,
// and `every_computed_gate_reaches_the_report_test.rs` parses the report literal),
// so these follow that idiom. Every scan runs over comment-stripped source, so a
// commented-out example cannot satisfy one.
//
// These are guards over *facts*, not over formatting. A wiring guard that
// misreads the wiring and then reports a correct implementation as broken is
// worse than no guard, so each assertion below is anchored on the loosest
// spelling that still carries the fact:
//
//   * the evaluator receives the change's **body** as a parameter whose name
//     says body (`pr_body`, `body`), and the judge call is rooted at that
//     parameter. The route through a body field on `PrDiffContext` is closed —
//     not for its spelling, but because its last link, that the constructor
//     stores the body it was handed, cannot be asserted from source, and an
//     unpopulated field reads as `""` for every pull request. See
//     `evaluator_body_parameters`;
//   * `product_bar_status` is derived from a call to product_bar's `judge` —
//     qualified (`product_bar::judge(...)`, `crate::pre_merge_guard::
//     product_bar::judge(...)`) or imported (`use super::product_bar::judge;`
//     then `judge(...)`), directly or through a `let` binding, with or without a
//     type annotation — and the field's value is that call and nothing else:
//     nothing wrapped around it, nothing chained onto it, nothing written over
//     it afterwards in the evaluator or in the pipeline;
//   * the pipeline hands that call site the change's body, whole, and no `let`
//     between the parameter and the call clamps it — under the same name or
//     under a new one;
//   * and every caller of the pipeline hands IT the change's body, for the same
//     reason the evaluator takes a parameter rather than reading a field: a
//     value chain is only as good as its weakest link, and this one had a caller
//     passing `""`. See
//     `every_caller_of_the_review_pipeline_hands_it_the_change_body`.
//
// `assert_the_wiring_parsers_read_a_real_wiring` exercises every one of those
// parsers on BOTH sides — the spellings a correct wiring is written in, and the
// fail-opens each rule exists for — so the guard is tested rather than assumed.

fn source(rel: &str) -> String {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `src` with `//` line comments removed, so a commented-out call or a doc
/// comment quoting one cannot satisfy any scan below.
fn without_line_comments(src: &str) -> String {
    src.lines()
        .map(strip_line_comment)
        .collect::<Vec<_>>()
        .join("\n")
}

/// `src` with `/* .. */` block comments blanked, newlines preserved so the
/// per-line scans keep their line numbers.
///
/// # Why this exists
///
/// `without_line_comments` handles `//` and nothing else, and the determinism
/// sweep below bans eighteen substrings — `Command`, `Instant`, `::var(` among
/// them — from what is left. So an ordinary Rust block comment in
/// `product_bar.rs` explaining, say, why the gate runs no `Command`, would fail
/// the test for its prose. That is the "a guard that misreads what it guards is
/// worse than none" failure this file spends a page warning about, left open in
/// the one comment syntax the stripper did not know.
///
/// Run AFTER `without_string_literals`, so a `/*` inside an author-facing
/// message cannot open a comment that swallows the rest of the file — the same
/// failure mode the raw-string and char-literal cases of that parser exist for.
/// Rust block comments nest, so the depth is counted rather than toggled.
fn without_block_comments(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            depth += 1;
            out.push_str("  ");
            i += 2;
            continue;
        }
        if depth > 0 && chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
            depth -= 1;
            out.push_str("  ");
            i += 2;
            continue;
        }
        if depth > 0 {
            out.push(if chars[i] == '\n' { '\n' } else { ' ' });
        } else {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}

/// Every `.rs` file under `src/`, keyed by its path relative to the crate root.
///
/// The universe the module closure below is resolved against. Read once and
/// passed around, so the closure walk is a pure function of a file map and can
/// be exercised on a fixture — see the two-file delegation fixture in
/// `the_verdict_depends_on_nothing_but_the_change_it_was_handed`.
fn crate_sources() -> BTreeMap<String, String> {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_rust_sources(&manifest.join("src"), &mut files);
    files
        .into_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(&manifest)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
            (rel, text)
        })
        .collect()
}

/// The files the Product gate's module is named after: everything matching
/// `src/pre_merge_guard/product_bar*.rs`, plus everything under a
/// `src/pre_merge_guard/product_bar/` directory.
///
/// The SEED of the closure, not the closure. A gate split across a module and
/// its siblings is reached from here by following its imports; see
/// `module_closure`.
fn product_bar_seeds(files: &BTreeMap<String, String>) -> Vec<String> {
    files
        .keys()
        .filter(|rel| {
            rel.starts_with("src/pre_merge_guard/product_bar")
                && (rel.ends_with(".rs"))
                && (rel["src/pre_merge_guard/product_bar".len()..].starts_with('/')
                    || !rel["src/pre_merge_guard/product_bar".len()..].contains('/'))
        })
        .cloned()
        .collect()
}

/// The module path a file declares, as segments under the crate root.
///
/// `src/pre_merge_guard/product_bar.rs` is `[pre_merge_guard, product_bar]`,
/// `src/pre_merge_guard/mod.rs` is `[pre_merge_guard]`, and `src/lib.rs` is the
/// crate root itself, `[]`.
fn module_path_of(rel: &str) -> Vec<String> {
    let Some(inner) = rel.strip_prefix("src/") else {
        return Vec::new();
    };
    let stem = inner.strip_suffix(".rs").unwrap_or(inner);
    let mut segments: Vec<String> = stem.split('/').map(|s| s.to_string()).collect();
    if matches!(
        segments.last().map(String::as_str),
        Some("mod" | "lib" | "main")
    ) {
        segments.pop();
    }
    segments
}

/// The file an intra-crate `use` path in `from_rel` reaches, or `None`.
///
/// `crate::`, `super::` and `self::` are resolved against the importing file's
/// own module path; anything else is an external crate, which `impure_import`
/// judges on its own. Trailing segments are dropped one at a time until a file
/// matches, because a `use` path ends in an ITEM (`super::bar_vocabulary::judge`
/// reaches `src/pre_merge_guard/bar_vocabulary.rs`) as often as in a module.
///
/// # Why an ancestor module is not a target
///
/// `use super::GateStatus;` is what the gate imports its own return type with,
/// and dropping the item segment off it lands on `src/pre_merge_guard/mod.rs` —
/// the gate's PARENT. Following that would pull the whole guard subtree into the
/// closure and the sweep would report the gate for what its neighbours do:
/// `evaluator.rs` shells out, `scanner.rs` reads the working tree, and neither
/// is the Product seat's code. A guard that misreads what it guards is worse
/// than no guard, so a resolved target that is a strict ancestor of the
/// importing module is not followed. The bound that leaves — a gate whose parser
/// lives in `pre_merge_guard/mod.rs` itself, which is the module tree's wiring
/// and not a place a markdown parser goes — is stated in open_questions.
fn resolved_import_target(
    from_rel: &str,
    path: &str,
    files: &BTreeMap<String, String>,
) -> Option<String> {
    let segments: Vec<&str> = path
        .split("::")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let from = module_path_of(from_rel);
    let mut here = from.clone();
    let mut rest = segments.as_slice();
    match *rest.first()? {
        "crate" => {
            here.clear();
            rest = &rest[1..];
        }
        "self" => rest = &rest[1..],
        "super" => {
            while rest.first() == Some(&"super") {
                here.pop()?;
                rest = &rest[1..];
            }
        }
        // An external crate. `impure_import` decides that one; there is no file
        // under `src/` to add.
        _ => return None,
    }

    let mut target: Vec<String> = here;
    target.extend(rest.iter().map(|s| (*s).to_string()));

    while !target.is_empty() {
        if target.len() < from.len() && from[..target.len()] == target[..] {
            return None;
        }
        let joined = target.join("/");
        for candidate in [format!("src/{joined}.rs"), format!("src/{joined}/mod.rs")] {
            if files.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        target.pop();
    }
    None
}

/// Every file the gate's code can be reached from: the seeds, plus every file
/// any of them imports, to a fixed point.
///
/// # Why the scanned universe is a closure and not a filename prefix
///
/// The determinism scan used to read exactly one file, then
/// `src/pre_merge_guard/product_bar*.rs` plus a `product_bar/` directory. The
/// behavioural half of that test only proves two calls in one process agree —
/// which a gate that reads a file satisfies trivially, because the file does not
/// change between the two calls — so the whole property lived on the source
/// scan, and the source scan was one RENAME away from vacuous:
///
/// ```text
/// // src/pre_merge_guard/product_bar.rs — passed the old scan unchanged
/// use super::bar_vocabulary;                     // impure_import -> None
/// pub fn judge(pr_body: &str) -> GateStatus { bar_vocabulary::judge(pr_body) }
///
/// // src/pre_merge_guard/bar_vocabulary.rs — matched no prefix, never opened
/// use std::fs;
/// fn deferrals() -> Vec<String> { fs::read_to_string("config/deferrals.txt")… }
/// ```
///
/// `impure_import` returns `None` for any path whose first segment is `crate`,
/// `self` or `super`, so the delegation itself is invisible by design — it has
/// to be, or every gate that imports its own `GateStatus` would be reported. The
/// previous revision's doc comment named exactly this escape and claimed to have
/// closed it; it closed it only for siblings an implementer happened to name
/// `product_bar*`, and `markdown.rs`, `deferrals.rs` or `bar_vocabulary.rs` are
/// all ordinary names for the module a several-hundred-line parser is split out
/// into. In any environment where the file it reads is absent or stale, every
/// pull request is then told it wrote no bar.
///
/// So the universe is the module closure the gate can actually reach. Both the
/// import rule and the substring sweep run over the whole of it, and
/// `impurity_in` is exercised on a two-file delegation fixture — one sibling
/// reaching for `std::fs`, one reaching only for `std::fmt` — before it is
/// trusted.
fn module_closure(files: &BTreeMap<String, String>, seeds: &[String]) -> Vec<(String, String)> {
    let mut chosen: BTreeSet<String> = seeds
        .iter()
        .filter(|rel| files.contains_key(*rel))
        .cloned()
        .collect();

    loop {
        let mut grew = false;
        for rel in chosen.clone() {
            let src = without_block_comments(&without_line_comments(&files[&rel]));
            for line in src.lines() {
                for path in imported_paths(line.trim()) {
                    if let Some(target) = resolved_import_target(&rel, &path, files)
                        && chosen.insert(target)
                    {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }

    chosen
        .into_iter()
        .map(|rel| {
            let text = files[&rel].clone();
            (rel, text)
        })
        .collect()
}

/// Why some file in the gate's module closure makes the verdict depend on
/// something other than the change it was handed — or `None`.
///
/// Both halves of the determinism rule, over the whole closure: the import rule
/// (`impure_import`, a denylist of EFFECTS) and the sweep for the things
/// reachable without a `use` line at all.
///
/// Returned rather than asserted so the rule can be exercised on both sides on a
/// fixture. A guard nobody has watched fire is a guard nobody knows fires.
fn impurity_in(sources: &[(String, String)]) -> Option<String> {
    for (rel, raw) in sources {
        let src = without_block_comments(&without_string_literals(&without_line_comments(raw)));
        for line in src.lines() {
            let trimmed = line.trim();
            for path in imported_paths(trimmed) {
                if let Some(reason) = impure_import(&path) {
                    return Some(format!(
                        "{rel} imports {trimmed:?}, which reaches {path:?} — {reason}. \
                         The Product artifact is authored on the change under review \
                         and nowhere else: a gate that reads a file, an environment \
                         variable, a clock or the network is both a flake this suite \
                         could not attribute and a second source of truth for what the \
                         author wrote"
                    ));
                }
            }
        }

        // The things reachable with a `use` that `impure_import` admits — `use
        // std;` followed by a full path, or `use std::sync;` followed by
        // `sync::mpsc` — are caught here rather than by widening the import rule
        // back into a ban on spellings. `::io::` and `::net::` are spelled with
        // both separators so that an ordinary identifier ending in those two
        // letters (`Ratio::new`) is not mistaken for a syscall.
        for forbidden in [
            "env!",
            "option_env!",
            "include_str!",
            "include_bytes!",
            "File::open",
            "fs::",
            "env::",
            "process::",
            "thread::",
            "mpsc",
            "::io::",
            "::net::",
            "Command",
            "::var(",
            "SystemTime",
            "Instant",
            "reqwest",
            "tokio",
        ] {
            if src.contains(forbidden) {
                return Some(format!(
                    "{rel} reaches for {forbidden}. The gate's verdict must be a \
                     function of the one string it was handed and nothing else"
                ));
            }
        }
    }
    None
}

/// Every source file the Product gate's own code can be reached from, as
/// (path relative to the crate root, source text).
///
/// The seeds under `src/pre_merge_guard/product_bar*` closed under every
/// intra-crate import. See `module_closure` for why the closure and not the
/// prefix, and `resolved_import_target` for the one bound it keeps.
fn product_bar_sources() -> Vec<(String, String)> {
    let files = crate_sources();
    let seeds = product_bar_seeds(&files);
    module_closure(&files, &seeds)
}
/// Every `.rs` file under `dir`, recursively.
fn collect_rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("a readable directory entry").path();
        if path.is_dir() {
            collect_rust_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// `src` with the contents of every string literal, raw string literal and
/// character literal blanked out — the delimiters left in place, and one blank
/// per character so nothing shifts.
///
/// Used only for the forbidden-substring sweep in
/// `the_verdict_depends_on_nothing_but_the_change_it_was_handed`. That sweep
/// bans `"Command"`, `"Instant"` and `"::var("` as *reaches for I/O*, and a
/// failure message that happened to contain one of those words would have
/// failed the test for a reason unrelated to determinism — the gate's own
/// author-facing prose is not a syscall. Code cannot hide inside a literal, so
/// blanking them costs the sweep nothing.
///
/// # Why it knows about char literals and raw strings
///
/// The previous revision toggled `in_string` on any `"` and treated `\` as an
/// escape everywhere inside one. Both are wrong in the direction that silently
/// DISABLES the sweep rather than tightening it:
///
///   * a `'"'` char literal — which any hand-rolled character parser in this
///     repository is liable to contain — flips the toggle on and never flips it
///     back, so every line below it is blanked;
///   * a raw string ending in an odd number of backslashes (`r"C:\"`) had its
///     closing quote consumed as an escaped character, with the same effect.
///
/// The sweep is the only guard that catches `std::env::var(..)` or
/// `std::fs::read_to_string(..)` reached without a `use` line, so a blanking
/// parser that can silently blank the rest of the file out from under it turns
/// that guard into a no-op — the guard-misreads-what-it-guards failure this
/// file calls worse than no guard at all. Lifetimes (`&'a str`, `'static`,
/// `'_`) are not char literals and are left alone.
fn without_string_literals(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;

    while i < chars.len() {
        // A raw string: r"…", r#"…"#, br##"…"##. Backslash is not an escape
        // inside one, and the terminator is a quote followed by as many `#` as
        // the opener carried.
        if let Some((hashes, open)) = raw_string_opener(&chars, i) {
            for c in &chars[i..=open] {
                out.push(*c);
            }
            let mut j = open + 1;
            while j < chars.len() && !raw_string_closes_at(&chars, j, hashes) {
                out.push(if chars[j] == '\n' { '\n' } else { ' ' });
                j += 1;
            }
            if j >= chars.len() {
                return out;
            }
            for c in &chars[j..=j + hashes] {
                out.push(*c);
            }
            i = j + hashes + 1;
            continue;
        }

        // A char literal: 'x', '\n', '\u{200b}' — but never a lifetime.
        if chars[i] == '\''
            && let Some(close) = char_literal_close(&chars, i)
        {
            out.push('\'');
            for _ in i + 1..close {
                out.push(' ');
            }
            out.push('\'');
            i = close + 1;
            continue;
        }

        // An ordinary string literal.
        if chars[i] == '"' {
            out.push('"');
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '"' {
                if chars[j] == '\\' {
                    out.push(' ');
                    if let Some(next) = chars.get(j + 1) {
                        out.push(if *next == '\n' { '\n' } else { ' ' });
                    }
                    j += 2;
                    continue;
                }
                out.push(if chars[j] == '\n' { '\n' } else { ' ' });
                j += 1;
            }
            if j >= chars.len() {
                return out;
            }
            out.push('"');
            i = j + 1;
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }
    out
}

/// `(hash count, index of the opening quote)` when a raw string literal starts
/// at `i`, and `None` otherwise. The `r` must open a token, so the `r` in `for`
/// is not mistaken for one.
fn raw_string_opener(chars: &[char], i: usize) -> Option<(usize, usize)> {
    if i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
        return None;
    }
    let mut k = i;
    if chars.get(k) == Some(&'b') {
        k += 1;
    }
    if chars.get(k) != Some(&'r') {
        return None;
    }
    k += 1;
    let first_hash = k;
    while chars.get(k) == Some(&'#') {
        k += 1;
    }
    if chars.get(k) != Some(&'"') {
        return None;
    }
    Some((k - first_hash, k))
}

/// Whether the raw string opened with `hashes` hashes terminates at `j`.
fn raw_string_closes_at(chars: &[char], j: usize, hashes: usize) -> bool {
    chars[j] == '"' && (1..=hashes).all(|h| chars.get(j + h) == Some(&'#'))
}

/// The index of the quote that closes the char literal starting at `i`, or
/// `None` when the `'` opens a lifetime rather than a literal.
fn char_literal_close(chars: &[char], i: usize) -> Option<usize> {
    match chars.get(i + 1)? {
        // '\n', '\'', '\\', '\u{feff}' — the escape is short, and the closing
        // quote is the next one.
        '\\' => (i + 2..chars.len().min(i + 14)).find(|k| chars[*k] == '\''),
        // 'x'. Anything else opened by a `'` is a lifetime.
        _ if chars.get(i + 2) == Some(&'\'') => Some(i + 2),
        _ => None,
    }
}

/// Every crate path a `use` line reaches, with a grouped import expanded into
/// its members: `use std::{cmp, fmt};` yields `std::cmp` and `std::fmt`, and
/// `use std::{collections::BTreeSet, fmt::Write};` yields both of those.
///
/// Returns empty for a line that is not an import.
///
/// Expansion rather than a ban on braces: the property the allowlist enforces
/// is that the verdict is a function of the string the gate was handed, and a
/// grouped import reaches exactly the same modules a sequence of single imports
/// would. `pub use` counts as an import for the same reason.
fn imported_paths(line: &str) -> Vec<String> {
    let t = line.trim();
    let rest = t
        .strip_prefix("pub use ")
        .or_else(|| t.strip_prefix("use "))
        .map(|r| r.trim().trim_end_matches(';').trim())
        .map(|r| r.trim_start_matches("::"));
    let Some(rest) = rest else {
        return Vec::new();
    };

    let Some(open) = rest.find('{') else {
        return vec![rest.to_string()];
    };
    let prefix = &rest[..open];
    let Some(close) = rest.rfind('}') else {
        return vec![rest.to_string()];
    };
    let inner = &rest[open + 1..close];

    // Split the group on the commas that are not inside a nested group, then
    // expand each member against the shared prefix. `self` re-imports the
    // prefix itself.
    let mut members: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for c in inner.chars() {
        match c {
            '{' => {
                depth += 1;
                current.push(c);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ',' if depth == 0 => members.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    members.push(current);

    let mut out = Vec::new();
    for member in members {
        let member = member.trim();
        if member.is_empty() {
            continue;
        }
        if member == "self" {
            out.push(prefix.trim_end_matches("::").to_string());
        } else if member.contains('{') {
            out.extend(imported_paths(&format!("use {prefix}{member};")));
        } else {
            out.push(format!("{prefix}{member}"));
        }
    }
    out
}

/// Why importing `path` would make the verdict depend on something other than
/// the change the gate was handed — or `None` if it cannot.
///
/// # Why this is a denylist and not an allowlist
///
/// Three revisions of this file stated the determinism rule as a whitelist of
/// import prefixes, and three times review found a correct implementation turned
/// red by it for a spelling with no effect behind it. `regex` and the
/// `unicode_*` crates were added in the first two rounds; the third found the
/// list still missing `std::mem`, `std::char`, `std::ops`, `std::convert`,
/// `std::num`, `std::slice`, `std::hash`, `std::sync::Arc` and `std::vec`, so
/// `use std::mem;` in a line-splitting loop was accused of reading "a file, an
/// environment variable, a clock or the network". The list forbade a spelling
/// rather than an effect, and each round told the implementer to edit a settled
/// specification test mid-implementation — the one thing this project's method
/// forbids.
///
/// The stated property is what this now checks: a file, an environment variable,
/// a socket, a clock, another thread or another process. Those live in a short,
/// stable set of `std` subtrees, and everything else in `std`, `core` and
/// `alloc` is a pure data structure or a pure formatting or text facility. A
/// third-party crate is a different matter — it can do anything — so those are
/// still admitted by name, from a small list of pure-text crates.
///
/// `crate::`, `super::` and `self::` are admitted: this crate's own modules are
/// this crate's business, and the forbidden-token sweep below is what catches a
/// gate reaching for I/O through one of them without a `use` line. That
/// remaining seam is listed in open_questions.
fn impure_import(path: &str) -> Option<&'static str> {
    /// The `std` subtrees that reach outside the process, the machine or the
    /// moment. Matched on `::` segment boundaries, so `std::iter` is not
    /// `std::io` and `std::sync::Arc` is not `std::sync::mpsc`.
    const IMPURE_SUBTREES: &[&str] = &[
        "std::env",
        "std::fs",
        "std::io",
        "std::net",
        "std::os",
        "std::process",
        "std::sync::mpsc",
        "std::thread",
        "std::time",
    ];

    /// Third-party crates that read the string they are handed and nothing else.
    /// `regex` is a first-class dependency of this crate and the house idiom for
    /// exactly this kind of marker parsing — src/cedar_guard.rs,
    /// src/supply_chain_guard.rs, src/clean_architecture_guard.rs,
    /// src/adr_drift_ratchet.rs and src/cell_isolation_guard.rs all open with
    /// `use regex::Regex;`. The rest are the pure-text crates this suite's
    /// invisible-character and grapheme demands point an implementer at.
    const PURE_CRATES: &[&str] = &[
        "aho_corasick",
        "itertools",
        "lazy_static",
        "memchr",
        "once_cell",
        "regex",
        "regex_lite",
        "unicode_normalization",
        "unicode_segmentation",
        "unicode_width",
    ];

    let path = path.trim();
    for subtree in IMPURE_SUBTREES {
        if path == *subtree || path.starts_with(&format!("{subtree}::")) {
            return Some(
                "a std subtree that reaches outside the process, the machine or the \
                 moment — a file, an environment variable, a socket, a clock, another \
                 thread or another process",
            );
        }
    }

    let first = path.split("::").next().unwrap_or(path).trim();
    match first {
        "std" | "core" | "alloc" | "crate" | "self" | "super" => None,
        _ if PURE_CRATES.contains(&first) => None,
        _ => Some(
            "a third-party crate outside the small list of pure-text crates, which can \
             do anything at all",
        ),
    }
}

/// One line, with any `//` comment on it dropped.
///
/// `//` inside a string literal is not a comment. The naive version of this
/// truncated at the first `//` anywhere on the line, so a single URL in a
/// message string — `"see https://…"` — would silently blind every scan below
/// by cutting the line before the code on it. Nothing in the evaluator or the
/// review pipeline carries one today; the point is that adding one must not
/// quietly turn a wiring guard into a no-op.
fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_string => {
                i += 2;
                continue;
            }
            b'"' => in_string = !in_string,
            b'/' if !in_string && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// The identifier a `let` introduces immediately before `head` ends, if the
/// text between the `=` and the end of `head` is still the same statement.
///
/// A type annotation is stripped before the identifier is validated. Without
/// that, `let product_bar_status: GateStatus = product_bar::judge(pr_body);` —
/// a perfectly correct wiring — yielded no binding at all, and the wiring test
/// then failed a correct implementation with a message accusing it of the exact
/// defect it did not have.
fn let_binding_before(head: &str) -> Option<String> {
    let pos = head.rfind("let ")?;
    let tail = &head[pos + "let ".len()..];
    let eq = tail.find('=')?;
    if tail[eq + 1..].contains(';') {
        return None;
    }

    let mut name = tail[..eq].trim();
    name = name.strip_prefix("mut ").unwrap_or(name).trim();
    // `x: GateStatus` -> `x`. A binding name cannot contain `:`, so the first
    // one on the left of the `=` opens the annotation.
    if let Some(colon) = name.find(':') {
        name = name[..colon].trim();
    }

    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some(name.to_string())
}

/// Whether `src` brings product_bar's `judge` into scope unqualified.
fn imports_product_bar_judge(src: &str) -> bool {
    src.lines().any(|l| {
        let t = l.trim();
        t.starts_with("use ")
            && t.contains("product_bar")
            && (t.contains("judge") || t.contains("::*"))
    })
}

/// Whether `expr` is, or contains, a call to product_bar's `judge`.
fn is_a_product_bar_judge_call(src: &str, expr: &str) -> bool {
    expr.contains("product_bar::judge(")
        || (imports_product_bar_judge(src) && expr.contains("judge("))
}

/// Every call to product_bar's `judge` in `src`, as (binding, arguments).
///
/// `binding` is `Some(name)` when the call is the initialiser of `let name = `.
///
/// Reaching `judge` through an import is as correct as qualifying it, so a bare
/// `judge(` counts too — but only when `src` actually imports it, so an
/// unrelated gate's `judge` appearing in the evaluator later cannot turn these
/// tests red for a reason that has nothing to do with the Product seat.
fn product_bar_judge_calls(src: &str) -> Vec<JudgeCall> {
    let mut anchors: Vec<&str> = vec!["product_bar::judge("];
    if imports_product_bar_judge(src) {
        anchors.push("judge(");
    }

    let mut out = Vec::new();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for anchor in anchors {
        out.extend(calls_at_anchor(src, anchor, &mut seen));
    }
    out
}

/// One call to product_bar's `judge`.
///
/// `tail` is what the source does with the value the instant the call closes,
/// and it is carried because cutting the call expression out — which is how
/// `unmeasured_alternatives_in` reads a statement — also cuts out everything
/// the residue could have told us about what is done TO the result. See
/// `post_processing_after`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct JudgeCall {
    /// `Some(name)` when the call is the initialiser of `let name = `.
    binding: Option<String>,
    /// The text between the call's parentheses, trimmed of whitespace and of a
    /// single trailing comma — the two things rustfmt adds when it wraps a call
    /// across lines, and neither of which is anything the source DID to the
    /// argument. See `calls_at_anchor`.
    args: String,
    /// The text immediately after the call's closing paren, truncated to the
    /// handful of characters this file asks a question of.
    tail: String,
}

/// The calls whose `(` follows `anchor`, skipping opening parens already
/// claimed by an earlier (more specific) anchor.
///
/// # Why the captured argument is normalised
///
/// `args` used to be the raw text between the parentheses, and
/// `truncated_argument` then demanded that `arg.trim()` be a plain path. For a
/// call rustfmt wraps — which is what happens the moment the fully qualified
/// path is written inside the deeply indented report literal, or the implementer
/// simply formats it that way —
///
/// ```text
/// product_bar::judge(
///     pr_body,
/// )
/// ```
///
/// the raw text trims to `"pr_body,"`. A trailing comma is not a plain path and
/// no `WHOLE_VALUE_ADAPTERS` suffix strips it, so the guard panicked that a
/// perfectly correct wiring "is not a plain path to the change's body" —
/// accusing it of the truncation defect it does not have, and leaving the
/// implementer editing a settled specification test mid-implementation, which
/// this project's method forbids.
///
/// Whitespace and rustfmt's trailing comma are formatting, not effects, so they
/// are removed before any rule reads the argument. Everything the rules are
/// actually about — a slice, a `.take(`, a literal, a different identifier —
/// survives the trim untouched. The sibling path was already safe: at the
/// pipeline's own call site the argument comes from `one_argument_per_line`,
/// which trims and drops the trailing comma already.
fn calls_at_anchor(src: &str, anchor: &str, seen: &mut BTreeSet<usize>) -> Vec<JudgeCall> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(i) = src[from..].find(anchor) {
        let start = from + i;
        let open = start + anchor.len() - 1;
        if !seen.insert(open) {
            from = open + 1;
            continue;
        }
        let mut depth = 0i32;
        let mut end = None;
        for (k, c) in src[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + k);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(end) = end {
            let raw = src[open + 1..end].trim();
            out.push(JudgeCall {
                binding: let_binding_before(&src[..start]),
                args: raw.strip_suffix(',').unwrap_or(raw).trim().to_string(),
                tail: src[end + 1..].chars().take(40).collect(),
            });
        }
        from = open + 1;
    }
    out
}

/// The initialiser expressions given to `field` in every struct literal in
/// `src`. A shorthand `field,` yields the field's own name.
fn struct_field_initialisers(src: &str, field: &str) -> Vec<String> {
    let shorthand = format!("{field},");
    let labelled = format!("{field}:");
    src.lines()
        .filter_map(|l| {
            let t = l.trim();
            if t == shorthand || t == field {
                Some(field.to_string())
            } else {
                t.strip_prefix(&labelled)
                    .map(|rest| rest.trim().trim_end_matches(',').trim().to_string())
            }
        })
        .collect()
}

/// The leading identifier of an expression, e.g. `pb` in `pb.clone()`.
fn leading_ident(expr: &str) -> String {
    expr.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// The identifier an expression is rooted at: `pr_body` in `&pr_body.trim()`.
///
/// `&`, `&mut ` and surrounding space are stripped first, because all three are
/// how a caller spells "this value" rather than something done to it.
fn root_ident(expr: &str) -> String {
    let e = expr
        .trim()
        .trim_start_matches('&')
        .trim_start()
        .trim_start_matches("mut ")
        .trim_start();
    leading_ident(e)
}

/// The parameter names declared by a signature that writes one parameter per
/// line, which is how `evaluate_pre_merge_gates` is written.
///
/// The first line is the `fn name(` itself, and `&self` carries no colon, so
/// both drop out. A signature this parser cannot read produces an empty list,
/// and `the_evaluator_receives_the_change_under_review` fails loudly on that
/// rather than letting the guard answer "no" without looking.
fn signature_parameters(signature: &str) -> Vec<String> {
    signature
        .lines()
        .skip(1)
        .filter_map(|l| {
            let t = l.trim().trim_start_matches("mut ").trim();
            let colon = t.find(':')?;
            let name = t[..colon].trim();
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// The declared signature that opens at `anchor`, from the `fn` line to the `)`
/// that closes its parameter list.
fn signature_of(src: &str, anchor: &str) -> Option<String> {
    let start = src.find(anchor)?;
    let tail = &src[start..];
    let end = tail.find(") -> ")?;
    Some(tail[..end].to_string())
}

/// The declared signature of `evaluate_pre_merge_gates`, from `pub fn` to the
/// `)` that closes its parameter list.
fn evaluator_signature(src: &str) -> Option<String> {
    signature_of(src, "pub fn evaluate_pre_merge_gates(")
}

/// The arguments of the call whose opening paren ends `call` at `start`, one per
/// line — the way every call this file reads is actually written.
///
/// `Err` when the call is NOT written one argument per line. A wiring guard that
/// cannot read the wiring must say so, rather than quietly find no arguments and
/// pass, or report a swap that is not there.
fn one_argument_per_line(src: &str, start: usize, call: &str) -> Result<Vec<String>, String> {
    let after_open = start + call.len();
    let line_end = src[after_open..]
        .find('\n')
        .map(|i| after_open + i)
        .unwrap_or(src.len());
    let first_line_tail = &src[after_open..line_end];
    if !first_line_tail.trim().is_empty() {
        return Err(format!(
            "this test reads one argument per line, and a call to {call} carries \
             {first_line_tail:?} on its opening line. Fix the test to parse the new \
             shape rather than let it mis-parse — a wiring guard that cannot read the \
             wiring is worse than none"
        ));
    }
    Ok(src[start..]
        .lines()
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with(')'))
        .map(|l| l.trim().trim_end_matches(',').to_string())
        .collect())
}

/// The parameters of `evaluate_pre_merge_gates` whose names say body.
///
/// # Why the body must arrive as a PARAMETER, and not on `PrDiffContext`
///
/// Two revisions of these guards accepted a second route: put `pub pr_body:
/// String` on `PrDiffContext` — the change-under-review struct the evaluator
/// already receives — populate it where the context is built, and judge
/// `&diff_ctx.pr_body`. That really is a correct and arguably cleaner wiring
/// than a sixty-ninth positional argument, and the guards were widened to admit
/// it so this file would not decide the implementer's surface for them.
///
/// It cannot be closed from source alone, and review found the hole. Nothing
/// here can assert that `GitManager::prepare_pr_diff` STORES the parameter it
/// was handed onto the field it declares. Add the field, add a `pr_body: &str`
/// parameter, construct the context with `pr_body: String::new()` — or populate
/// it at one of the several construction sites and miss the one
/// `prepare_pr_diff` uses — and every wiring assertion passes: the pipeline
/// statement mentions the body, the evaluator's signature takes a
/// `PrDiffContext` and reads `.pr_body`, and `judge(&diff_ctx.pr_body)` is an
/// untruncated plain path with nothing chained onto it. `judge` itself stays
/// perfectly correct, so every behavioural test in this file stays green, and
/// the gate reads `""` for every pull request and reports BOTH artifacts
/// missing on 100% of changes. That is the fabricated accusation at full
/// incidence, which this file names as equal in severity to a false green.
///
/// The behavioural close the reviewer asked for — construct a `PrDiffContext`
/// from a known non-empty body and assert the field carries that exact string —
/// cannot be written against `prepare_pr_diff`, which clones a repository and
/// shells out to `git fetch` before it constructs anything. A test that reaches
/// the network is neither hermetic nor deterministic, and a source scan of the
/// constructor is the same grep one layer down.
///
/// So route two is closed and route one is required. A parameter is not a grep:
/// it can hold only what the caller passed, so `body` -> parameter ->
/// `judge(pr_body)` is a value chain with no unasserted link in it. The cost is
/// one more argument on a function that already takes sixty-eight, in a file
/// whose house idiom is exactly that. This is listed in open_questions as a
/// decision a human can veto; vetoing it means reopening route two AND writing
/// the constructor seam that lets its last link be asserted behaviourally.
fn evaluator_body_parameters() -> Vec<String> {
    let src = without_line_comments(&source("src/pre_merge_guard/evaluator.rs"));
    evaluator_signature(&src)
        .map(|sig| {
            signature_parameters(&sig)
                .into_iter()
                .filter(|p| p.contains("body"))
                .collect()
        })
        .unwrap_or_default()
}

/// The `let` statement that binds `name` in `src`, from the `let` to the `;`
/// that closes it.
///
/// Used to ask what a call-site argument was built from: if the body rides on
/// the change-under-review struct rather than on a parameter of its own, the
/// pipeline's obligation is that the statement producing that struct was handed
/// the body.
fn binding_statement(src: &str, name: &str) -> Option<String> {
    binding_statements(src, name).into_iter().next()
}

/// Every `let` statement in `src` that binds `name`, in source order.
///
/// `binding_statement` answers "what was this built from" and only ever needs
/// the first. `shadowed_argument` asks a different question — "did ANYTHING
/// rebind this name between the parameter list and the call" — and a guard that
/// looked only at the first `let` would be defeated by writing the clamp under
/// a harmless one.
fn binding_statements(src: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(i) = src[from..].find("let ") {
        let start = from + i;
        let tail = &src[start + "let ".len()..];
        let bound = tail
            .trim_start()
            .trim_start_matches("mut ")
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>();
        if bound == name {
            let end = tail.find(';').map(|e| start + "let ".len() + e + 1);
            out.push(src[start..end.unwrap_or(src.len())].to_string());
        }
        from = start + "let ".len();
    }
    out
}

/// What a `let` statement binds its name TO: the text between the `=` that
/// opens the initialiser and the `;` that closes the statement.
fn binding_initialiser(statement: &str) -> Option<String> {
    let (_, rhs) = statement.split_once('=')?;
    Some(rhs.trim().trim_end_matches(';').trim().to_string())
}

/// Whether `name` occurs in `expr` as a whole identifier rather than as part of
/// a longer one, so `pr_body` is not found inside `pr_body_len`.
fn mentions_ident(expr: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    expr.match_indices(name).any(|(i, _)| {
        let before = expr[..i].chars().next_back();
        let after = expr[i + name.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// Why the value `root` names is no longer the whole change body by the time
/// `judge` is handed it — or `None` when nothing between the parameter list and
/// the call touched it.
///
/// # Why the argument's TEXT is not enough
///
/// `truncated_argument` reads only the literal characters between the judge
/// call's parentheses, so it is defeated by one shadowing `let` on the line
/// above — which is idiomatic Rust and precisely where a defensive clamp on
/// webhook-supplied text gets written:
///
/// ```text
/// let pr_body = &pr_body[..4000.min(pr_body.len())];   // clamp
/// let product_bar_status = product_bar::judge(pr_body);
/// ```
///
/// `root_ident("pr_body")` is a declared body parameter; `truncated_argument`
/// sees a plain path and returns `None`; `unmeasured_alternatives_in` reads the
/// `product_bar_status` statement, whose residue is `let product_bar_status = ;`;
/// `assignments_to` finds nothing, because the shadow writes `pr_body` and not
/// `product_bar_status`. `judge` stays perfectly correct, so
/// `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact` cannot see it —
/// and every author of a long, careful pull request is told they wrote no
/// acceptance bar. That is the exact defect `truncated_argument` exists to
/// close, reinstated one line above the call it guards.
///
/// So the rule follows the VALUE rather than the token: a `let` that rebinds
/// `root` from `root` is put through the same argument rule as an inline
/// expression would be. A rebinding that drops none of the text — `let pr_body =
/// pr_body.trim();` — stays clean, because `truncated_argument` judges the
/// effect and not the spelling.
///
/// A `let` of the same name whose initialiser does NOT mention `root` and names
/// no other body either is not a shadow of the parameter at all: it is an
/// unrelated binding, and the shape that occurs in practice is a fixture in the
/// file's own `#[cfg(test)]` module. Flagging those would be exactly the
/// fabricated accusation this file forbids, so they are left alone, and the
/// contents of every string literal are blanked before the question is asked —
/// `let body = "- [ ] p99 < 5ms";` is markdown, not an index expression.
///
/// # The clamp under a NEW name
///
/// Review found the same defect wearing a different label:
///
/// ```text
/// let body_excerpt = &body[..2000];
/// … evaluate_pre_merge_gates(…, body_excerpt, …)
/// ```
///
/// `root_ident("body_excerpt")` still contains "body", so the argument is the
/// one the guards pick up and it reads as a plain path; the clamp is one line up
/// under a name that shadows nothing. That arm is judged by the EFFECT rule
/// alone (`truncating_token_in`) rather than by the plain-path rule, because
/// `let pr_body = meta.body.unwrap_or_default();` drops none of the text and is
/// an ordinary correct binding that the strict rule would accuse.
fn shadowed_argument(src: &str, root: &str) -> Option<String> {
    for statement in binding_statements(src, root) {
        let Some(raw) = binding_initialiser(&statement) else {
            continue;
        };
        let initialiser = without_string_literals(&raw);
        if mentions_ident(&initialiser, root) {
            if let Some(defect) = truncated_argument(&initialiser) {
                return Some(format!(
                    "{statement:?} rebinds {root:?} from itself before the gate sees \
                     it, and {defect}"
                ));
            }
        } else if names_the_change_body(&initialiser)
            && let Some(defect) = truncating_token_in(&initialiser)
        {
            return Some(format!(
                "{statement:?} binds {root:?} to less than the whole change body \
                 before the gate sees it, and {defect}"
            ));
        }
    }
    None
}

/// Every line of `src` that assigns to `name` WITHOUT introducing it.
///
/// `binding_statement` stops at the semicolon that closes the `let` — its own
/// parser test requires that, or every binding would look as though it were
/// built from every later value — so `unmeasured_alternatives_in` sees only
/// what the initialiser does. A fail-open written as a later REASSIGNMENT is
/// therefore invisible to it, and `let_binding_before` strips `mut ` on
/// purpose, so `let mut product_bar_status = …` is an accepted binding:
///
///     let mut product_bar_status = product_bar::judge(&pr_body);
///     if pr_body.trim().is_empty() {
///         product_bar_status = GateStatus::NotMeasured { .. };
///     }
///
/// binds the right name, calls `judge` over the body, leaves nothing behind
/// when the call is cut out of the `let`, and hands the struct the shorthand.
/// Every wiring assertion passes, `judge` stays perfectly correct so every
/// behavioural test passes — and every pull request opened with an empty body
/// certifies the Product seat as `NotMeasured`, which `is_acceptable()` returns
/// true for. One line away from the `if` form the guard already closes.
///
/// The `let` line itself is not an assignment by this reading: its left-hand
/// side trims to `let name`, or `let mut name`, or `let name: GateStatus`, none
/// of which equals `name`. Comparisons are excluded so `product_bar_status ==
/// GateStatus::Passed` is not mistaken for a write, because a guard that
/// misreads a correct implementation and reports it as the defect it does not
/// have is worse than no guard.
///
/// # Why the receiver does not matter
///
/// A previous revision matched only when the whole left-hand side trimmed to
/// the bare identifier, so it saw `product_bar_status = …` and never saw
/// `report.product_bar_status = …` — the same fail-open, written onto the field
/// instead of onto the binding, and one the evaluator's own neighbours already
/// spell that way (`src/pre_merge_guard/report.rs` writes
/// `r.test_suite_status = GateStatus::Errored(…)`). Review verified it against
/// a working reference gate: inserting
///
/// ```text
/// if pr_body.is_empty() {
///     report.product_bar_status = GateStatus::Passed;
/// }
/// ```
///
/// immediately before `report.seal();` was rustfmt-clean, passed every test in
/// this file and every other test in the repository, and certified the Product
/// seat for every change opened with an empty body. The guard worked for
/// exactly one of the two spellings of one fail-open.
///
/// So the question is asked of the PATH: any left-hand side ending in
/// `.product_bar_status`, on any receiver (`report.`, `r.`, `self.`), is a
/// write to the field. A struct-literal line (`product_bar_status,` or
/// `product_bar_status: product_bar::judge(pr_body),`) carries no `=` at all
/// and is not one, and neither is a comparison.
fn assignments_to(src: &str, name: &str) -> Vec<String> {
    src.lines()
        .filter(|line| {
            let Some((lhs, rhs)) = line.split_once('=') else {
                return false;
            };
            // `==` — a comparison, not a write.
            if rhs.starts_with('=') {
                return false;
            }
            // `!=`, `<=`, `>=`, `+=`-style compounds: the operator's first
            // character sits at the end of what `split_once` called the LHS.
            if lhs.ends_with(['!', '<', '>', '+', '-', '*', '/', '%', '&', '|', '^']) {
                return false;
            }
            writes_to(lhs, name)
        })
        .map(|l| l.trim().to_string())
        .collect()
}

/// Whether the left-hand side `lhs` writes `name`: the bare binding, or `name`
/// as the last segment of a field path on any receiver.
fn writes_to(lhs: &str, name: &str) -> bool {
    let lhs = lhs.trim();
    if lhs == name {
        return true;
    }
    let Some(receiver) = lhs.strip_suffix(&format!(".{name}")) else {
        return false;
    };
    !receiver.is_empty()
        && receiver
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '*')
}

/// `text` with every product_bar `judge(..)` call expression cut out of it.
///
/// What is left of a statement once the measurement is removed is everything
/// the statement does *besides* measuring. For a correct wiring that residue is
/// `let product_bar_status = ;` and holds nothing; for a wiring that keeps a
/// second value in reserve it holds the reserve.
fn without_judge_calls(src: &str, text: &str) -> String {
    let mut anchors: Vec<&str> = vec!["product_bar::judge("];
    if imports_product_bar_judge(src) {
        anchors.push("judge(");
    }

    let mut out = text.to_string();
    loop {
        let Some((start, anchor)) = anchors
            .iter()
            .filter_map(|a| out.find(*a).map(|i| (i, *a)))
            .min_by_key(|(i, _)| *i)
        else {
            return out;
        };
        let open = start + anchor.len() - 1;
        let mut depth = 0i32;
        let mut end = None;
        for (k, c) in out[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + k);
                        break;
                    }
                }
                _ => {}
            }
        }
        let cut_to = end.unwrap_or(out.len() - 1);
        // The whole PATH, not just the anchor. `crate::pre_merge_guard::
        // product_bar::judge(pr_body)` is a correct and ordinary spelling, and a
        // cut that starts at `product_bar::judge(` leaves `crate::pre_merge_guard::`
        // behind — which the residue rule below would then report as something
        // the statement does BESIDES measuring. A guard that misreads a correct
        // wiring is worse than no guard.
        let from = path_start(&out, start);
        out.replace_range(from..=cut_to, " ");
    }
}

/// Where the module path leading into a call at `at` begins.
///
/// `crate::pre_merge_guard::product_bar::judge(..)` is one expression, and a cut
/// that starts at the anchor leaves the front of the path standing.
fn path_start(text: &str, at: usize) -> usize {
    let mut start = at;
    while let Some(head) = text[..start].strip_suffix("::") {
        let segment = head
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
            .last();
        match segment {
            Some((i, _)) => start = i,
            None => return start,
        }
    }
    start
}

/// The syntax that BINDS a value to a name and nothing else — `let`, an
/// optional `mut`, the bound identifier, an optional type annotation, and the
/// `=` that opens the initialiser — stripped off the front of `text`.
///
/// `None` when `text` opens with no binding head at all (a struct-literal
/// initialiser has none) or with one this parser cannot read. The caller keeps
/// the whole text in that case, so an unreadable head surfaces as a residue and
/// fails loudly rather than being quietly skipped.
fn binding_head_of(text: &str) -> Option<&str> {
    let rest = text.trim_start().strip_prefix("let ")?.trim_start();
    let rest = rest.strip_prefix("mut ").unwrap_or(rest).trim_start();
    let name_end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if name_end == 0 {
        return None;
    }
    let rest = rest[name_end..].trim_start();
    let rest = match rest.strip_prefix(':') {
        Some(annotated) => {
            let eq = annotated.find('=')?;
            let ty = annotated[..eq].trim();
            // A type, not an expression: no call, no block, no index, no
            // arithmetic. `GateStatus`, `Option<GateStatus>` and
            // `crate::pre_merge_guard::GateStatus` are types; `soften(x)` is not,
            // and an annotation is the one place a wrapper could otherwise hide
            // from the residue.
            if ty.is_empty()
                || !ty
                    .chars()
                    .all(|c| c.is_alphanumeric() || " _:<>&,'".contains(c))
            {
                return None;
            }
            &annotated[eq..]
        }
        None => rest,
    };
    rest.strip_prefix('=')
}

/// What is left of `statement` once the measurement AND the syntax that binds
/// it to a name have both been removed. Empty is the only acceptable answer.
///
/// # Why a residue and not a denylist
///
/// The previous revision asked whether any of four substrings — `GateStatus::`,
/// `if `, `match `, `unwrap_or` — survived the cut. That catches the shapes it
/// enumerates and nothing else, and review measured what it misses. A softening
/// wrapper written AROUND the call rather than chained onto it:
///
/// ```text
/// product_bar_status: soften(product_bar::judge(pr_body)),
/// ```
///
/// where `soften` maps `Failed(m)` onto `Warning(m)`, leaves the residue
/// `soften( )`, which holds none of the four. `post_processing_after` cannot see
/// it either: it inspects only what FOLLOWS the call's closing paren, and it
/// treats `)` as clean because that is what an argument looks like. So the
/// byte-identical fail-open this file already closes for `.softened()` stood
/// open one keystroke to the LEFT of the call, and review confirmed it against a
/// working reference gate — `test result: ok. 41 passed; 0 failed`, with every
/// change certifying the Product seat because `Warning` is `is_acceptable()`.
/// `GateStatus::from(product_bar::judge(pr_body))` is the same shape, and so is
/// any wrapper nobody has thought of yet.
///
/// A denylist can only ever name the wrappers someone thought of. The residue
/// states the rule instead: the field's value IS the measurement, so a statement
/// that does anything besides bind it has something left over.
fn residue_beyond_the_measurement(src: &str, statement: &str) -> String {
    let cut = without_judge_calls(src, statement);
    let tail = binding_head_of(&cut).unwrap_or(cut.as_str()).to_string();
    tail.trim().trim_end_matches([';', ',']).trim().to_string()
}

/// Whether any identifier in `expr` is named for the change's body.
///
/// Identifier-wise rather than substring-wise, and over EVERY identifier rather
/// than only the root: `&meta.body.unwrap_or_default()` is the body reached
/// through a field and `&pr_body` is the body reached through a local, and both
/// are ordinary correct spellings at a call site. `&pr.title` and `"main"` are
/// neither. Callers that care whether the expression is a literal ask that
/// separately, because a literal can spell any word it likes.
fn names_the_change_body(expr: &str) -> bool {
    let mut identifier = String::new();
    let mut found = false;
    for c in expr.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() || c == '_' {
            identifier.push(c);
        } else {
            found |= identifier.to_lowercase().contains("body");
            identifier.clear();
        }
    }
    found
}

/// Expressions that construct an EMPTY value of the body's type.
///
/// None of them can hold anything the author wrote, so a local bound to one and
/// then passed at the body position is the `""` literal wearing a name.
/// Spelled without their arguments so `String::new()` and `String::new( )` are
/// the same thing to this list.
const EMPTY_VALUE_CONSTRUCTORS: &[&str] = &[
    "String::new(",
    "String::default(",
    "Default::default(",
    "str::default(",
];

/// Why the local a call site passes at the body position does not hold the
/// change's body — or `None` when nothing in the file says it does not.
///
/// # Why the caller hop needs a value chain and not a grep
///
/// `every_caller_of_the_review_pipeline_hands_it_the_change_body` used to ask
/// three questions about the ARGUMENT's spelling and nothing about what the
/// argument was BOUND to: that it holds no `"`, that some identifier in it says
/// body, and that it carries no truncating token. `shadowed_argument` — the rule
/// that follows the value one line up, applied at the pipeline's own call site
/// by this same commit — was never applied here. So the cheapest way to turn
/// that test green at the one call site this repository has that owns no body,
/// `src/cli/server.rs`, was
///
/// ```text
/// let pr_body = String::new();          // or pr.title.clone()
/// … execute_pr_review(…, &pr_body, …)
/// ```
///
/// `root_ident` is `pr_body`, `names_the_change_body` is true, there is no
/// literal in the argument text and no truncating token, and the position lines
/// up. Every assertion passed, `judge` stayed perfectly correct, every other
/// test in the suite stayed green — and the outage-recovery sweep still handed
/// the gate the empty string, so every pull request certified through that path
/// was told it wrote neither artifact. That is the same 100%-incidence
/// fabricated accusation the test exists to prevent, reinstated one line above
/// the call it guards, and it abandons this file's own argument against the
/// `PrDiffContext` route — "a parameter is not a grep: it can hold only what the
/// caller passed" — at precisely the hop where the known-bad call site lives.
///
/// # When the rule applies, and why not always
///
/// Only when the argument is ROOTED at the identifier that names the body — a
/// local claiming to BE the body, `&pr_body`. At three of this repository's call
/// sites the body is reached through a FIELD of a local
/// (`&meta.body.unwrap_or_default()`), where the local is a metadata struct and
/// is not claiming to be the body at all: following `meta` to `let meta =
/// fetch_pr_metadata(..).await?;` and demanding that initialiser say "body"
/// would report all three correct call sites, which is the accusation this file
/// forbids. The field access does the naming there, and the `"` and
/// truncating-token rules already cover it.
///
/// The bound: a root bound by no `let` in the file — a parameter of the
/// enclosing function — is left alone. Following it is the next hop up, and this
/// rule stops at one; see open_questions.
fn caller_binding_defect(src: &str, root: &str) -> Option<String> {
    // The LAST binding before the call is the one in effect at it, and `src` is
    // the file up to the call, so a `#[cfg(test)]` fixture further down the file
    // that happens to bind the same name is not mistaken for the wiring.
    let statement = binding_statements(src, root).pop()?;
    let initialiser = binding_initialiser(&statement)?;

    if initialiser.contains('"') {
        return Some(format!(
            "{statement:?} binds {root:?} to a string literal, which can hold only \
             what this file wrote and never what the author of the change wrote"
        ));
    }
    let dense: String = initialiser.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(ctor) = EMPTY_VALUE_CONSTRUCTORS
        .iter()
        .find(|c| dense.contains(**c))
    {
        return Some(format!(
            "{statement:?} binds {root:?} to an empty-value constructor ({ctor}…)), \
             which can hold nothing the author wrote — the empty-string literal \
             wearing the name of the body"
        ));
    }
    if !names_the_change_body(&initialiser) {
        return Some(format!(
            "{statement:?} binds {root:?} to something no identifier in which names \
             the pull request's body"
        ));
    }
    if let Some(defect) = truncating_token_in(&without_string_literals(&initialiser)) {
        return Some(format!(
            "{statement:?} binds {root:?} to less than the whole change body, and \
             {defect}"
        ));
    }
    None
}

/// Argument syntax that hands the gate LESS than the whole change body.
///
/// Every one of these slices, skips or shortens the text before `judge` ever
/// sees it, and each is an ordinary thing to write.
const TRUNCATING_TOKENS: &[&str] = &[
    "[",
    "..",
    ".lines(",
    ".take(",
    ".chars(",
    ".truncate",
    ".get(",
];

/// Method calls that hand the callee the same text in a different type, or with
/// surrounding whitespace removed. None of them can drop the middle or the end
/// of a body, which is the only thing the argument rule is about, so rejecting
/// them would be forbidding a spelling rather than an effect.
const WHOLE_VALUE_ADAPTERS: &[&str] = &[
    "as_deref",
    "as_ref",
    "as_str",
    "clone",
    "into",
    "to_owned",
    "to_string",
    "trim",
    "trim_end",
    "trim_start",
];

/// Why `arg` is not the whole change body — or `None` when it is a plain path
/// to it, with at most a leading `&`.
///
/// # Why the argument is inspected at all
///
/// `unmeasured_alternatives_in` cuts the entire `judge(..)` call expression out
/// of the statement before looking at what is left, so it is blind to what was
/// done to the call's INPUT. The whole point of
/// `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact` — two kilobytes
/// of background, with fixture invariants demanding the marker sit past byte
/// 1500 and line 25 — is reinstated one layer up by
///
/// ```text
/// let product_bar_status =
///     product_bar::judge(&pr_body[..2000.min(pr_body.len())]);
/// ```
///
/// The argument still contains "body". The residue is `let product_bar_status =
/// ;` and holds no alternative. Nothing is reassigned. `judge` itself stays
/// perfectly correct, so every behavioural assertion in this file is green —
/// and every author who writes a long, careful pull request is told they wrote
/// no acceptance bar. `judge(&pr_body.lines().take(40).collect::<String>())`
/// is the same defect in the other unit.
fn truncated_argument(arg: &str) -> Option<String> {
    if let Some(defect) = truncating_token_in(arg) {
        return Some(defect);
    }

    let mut expr = arg.trim();
    while let Some(rest) = expr
        .strip_prefix('&')
        .or_else(|| expr.strip_prefix('*'))
        .or_else(|| expr.strip_prefix("mut "))
    {
        expr = rest.trim_start();
    }

    loop {
        if is_plain_path(expr) {
            return None;
        }
        let Some(head) = expr.strip_suffix("()") else {
            break;
        };
        let Some(dot) = head.rfind('.') else {
            break;
        };
        if !WHOLE_VALUE_ADAPTERS.contains(&&head[dot + 1..]) {
            break;
        }
        expr = head[..dot].trim_end();
    }

    Some(format!(
        "{expr:?} is not a plain path to the change's body. The argument must be the \
         identifier or field access itself — `pr_body`, `&self.pr_body` — possibly \
         through one of {WHOLE_VALUE_ADAPTERS:?}, none of which can drop any of the text"
    ))
}

/// The first of `TRUNCATING_TOKENS` in `text`, and why it is one.
///
/// The EFFECT half of the argument rule, split out because it is the only half
/// that survives being applied one hop further up the value chain.
/// `truncated_argument` additionally demands a PLAIN PATH, which is exactly
/// right for the expression handed straight to `judge` and wrong for a caller's
/// `&meta.body.unwrap_or_default()` — an ordinary, correct spelling of "the body
/// this pull request carries" that drops none of the text and is not a plain
/// path. Applying the strict rule up there would be the fabricated accusation
/// this file forbids; applying the effect rule catches `&body[..4000]` at the
/// same call sites without touching the correct ones.
fn truncating_token_in(text: &str) -> Option<String> {
    TRUNCATING_TOKENS
        .iter()
        .find(|token| text.contains(**token))
        .map(|token| {
            format!(
                "it carries {token:?}, which slices, skips or shortens the body before \
                 the gate ever sees it"
            )
        })
}

/// Whether `expr` is a bare identifier or a field access: `pr_body`,
/// `self.pr_body`.
fn is_plain_path(expr: &str) -> bool {
    !expr.is_empty()
        && !expr.starts_with('.')
        && !expr.ends_with('.')
        && expr
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == ':')
}

/// The character chained onto a `judge(..)` call, or `None` when the verdict is
/// used exactly as it comes.
///
/// # Why the closing paren is inspected at all
///
/// Cutting the call expression out to look at the residue also cuts out
/// everything the residue could have caught about what is done TO the result:
///
/// ```text
/// let product_bar_status = product_bar::judge(&pr_body).softened();
/// ```
///
/// where `softened()` maps `Failed(m)` onto `Warning(m)` — the "soft launch so
/// we do not break open pull requests" change an engineer writes on day one.
/// `Warning` is `is_acceptable()`, so every change certifies the Product seat.
/// The residue holds `.softened();` and none of `UNMEASURED_ALTERNATIVES`; the
/// field is bound to a judge call; nothing else looks. That is the exact
/// fail-open the conditional and reassignment guards were built for, one method
/// call to the right of both of them. `.or(..)` and `.map(..)` are the same
/// shape.
///
/// A verdict used as it comes is followed by `;` (a `let`), `,` (a struct
/// literal field) or `)` (an argument). Anything else is something happening to
/// the measurement between `judge` and the field.
fn post_processing_after(tail: &str) -> Option<char> {
    let next = tail.trim_start().chars().next()?;
    if matches!(next, ';' | ',' | ')') {
        None
    } else {
        Some(next)
    }
}

#[test]
fn the_evaluator_receives_the_change_under_review() {
    assert_the_wiring_parsers_read_a_real_wiring();

    let src = without_line_comments(&source("src/pre_merge_guard/evaluator.rs"));
    let signature =
        evaluator_signature(&src).expect("the evaluator declares evaluate_pre_merge_gates");

    // This parser reads one parameter per line. If the signature is ever
    // collapsed, it must say so rather than quietly find no parameters and
    // report the wiring absent — a wiring guard that cannot read the wiring is
    // worse than none.
    let parameters = signature_parameters(&signature);
    assert!(
        parameters.len() > 4,
        "this test reads one parameter per line and found only {parameters:?} on \
         evaluate_pre_merge_gates. Fix the test to parse the new shape rather than \
         let it mis-parse. Signature: {signature:?}"
    );

    // The fact, not the spelling: the evaluator has to be handed the text the
    // Product artifact is written in, AS A PARAMETER. A gate cannot measure
    // text the evaluator is never given, and a gate that measures nothing gates
    // nothing.
    //
    // See `evaluator_body_parameters` for why the route through a `pr_body`
    // field on `PrDiffContext` is closed rather than accepted: nothing readable
    // from source establishes that the constructor stores the body it was
    // handed, so that route leaves a link where `pr_body` can be `String::new()`
    // for every pull request while every assertion in these three tests passes
    // and `judge` stays perfectly correct. A parameter is not a grep — it can
    // hold only what the caller passed — so `body` -> parameter -> `judge` is a
    // value chain with no unasserted link. Listed in open_questions.
    let body_parameters = evaluator_body_parameters();
    assert!(
        !body_parameters.is_empty(),
        "evaluate_pre_merge_gates never receives the change's body, which is where \
         the written problem and the done-when bar are authored. Hand it a parameter \
         whose name says body. Putting the body on PrDiffContext instead is not \
         enough: nothing this file can read establishes that the constructor stores \
         the body it was handed, so an unpopulated field reads as the empty string \
         for every pull request and the gate reports both artifacts missing on every \
         change while judging correctly. Parameters: {parameters:?}"
    );
}

#[test]
fn the_evaluator_computes_product_bar_status_by_judging_that_change() {
    assert_the_wiring_parsers_read_a_real_wiring();

    let src = without_line_comments(&source("src/pre_merge_guard/evaluator.rs"));
    let body_parameters = evaluator_body_parameters();

    let calls = product_bar_judge_calls(&src);
    assert!(
        !calls.is_empty(),
        "src/pre_merge_guard/evaluator.rs never calls product_bar::judge, so \
         product_bar_status is not derived from the change under review. A gate \
         computed from nothing gates nothing: it is named on the scorecard, counted \
         in TOTAL_GATES, and blocks no pull request"
    );

    for call in &calls {
        let args = &call.args;

        // ROOTED AT THE PARAMETER THE EVALUATOR WAS HANDED, not merely holding
        // the substring "body". `judge(&diff_ctx.pr_body)` contains it too, and
        // a `pr_body` field the constructor never populates is the empty string
        // for every pull request — the gate then reports both artifacts missing
        // on 100% of changes while `judge` stays perfectly correct and every
        // behavioural test in this file stays green. See
        // `evaluator_body_parameters`.
        let root = root_ident(args);
        assert!(
            body_parameters.contains(&root),
            "the Product gate is judged over judge({args}), which is rooted at \
             {root:?} — not at any parameter of evaluate_pre_merge_gates whose name \
             says body ({body_parameters:?}). The gate must read the text the \
             evaluator was HANDED. A field on a struct the evaluator merely receives \
             is not that: nothing here can establish that the struct's constructor \
             stored the body, so an unpopulated field reads as \"\" for every pull \
             request and the seat accuses every author of writing neither artifact"
        );

        // AND THE WHOLE OF IT. The residue check below cuts the judge call out
        // before it looks, so it is blind to what was done to the call's INPUT.
        // `judge(&pr_body[..2000.min(pr_body.len())])` still
        // names the body, still leaves an empty residue, still binds the field
        // to a judge call — and reinstates the truncation that
        // `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact` exists to
        // close, one layer up where no behavioural test can see it.
        if let Some(defect) = truncated_argument(args) {
            panic!(
                "the Product gate is handed less than the change's body: \
                 judge({args}) — {defect}. A bar written at the far end of a long \
                 pull request is still the artifact, so a gate that reads a prefix of \
                 the body tells every author of a long, careful change that they wrote \
                 no acceptance bar. `judge` stays perfectly correct while it does it, \
                 which is why no behavioural test in this file can catch it"
            );
        }

        // AND NOTHING CLAMPED IT ON THE WAY IN. The rule above reads the literal
        // text between the call's parentheses, so one shadowing `let` on the
        // line above defeats it — and a shadowing `let` is idiomatic Rust and
        // exactly where a defensive clamp on webhook-supplied text gets written:
        //
        //     let pr_body = &pr_body[..4000.min(pr_body.len())];
        //     let product_bar_status = product_bar::judge(pr_body);
        //
        // The argument is a plain path rooted at a declared body parameter, the
        // residue is empty, and nothing assigns to `product_bar_status` — every
        // other guard here is satisfied, `judge` stays perfectly correct, and
        // every author of a long pull request is told they wrote no bar. So the
        // guard follows the value rather than the token. See
        // `shadowed_argument`.
        if let Some(defect) = shadowed_argument(&src, &root) {
            panic!(
                "the Product gate is handed less than the change's body: {defect}. The \
                 argument to judge({args}) reads as a plain path only because the \
                 clamp was written one line above the call. A bar written at the far \
                 end of a long pull request is still the artifact, and `judge` stays \
                 perfectly correct while a clamp here tells every author of a long, \
                 careful change that they wrote no acceptance bar"
            );
        }

        // AND THE VERDICT IS USED AS IT COMES. Cutting the call out also cuts
        // out what is done to its RESULT: `judge(&pr_body).softened()`,
        // mapping Failed onto Warning, leaves a residue holding none of
        // UNMEASURED_ALTERNATIVES and certifies every change, because Warning is
        // `is_acceptable()`.
        if let Some(chained) = post_processing_after(&call.tail) {
            panic!(
                "something is chained onto the Product gate's verdict: {chained:?} \
                 follows judge({args}) in the evaluator. Nothing may sit between the \
                 measurement and the field — `.softened()`, `.or(..)` and `.map(..)` \
                 all map Failed onto a status `is_acceptable()` returns true for, so \
                 every change certifies the Product seat while every behavioural test \
                 in this file stays green. If a change legitimately passes, say so in \
                 `judge`, where \
                 a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured can see it"
            );
        }
    }

    // Binding the field to the call, not merely to the absence of a literal. A
    // scan for "a line holding both product_bar_status and GateStatus::" is
    // satisfied by `let pb = GateStatus::Passed;` plus `product_bar_status: pb,`
    // on two separate lines, and by the realistic copy-paste
    // `product_bar_status: doc_parity_status.clone(),`. Both are caught here:
    // the initialiser must be the judge call itself, or an identifier a `let`
    // bound to one.
    let bindings: Vec<String> = calls.iter().filter_map(|c| c.binding.clone()).collect();
    let initialisers = struct_field_initialisers(&src, "product_bar_status");
    assert!(
        !initialisers.is_empty(),
        "no struct literal in the evaluator gives product_bar_status a value, so the \
         report cannot carry the Product seat's measurement"
    );
    for init in &initialisers {
        let derived_from_the_call =
            is_a_product_bar_judge_call(&src, init) || bindings.contains(&leading_ident(init));
        assert!(
            derived_from_the_call,
            "product_bar_status is initialised from {init:?}, which is neither a \
             product_bar::judge call nor an identifier bound to one. A literal Passed \
             certifies every change; a literal NotMeasured leaves a named gate that \
             measures nothing forever; a neighbouring gate's status publishes someone \
             else's measurement under the Product seat's name. Bindings from judge \
             calls: {bindings:?}"
        );

        // AND THE VALUE IS THE MEASUREMENT, UNCONDITIONALLY. Everything above
        // is satisfied by a conditional whose other branch fails open —
        //
        //     let product_bar_status = if pr_body.trim().is_empty() {
        //         GateStatus::NotMeasured { .. }
        //     } else {
        //         product_bar::judge(pr_body)
        //     };
        //
        // — which binds the right name, calls judge over the body, and hands
        // the struct the shorthand. `judge` stays perfectly correct, so every
        // behavioural test in this file is green, and every pull request opened
        // with an empty body certifies the Product seat as NotMeasured, which
        // `is_acceptable()` returns true for. That is the exact defect this
        // file exists to close, reintroduced at the one seam no behavioural
        // test reaches: an empty body is precisely the change with no bar.
        //
        // AND THE WRAPPING FORM OF THE SAME THING, which is where the previous
        // revision of this guard stopped. It asked whether any of four
        // substrings survived the cut — `GateStatus::`, `if `, `match `,
        // `unwrap_or` — and review measured what a denylist of four misses:
        //
        //     product_bar_status: soften(product_bar::judge(pr_body)),
        //
        // with `fn soften(s: GateStatus) -> GateStatus` mapping `Failed(m)` onto
        // `Warning(m)`. The residue is `soften( )`, which holds none of the
        // four; `post_processing_after` cannot see it either, because it reads
        // only what FOLLOWS the closing paren and treats `)` as clean — that is
        // what an argument looks like. So the byte-identical fail-open this
        // file already closes for `.softened()` stood open one keystroke to the
        // LEFT of the call. Review confirmed it against a working reference
        // gate: `test result: ok. 41 passed; 0 failed`, with `Warning`
        // `is_acceptable()` and every change certifying the Product seat.
        //
        // So the statement is asked what is left of it once the measurement AND
        // the syntax that binds the measurement to a name are both removed. For
        // a correct wiring, nothing at all — and that rule needs no list of the
        // wrappers anybody happened to think of.
        let statement = if is_a_product_bar_judge_call(&src, init) {
            init.clone()
        } else {
            binding_statement(&src, &leading_ident(init)).unwrap_or_else(|| init.clone())
        };
        let residue = residue_beyond_the_measurement(&src, &statement);
        assert!(
            residue.is_empty(),
            "product_bar_status is not simply the measurement: {residue:?} is left of \
             {statement:?} once the product_bar::judge call and the syntax binding it \
             to a name are cut out of it. The field's value must be the judge call and \
             nothing else. A wrapper around the call maps Failed onto a status \
             `is_acceptable()` returns true for and certifies every change; a branch \
             that answers NotMeasured for a body the gate found nothing in certifies \
             every change whose author wrote nothing, which is the one case this seat \
             is for — absence of the bar IS the defect, not an unread measurement. If \
             a change can legitimately pass, say so in `judge`, where \
             a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured can see it"
        );

        // AND NOTHING LATER OVERWRITES IT. The check above reads the `let`
        // statement and stops at its semicolon, because `binding_statement`
        // must stop there or every binding looks as though it were built from
        // every later value. So the same fail-open, written as a reassignment
        // one line down, walks straight past it:
        //
        //     let mut product_bar_status = product_bar::judge(&pr_body);
        //     if pr_body.trim().is_empty() {
        //         product_bar_status = GateStatus::NotMeasured { .. };
        //     }
        //
        // `let_binding_before` strips `mut ` deliberately, so that binding is
        // accepted; the initialiser residue is empty; the struct takes the
        // shorthand. Every wiring assertion passes and every pull request with
        // an empty body certifies the Product seat as NotMeasured — which is
        // `a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured`
        // reinstated at the one seam no behavioural test reaches.
        // The field itself, unconditionally — the shorthand initialiser is not
        // the only route to it. `report.product_bar_status = GateStatus::Passed;`
        // placed before `report.seal()` is the same fail-open written onto the
        // field instead of onto the binding, it is the idiom report.rs already
        // uses (`r.test_suite_status = GateStatus::Errored(..)`), and review
        // verified it passes every other test in this repository.
        let mut overwrites = assignments_to(&src, "product_bar_status");
        let bound = leading_ident(init);
        if !is_a_product_bar_judge_call(&src, init) && bound != "product_bar_status" {
            overwrites.extend(assignments_to(&src, &bound));
        }
        assert!(
            overwrites.is_empty(),
            "product_bar_status is measured and then written over: {overwrites:?} \
             assign to the Product seat's status after it is bound to the judge call. \
             The field's value must be the measurement, full stop — a later branch \
             that substitutes NotMeasured (or Passed, or Warning) for a body the gate \
             found nothing in certifies every change whose author wrote nothing, \
             which is the one case this seat exists for. If the body can \
             legitimately be absent, say so in `judge`, where \
             a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured can see it"
        );
    }
}

#[test]
fn the_review_pipeline_hands_the_evaluator_the_change_body() {
    // The evaluator can only judge what the pipeline gives it. `body` is
    // already in scope at this call site — it is handed to `review_pr` and to
    // `ensure_documentation_parity` a few lines above — so the only thing that
    // can go wrong is forgetting to pass it, and then the Product gate is a
    // name on the scorecard with nothing behind it.
    const CALL: &str = "evaluate_pre_merge_gates(";
    let src = without_line_comments(&source("src/webhook/pipelines/review.rs"));
    let start = src
        .find(CALL)
        .expect("the review pipeline evaluates the pre-merge gates");

    // This parser reads one argument per line. If the call is ever collapsed
    // onto a single line it must fail loudly here rather than silently find no
    // arguments and pass, or report a swap that is not there.
    let args = one_argument_per_line(&src, start, CALL).unwrap_or_else(|why| panic!("{why}"));

    // CORRESPONDENCE, NOT PRESENCE. The previous revision asserted only that
    // SOME argument in this list was rooted at a name containing "body", while
    // `the_evaluator_receives_the_change_under_review` asserted only that SOME
    // parameter of the signature was named for one. Nothing bound argument N to
    // parameter N and nothing checked the two lists were the same length — so a
    // positional swap type-checked and passed the whole suite.
    //
    // It is not a hypothetical. `evaluate_pre_merge_gates` already declares a
    // second `&str` parameter next door (`review_verdict: &str`) and this call
    // site already passes `&review_resp.verdict` immediately before
    // `&shape_outcome`. Declare `pr_body: &str` beside `review_verdict` and get
    // the argument order wrong by one position and the compiler says nothing:
    //
    //     // evaluator.rs            // review.rs
    //     test_suite_passed: bool,   true,
    //     pr_body: &str,             &review_resp.verdict,
    //     review_verdict: &str,      body,
    //
    // `evaluator_body_parameters()` finds `pr_body`; `judge(pr_body)` is rooted
    // at it, is a plain path, and has nothing chained on; this test finds `body`
    // somewhere in the argument list. `judge` stays perfectly correct, so every
    // behavioural assertion in this file is green — and the Product seat judges
    // the AI reviewer's verdict string instead of the pull request body, and
    // reports BOTH artifacts missing on 100% of pull requests. That is the
    // fabricated accusation at full incidence, which this file names as equal in
    // severity to a false green.
    //
    // So the two ordered lists are compared as ordered lists.
    let evaluator_src = without_line_comments(&source("src/pre_merge_guard/evaluator.rs"));
    let parameters = signature_parameters(
        &evaluator_signature(&evaluator_src)
            .expect("the evaluator declares evaluate_pre_merge_gates"),
    );
    assert_eq!(
        args.len(),
        parameters.len(),
        "this test's two parsers disagree about how many things are being passed: \
         {} arguments at the call site, {} parameters on the signature. `&self` \
         carries no colon and drops out of the parameter list, so a correct wiring \
         has these equal. Fix the test to parse the new shape rather than let it \
         compare positions that do not line up — a wiring guard that cannot read the \
         wiring is worse than none. Arguments: {args:?}. Parameters: {parameters:?}",
        args.len(),
        parameters.len()
    );

    // ONE ROUTE, deliberately: an argument rooted at an identifier whose name
    // says body. See `evaluator_body_parameters` for why the route through a
    // `pr_body` field on `PrDiffContext` is closed rather than accepted.
    //
    // Rooted at, not merely containing. `&doc_report` and `&review_resp.verdict`
    // are both bound by statements that mention the body — `…, title, body)` —
    // and neither hands the body to the gate; the previous revision's check was
    // satisfied by those two bindings, which predate the change they were meant
    // to guard.
    let handed_over: Vec<usize> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| root_ident(a).contains("body"))
        .map(|(i, _)| i)
        .collect();
    let declared: Vec<usize> = parameters
        .iter()
        .enumerate()
        .filter(|(_, p)| p.contains("body"))
        .map(|(i, _)| i)
        .collect();

    // Exactly one on each side, so the position check below cannot be made
    // vacuous by "body" appearing twice: with two candidates on either side an
    // "any index matches any index" reading would pass a swap again.
    assert_eq!(
        handed_over.len(),
        1,
        "the review pipeline must hand the evaluator the pull request body — where \
         the written problem and the done-when bar are authored — exactly once. \
         `body` is already in scope at this call site: it goes to `review_pr` and to \
         `ensure_documentation_parity` a few lines above, so pass it, as an argument \
         rooted at the body itself. A gate handed nothing gates nothing, and a gate \
         handed the empty string accuses every author of writing neither artifact. \
         Two such arguments would make the position check below meaningless. \
         Arguments rooted at a name saying body: {handed_over:?}. Arguments: {args:?}"
    );
    assert_eq!(
        declared.len(),
        1,
        "evaluate_pre_merge_gates must declare exactly one parameter whose name says \
         body, or the position check below cannot tell which one the pipeline is \
         supposed to be filling. Parameters: {parameters:?}"
    );
    assert_eq!(
        handed_over[0],
        declared[0],
        "the pipeline passes the pull request body at argument position {}, and \
         evaluate_pre_merge_gates declares its body parameter at position {}. Rust \
         binds arguments to parameters BY POSITION, and the neighbouring \
         `review_verdict: &str` takes the same type — so this compiles, `judge` stays \
         perfectly correct, every behavioural test in this file stays green, and the \
         Product seat measures the AI reviewer's verdict string instead of the change \
         under review. Every pull request is then told it carries neither artifact. \
         Argument at {}: {:?}. Parameter at {}: {:?}",
        handed_over[0],
        declared[0],
        handed_over[0],
        args[handed_over[0]],
        declared[0],
        parameters[declared[0]]
    );

    // AND THE WHOLE OF IT, AT THE CALL SITE. Everything above establishes WHICH
    // argument is meant to be the body and that it lines up with the parameter.
    // Nothing above asks what was DONE to it — and the truncation rule this file
    // spends three magnitudes of fixtures closing, and then closes a second time
    // inside the evaluator, was applied only to the text between `judge`'s own
    // parentheses. The identical clamp written one line up, here, was fully
    // open. Review measured it against a working reference gate: changing this
    // argument from `body,` to `&body[..body.len().min(4000)],` gave
    // `test result: ok. 41 passed; 0 failed`, because `root_ident` of that
    // expression is still `body`, the position check still lines up, and `judge`
    // stays perfectly correct. Every author of a pull request body over four
    // kilobytes is then told they wrote no acceptance bar — the fabricated
    // accusation this file names as equal in severity to a false green.
    let handed = &args[handed_over[0]];
    if let Some(defect) = truncated_argument(handed) {
        panic!(
            "the review pipeline hands the evaluator less than the change's body: it \
             passes {handed:?} at the body position — {defect}. A bar written at the \
             far end of a long pull request is still the artifact, and `judge` stays \
             perfectly correct while a clamp here tells every author of a long, \
             careful change that they wrote no acceptance bar. Pass the body whole; if \
             something downstream needs a shorter string, shorten it downstream"
        );
    }

    // AND NOTHING CLAMPED IT ON THE WAY IN, here either. The rule above reads
    // the literal text of the argument, so a `let` on the line above defeats it,
    // and at a call site the clamp need not even shadow the parameter to do it:
    //
    //     let body_excerpt = &body[..2000];
    //     … evaluate_pre_merge_gates(…, body_excerpt, …)
    //
    // `root_ident("body_excerpt")` contains "body", so the argument is the one
    // every rule above picks up, and it reads as a plain path. See
    // `shadowed_argument`, which follows the value under both names.
    let handed_root = root_ident(handed);
    if let Some(defect) = shadowed_argument(&src, &handed_root) {
        panic!(
            "the review pipeline hands the evaluator less than the change's body: \
             {defect}. The argument {handed:?} reads as a plain path only because the \
             clamp was written above the call. A bar written at the far end of a long \
             pull request is still the artifact"
        );
    }

    // AND NOTHING HERE WRITES THE VERDICT AFTERWARDS. `assignments_to` was run
    // over the evaluator in the test above and over nothing else, so the same
    // fail-open written in THIS file — `cert_report.product_bar_status =
    // GateStatus::Warning(..)`, one line after the call and before the report is
    // sealed — was invisible to every guard in the suite. The scan has to cover
    // the file that holds the report between the measurement and the seal, not
    // only the file that computes it.
    let overwrites = assignments_to(&src, "product_bar_status");
    assert!(
        overwrites.is_empty(),
        "the review pipeline writes over the Product seat's measurement after the \
         evaluator computed it: {overwrites:?}. Between `evaluate_pre_merge_gates` and \
         the sealed report, nothing may substitute a status for the one the gate \
         measured — Warning, NotMeasured and Passed are all `is_acceptable()`, so any \
         of them certifies a change whose author wrote no acceptance bar, with every \
         behavioural test in this file green. If a change can legitimately pass, say \
         so in `judge`"
    );
}

#[test]
fn every_caller_of_the_review_pipeline_hands_it_the_change_body() {
    assert_the_wiring_parsers_read_a_real_wiring();

    // THE VALUE CHAIN, ONE HOP FURTHER UP. The whole argument for forcing the
    // body through a PARAMETER rather than a field on `PrDiffContext` is that "a
    // parameter is not a grep: it can hold only what the caller passed" — see
    // `evaluator_body_parameters`. That argument applies with exactly the same
    // force to `execute_pr_review`'s own `body: &str`, and there it is not
    // hypothetical: review found a caller already passing the empty-string
    // literal.
    //
    //     // src/cli/server.rs — the outage-recovery startup sweep, which
    //     // dispatches a full review and certification for every uncertified
    //     // pull request after a restart
    //     let _ = crate::webhook::pipelines::review::execute_pr_review(
    //         &task_state, &repo, pr.number, &pr.title,
    //         "",
    //         "main", "HEAD", &pr.head_sha, false,
    //     )
    //
    // That is live code on this branch, and wiring the Product gate is what
    // arms it. Every pull request certified through that path would be handed
    // `product_bar_status = Failed("…no written problem statement and no
    // done-when acceptance bar…")` no matter what its author actually wrote,
    // `is_certified_ready` would go false, and the merge queue would block —
    // the fabricated accusation at 100% incidence on that path, which this file
    // names as equal in severity to a false green. Nothing else in this suite
    // reads `src/cli/server.rs`, so the whole file stays green while it ships.
    //
    // The rule is stated as an EFFECT and not as a spelling, because the four
    // correct call sites do not agree on one. Three of them reach the body
    // through a field (`&meta.body.unwrap_or_default()`), one through a local
    // (`&pr_body`), and a fifth could reasonably do something else again. What
    // every correct one has in common is that the argument is derived from the
    // pull request's own body — some identifier in it says so — that it is not a
    // literal, and that nothing along the way drops any of the text.
    const DEFINITION: &str = "pub async fn execute_pr_review(";
    const CALL: &str = "execute_pr_review(";

    let review_src = without_line_comments(&source("src/webhook/pipelines/review.rs"));
    let signature = signature_of(&review_src, DEFINITION)
        .expect("src/webhook/pipelines/review.rs declares execute_pr_review");
    let parameters = signature_parameters(&signature);
    assert!(
        parameters.len() > 4,
        "this test reads one parameter per line and found only {parameters:?} on \
         execute_pr_review. Fix the test to parse the new shape rather than let it \
         mis-parse — a wiring guard that cannot read the wiring is worse than none. \
         Signature: {signature:?}"
    );

    let declared: Vec<usize> = parameters
        .iter()
        .enumerate()
        .filter(|(_, p)| p.contains("body"))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        declared.len(),
        1,
        "execute_pr_review must declare exactly one parameter whose name says body, or \
         this scan cannot tell which argument position each caller is supposed to be \
         filling. Parameters: {parameters:?}"
    );
    let position = declared[0];

    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_rust_sources(&manifest.join("src"), &mut files);
    files.sort();

    let mut sites: Vec<(String, usize, String)> = Vec::new();
    // The file text up to each call, aligned with `sites` by index. Kept beside
    // the sites rather than in them because `sites` is printed in every failure
    // message and a whole source file is not a thing a reader can read.
    let mut prefixes: Vec<String> = Vec::new();
    for path in &files {
        let rel = path
            .strip_prefix(&manifest)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = without_line_comments(
            &std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{rel}: {e}")),
        );
        for (at, _) in text.match_indices(CALL) {
            // The declaration is not a call site. A `use` line carries no
            // parenthesis and never matches at all.
            if text[..at].trim_end().ends_with("fn") {
                continue;
            }
            let line = text[..at].lines().count();
            let args = one_argument_per_line(&text, at, CALL)
                .unwrap_or_else(|why| panic!("{rel}:{line}: {why}"));
            assert_eq!(
                args.len(),
                parameters.len(),
                "{rel}:{line}: this call passes {} arguments to an execute_pr_review \
                 that declares {} parameters. Fix the test to parse the new shape \
                 rather than let it compare positions that do not line up. Arguments: \
                 {args:?}. Parameters: {parameters:?}",
                args.len(),
                parameters.len()
            );
            sites.push((rel.clone(), line, args[position].clone()));
            prefixes.push(text[..at].to_string());
        }
    }

    // Non-vacuity. A scan that finds no call sites reports every caller correct
    // without having read one, which is the "answers without looking" failure
    // this file spends a page on. The floor is a floor and not an equality
    // because callers may legitimately be added or merged; if the pipeline
    // really does end up with fewer than three, lower it deliberately rather
    // than let the guard go quiet.
    assert!(
        sites.len() >= 3,
        "this scan found only {} call sites of execute_pr_review under src/, which is \
         fewer than this repository has. A call-site scan that finds nothing answers \
         \"every caller is correct\" without looking at one. Found: {sites:?}",
        sites.len()
    );

    for (index, (rel, line, arg)) in sites.iter().enumerate() {
        assert!(
            !arg.contains('"'),
            "{rel}:{line} hands execute_pr_review the string literal {arg:?} where the \
             pull request body belongs. Once the Product gate is wired, every change \
             certified through this path is told it carries no written problem \
             statement and no done-when acceptance bar, whatever its author actually \
             wrote — a fabricated accusation at 100% incidence on this path, which \
             blocks the merge queue for changes that did nothing wrong. Fetch the \
             body (the sibling call sites do it with \
             `&meta.body.unwrap_or_default()` off the metadata this path can also \
             fetch) and pass it. A literal cannot hold what the author wrote. All call \
             sites: {sites:?}"
        );
        assert!(
            names_the_change_body(arg),
            "{rel}:{line} hands execute_pr_review {arg:?} at the body position, and no \
             identifier in it names the pull request's body. Rust binds arguments to \
             parameters BY POSITION and the neighbouring parameters take the same \
             type, so passing the title, the base branch or the head sha here compiles \
             — and the Product seat then measures that string instead of the change \
             under review, reporting both artifacts missing on every pull request \
             through this path. Parameter at {position}: {:?}. All call sites: \
             {sites:?}",
            parameters[position]
        );
        if let Some(defect) = truncating_token_in(arg) {
            panic!(
                "{rel}:{line} hands execute_pr_review less than the change's body: it \
                 passes {arg:?} — {defect}. A bar written at the far end of a long \
                 pull request is still the artifact, so a clamp here tells every \
                 author of a long, careful change that they wrote no acceptance bar, \
                 while `judge` stays perfectly correct and every behavioural test in \
                 this file stays green. All call sites: {sites:?}"
            );
        }

        // AND THE VALUE THE LOCAL WAS BOUND TO, which is the same rule
        // `shadowed_argument` applies one level down and which this hop went
        // without. Every assertion above reads the argument's SPELLING, so the
        // cheapest way to make this test green at a call site that owns no body
        // is `let pr_body = String::new();` one line above it: the argument then
        // reads as a plain local naming the body, and the gate is still handed
        // the empty string on every pull request through that path. A name is
        // not a value; the binding is where the value comes from.
        let root = root_ident(arg);
        if names_the_change_body(&root)
            && let Some(defect) = caller_binding_defect(&prefixes[index], &root)
        {
            panic!(
                "{rel}:{line} hands execute_pr_review {arg:?} at the body position, and \
                 the local it is rooted at does not hold the change's body: {defect}. \
                 Once the Product gate is wired, every change certified through this \
                 path is told it carries no written problem statement and no done-when \
                 acceptance bar, whatever its author actually wrote — the fabricated \
                 accusation at 100% incidence on this path, with every other test in \
                 this file green. Fetch the body (the sibling call sites do it with \
                 `&meta.body.unwrap_or_default()` off the metadata this path can also \
                 fetch) and bind THAT. All call sites: {sites:?}"
            );
        }
    }
}

/// Exercises the hand-rolled parsers the wiring guards depend on.
///
/// Not a `#[test]` of its own, deliberately. Nothing here touches the gate, so
/// standing alone it would be green from the moment it was written, and a test
/// that has never been observed failing publishes assurance it has not earned —
/// the same reason the static half of
/// `the_verdict_depends_on_nothing_but_the_change_it_was_handed` lives inside
/// that test. It runs first inside the wiring tests instead, which are red on
/// the absent wiring.
#[track_caller]
fn assert_the_wiring_parsers_read_a_real_wiring() {
    // The wiring guards are the only thing standing between a named Product
    // gate and one that blocks nothing, and they are hand-rolled parsers. A
    // parser that misreads a correct implementation and then reports it as the
    // exact defect it does not have is worse than no guard at all — that is
    // this file's own standard, applied to this file.
    //
    // `let x: GateStatus = judge(..)` is the case that motivated this: the
    // previous revision's identifier check rejected any name containing a
    // character outside [alnum_], which the type annotation supplies, so a
    // correct annotated wiring produced no binding and failed the test.
    for (head, want) in [
        (
            "        let product_bar_status = ",
            Some("product_bar_status"),
        ),
        (
            "        let product_bar_status: GateStatus = ",
            Some("product_bar_status"),
        ),
        (
            "        let mut product_bar_status = ",
            Some("product_bar_status"),
        ),
        (
            "        let mut product_bar_status: GateStatus = ",
            Some("product_bar_status"),
        ),
        (
            "        let product_bar_status = super::",
            Some("product_bar_status"),
        ),
        // Not an initialiser: the statement the `let` opened has already ended.
        (
            "        let earlier = compute();\n        report.field = ",
            None,
        ),
        // No `let` at all: a bare call in expression position.
        ("        gates.push(", None),
    ] {
        assert_eq!(
            let_binding_before(head).as_deref(),
            want,
            "let_binding_before({head:?}) misread the binding"
        );
    }

    // The comment stripper must not treat a URL inside a string literal as the
    // start of a comment; truncating there would blind every scan above.
    for (line, want) in [
        (
            "let x = judge(pr_body); // wire it up",
            "let x = judge(pr_body); ",
        ),
        ("// let x = judge(pr_body);", ""),
        (
            r#"let m = "see https://example.invalid/x"; let x = judge(pr_body);"#,
            r#"let m = "see https://example.invalid/x"; let x = judge(pr_body);"#,
        ),
        (
            r#"let m = "a \" // b"; judge(pr_body)"#,
            r#"let m = "a \" // b"; judge(pr_body)"#,
        ),
    ] {
        assert_eq!(
            strip_line_comment(line),
            want,
            "strip_line_comment({line:?}) cut the line in the wrong place"
        );
    }

    // The call finder must see a `judge` reached through an import, and must
    // not see a `judge` belonging to some other module.
    let imported = "use super::product_bar::judge;\n\
                    let product_bar_status = judge(pr_body);\n";
    let found = product_bar_judge_calls(imported);
    assert_eq!(
        found.len(),
        1,
        "an imported judge call is as correct as a qualified one and must be found \
         exactly once; got {found:?}"
    );
    assert_eq!(
        (found[0].binding.as_deref(), found[0].args.as_str()),
        (Some("product_bar_status"), "pr_body"),
        "the call finder misread the imported call: {found:?}"
    );

    let foreign = "let shape_status = shape::judge(outcome);\n";
    assert!(
        product_bar_judge_calls(foreign).is_empty(),
        "another module's judge must not be mistaken for the Product seat's"
    );

    let qualified = "let product_bar_status = super::product_bar::judge(pr_body);\n";
    let found = product_bar_judge_calls(qualified);
    assert_eq!(
        found.len(),
        1,
        "a qualified call must be found exactly once, not twice; got {found:?}"
    );
    assert_eq!(
        (found[0].binding.as_deref(), found[0].args.as_str()),
        (Some("product_bar_status"), "pr_body"),
        "the call finder misread the qualified call: {found:?}"
    );

    // THE SIGNATURE READER, which is the whole of route one: the guards ask
    // whether the evaluator declares a parameter whose name says body, and then
    // whether the judge call is rooted at that parameter. A reader that finds
    // nothing turns both into an accusation with no evidence behind it, and one
    // that finds a name where there is none turns them into a no-op.
    let sig = "    pub fn evaluate_pre_merge_gates(\n\
               \x20       &self,\n\
               \x20       diff_ctx: &PrDiffContext,\n\
               \x20       pr_title: &str,\n\
               \x20       pr_body: &str,\n\
               \x20       doc_report: &DocGuardReport,\n";
    assert_eq!(
        signature_parameters(sig),
        vec!["diff_ctx", "pr_title", "pr_body", "doc_report"],
        "signature_parameters misread the evaluator's parameter list; `&self` carries \
         no colon and the `fn` line is not a parameter"
    );
    assert!(
        signature_parameters("    pub fn f(&self) -> Result<()> {\n").is_empty(),
        "signature_parameters must find nothing in a signature that declares no \
         parameters, rather than invent one"
    );

    // And it must be pointed at the real function, not at a file that no longer
    // declares it. `evaluator_body_parameters()` coming back empty is how the
    // guards conclude "the evaluator was never handed the body", so a renamed
    // or moved evaluator would make that a permanent yes-it-is-broken answer
    // given without looking.
    let real_signature = evaluator_signature(&without_line_comments(&source(
        "src/pre_merge_guard/evaluator.rs",
    )))
    .expect("src/pre_merge_guard/evaluator.rs declares evaluate_pre_merge_gates");
    let real_parameters = signature_parameters(&real_signature);
    for parameter in ["diff_ctx", "doc_report", "coverage_report"] {
        assert!(
            real_parameters.iter().any(|p| p == parameter),
            "src/pre_merge_guard/evaluator.rs no longer declares \
             evaluate_pre_merge_gates with a {parameter:?} parameter, one per line, so \
             this file is not reading the real signature any more and the wiring \
             guards answer without looking. Found: {real_parameters:?}"
        );
    }

    // THE ROOT READER. `&pr_body` and `pr_body.as_str()` are the body;
    // `&diff_ctx.pr_body` is a field on a struct, which is the route this file
    // closes, and `&doc_report` is a neighbour's report. All four contain the
    // substring the previous revision keyed on.
    for (expr, want) in [
        ("pr_body", "pr_body"),
        ("&pr_body", "pr_body"),
        ("&mut pr_body", "pr_body"),
        ("& pr_body", "pr_body"),
        ("pr_body.as_str()", "pr_body"),
        ("pr_body.trim()", "pr_body"),
        ("&diff_ctx.pr_body", "diff_ctx"),
        ("&review_resp.verdict", "review_resp"),
        ("\"\"", ""),
    ] {
        assert_eq!(
            root_ident(expr),
            want,
            "root_ident({expr:?}) misread what the expression is rooted at. Reading \
             `&diff_ctx.pr_body` as `pr_body` reopens the route where a field the \
             constructor never populates certifies the wiring while the gate reads \
             the empty string for every pull request"
        );
    }

    // The binding reader, which is what lets the pipeline satisfy its half by
    // building the diff context from the body rather than passing it separately.
    let pipeline = "    let repo_dir = state.git_mgr.ensure_repo_cloned(repo).await?;\n\
                     \x20   let diff_ctx = state\n\
                     \x20       .git_mgr\n\
                     \x20       .prepare_pr_diff(repo, pr_number, base_sha, head_sha, body)\n\
                     \x20       .await?;\n";
    let bound = binding_statement(pipeline, "diff_ctx").expect("diff_ctx is bound by a let");
    assert!(
        bound.contains("prepare_pr_diff") && bound.contains("body") && bound.ends_with(';'),
        "binding_statement read the wrong span for diff_ctx: {bound:?}"
    );
    assert!(
        !binding_statement(pipeline, "repo_dir")
            .expect("repo_dir is bound by a let")
            .contains("prepare_pr_diff"),
        "binding_statement must stop at the semicolon that closes the statement, or \
         every binding looks like it was built from every later value"
    );
    assert!(
        binding_statement(pipeline, "cert_report").is_none(),
        "binding_statement must find nothing for an identifier no let binds"
    );

    // THE RESIDUE READER, which is what the four-entry denylist became. A
    // conditional that keeps a literal status in reserve is one shape an
    // engineer writes; a softening WRAPPER around the call is another, and the
    // denylist could not see it — `soften(product_bar::judge(pr_body))` leaves
    // `soften( )` behind, which contains none of `GateStatus::`, `if `, `match `
    // or `unwrap_or`, and `post_processing_after` reads only what follows the
    // closing paren. Review measured that on a working reference gate and got
    // all forty-one tests green with every change certifying.
    //
    // BOTH SIDES, over both the forms a value reaches this rule in — a `let`
    // statement and a bare struct-literal initialiser — and over every spelling
    // of the path this file promises an implementer, because a rule that finds a
    // residue in `crate::pre_merge_guard::product_bar::judge(pr_body)` accuses
    // the most explicit correct wiring there is of the defect it does not have.
    let straight = "let product_bar_status = product_bar::judge(pr_body);\n";
    let annotated = "let product_bar_status: GateStatus = product_bar::judge(&pr_body);\n";
    for (statement, clean) in [
        (
            "let product_bar_status = product_bar::judge(pr_body);",
            true,
        ),
        (
            "let mut product_bar_status = product_bar::judge(pr_body);",
            true,
        ),
        (
            "let product_bar_status: GateStatus = product_bar::judge(&pr_body);",
            true,
        ),
        (
            "let product_bar_status = super::product_bar::judge(pr_body);",
            true,
        ),
        (
            "let product_bar_status = crate::pre_merge_guard::product_bar::judge(pr_body);",
            true,
        ),
        ("product_bar::judge(pr_body)", true),
        ("crate::pre_merge_guard::product_bar::judge(&pr_body)", true),
        // The WRAPPER, which is the whole reason this stopped being a denylist,
        // in both the shapes it is written in and in both statement forms.
        ("soften(product_bar::judge(pr_body))", false),
        ("GateStatus::from(product_bar::judge(pr_body))", false),
        (
            "soften(crate::pre_merge_guard::product_bar::judge(pr_body))",
            false,
        ),
        (
            "let product_bar_status = soften(product_bar::judge(pr_body));",
            false,
        ),
        // The chaining form, which `post_processing_after` also catches: two
        // rules covering one defect is not a problem, one covering none is.
        (
            "let product_bar_status = product_bar::judge(pr_body).softened();",
            false,
        ),
        // The conditional that keeps a literal status in reserve.
        (
            "let product_bar_status = if pr_body.trim().is_empty() { GateStatus::NotMeasured { gate_id: \"product_bar_status\".to_string(), reason: \"no body\".to_string() } } else { product_bar::judge(pr_body) };",
            false,
        ),
        (
            "let product_bar_status = match pr_body { \"\" => GateStatus::Passed, b => product_bar::judge(b) };",
            false,
        ),
    ] {
        let residue = residue_beyond_the_measurement(statement, statement);
        assert_eq!(
            residue.is_empty(),
            clean,
            "residue_beyond_the_measurement({statement:?}) misread the statement; \
             residue {residue:?}. A rule that finds something left over in a correct \
             wiring accuses it of the defect it does not have; one that finds nothing \
             left over in `soften(judge(..))` lets a mapping from Failed onto the \
             acceptable Warning certify every change"
        );
    }

    // THE REASSIGNMENT FORM OF THE SAME FAIL-OPEN, which is one line away from
    // the conditional above and invisible to every other check in these guards:
    // `binding_statement` stops at the `;` that closes the `let` (its own parser
    // test three blocks up requires that), and `let_binding_before` strips
    // `mut ` on purpose, so this shape binds the right name, calls judge over
    // the body, leaves an empty residue and hands the struct the shorthand.
    let reassigned = "    let mut product_bar_status = product_bar::judge(pr_body);\n\
                      \x20   if pr_body.trim().is_empty() {\n\
                      \x20       product_bar_status = GateStatus::NotMeasured { gate_id: \"product_bar_status\".to_string(), reason: \"the pull request has no body\".to_string() };\n\
                      \x20   }\n";
    assert!(
        residue_beyond_the_measurement(
            reassigned,
            &binding_statement(reassigned, "product_bar_status").expect("bound by a let"),
        )
        .is_empty(),
        "sanity, and the reason this check exists: the initialiser reader sees NOTHING \
         wrong with the reassignment fail-open, because the statement it reads ends at \
         the semicolon"
    );
    assert_eq!(
        assignments_to(reassigned, "product_bar_status").len(),
        1,
        "the reassignment that substitutes NotMeasured for the measurement must be \
         seen; got {:?}",
        assignments_to(reassigned, "product_bar_status")
    );

    // THE SAME FAIL-OPEN WRITTEN ONTO THE FIELD RATHER THAN ONTO THE BINDING,
    // which the previous revision could not see at all: it matched only when
    // the whole left-hand side trimmed to the bare identifier. Review verified
    // the hole against a working reference gate — `report.product_bar_status =
    // GateStatus::Passed;` before `report.seal();` was rustfmt-clean, passed
    // every test in this file and every other test in the repository, and
    // certified the Product seat for every change opened with an empty body.
    // report.rs already writes exactly this shape for a neighbouring gate.
    for onto_the_field in [
        "    report.product_bar_status = GateStatus::Passed;\n",
        "    r.product_bar_status = GateStatus::Warning(\"soft launch\".to_string());\n",
        "        self.product_bar_status = neighbour.clone();\n",
    ] {
        assert_eq!(
            assignments_to(onto_the_field, "product_bar_status").len(),
            1,
            "a write onto the report's own field is the same fail-open as a write onto \
             the binding, and the receiver does not change that: {onto_the_field:?} was \
             read as {:?}",
            assignments_to(onto_the_field, "product_bar_status")
        );
    }

    // And the mirror, so the widened check cannot fail a correct wiring for its
    // spelling. A plain `let`, an annotated `let`, a `let mut` that is never
    // written over, a struct-literal field in both its shorthand and its
    // labelled form, and a comparison on any receiver are all correct, and none
    // of them is an overwrite. Widening the guard without these is a licence to
    // fabricate an accusation against the one wiring this file asks for.
    for clean in [
        straight,
        annotated,
        "let mut product_bar_status = product_bar::judge(pr_body);\n",
        "if product_bar_status == GateStatus::Passed {\n",
        "    product_bar_status != GateStatus::Passed\n",
        "let product_bar_status: GateStatus = product_bar::judge(pr_body);\n",
        "            product_bar_status,\n",
        "            product_bar_status: product_bar::judge(pr_body),\n",
        "            product_bar_status: product_bar::judge(&pr_body),\n",
        "if report.product_bar_status == GateStatus::Passed {\n",
        "    report.product_bar_status != GateStatus::Passed\n",
        "    let carried = report.product_bar_status.clone();\n",
    ] {
        assert_eq!(
            assignments_to(clean, "product_bar_status"),
            Vec::<String>::new(),
            "assignments_to saw a write in {clean:?}, which introduces, initialises or \
             merely reads the field. A guard that misreads a correct wiring and reports \
             it as the defect it does not have is worse than no guard"
        );
    }

    // THE ARGUMENT RULE. `unmeasured_alternatives_in` cuts the whole judge call
    // out before it looks at the statement, so nothing else in these guards can
    // see what was done to the call's INPUT — and truncating the input is how
    // `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact` gets
    // reinstated one layer up, with `judge` left perfectly correct. Both sides,
    // because a rule that rejected `&pr_body` or `pr_body.as_str()` would fail
    // the very wiring this file asks for. A field access stays on the clean
    // side too: `truncated_argument` judges whether any TEXT was dropped, and
    // which identifier the path is rooted at is `root_ident`'s question, asked
    // separately by the judge-call test.
    for (arg, truncating) in [
        ("pr_body", false),
        ("&pr_body", false),
        ("&self.pr_body", false),
        ("self.pr_body", false),
        ("pr_body.as_str()", false),
        ("&pr_body.clone()", false),
        ("pr_body.trim()", false),
        ("&pr_body[..450]", true),
        ("&pr_body[..2000.min(pr_body.len())]", true),
        ("&pr_body.lines().take(40).collect::<String>()", true),
        ("pr_body.chars().take(2000).collect::<String>()", true),
        ("pr_body.get(..1500).unwrap_or(pr_body)", true),
        ("&mut truncate_body(pr_body)", true),
    ] {
        assert_eq!(
            truncated_argument(arg).is_some(),
            truncating,
            "truncated_argument({arg:?}) misjudged the argument. A rule that calls a \
             plain path a truncation accuses a correct wiring of the one defect it does \
             not have; one that calls a slice a plain path lets \
             `judge(&body[..2000])` through, and every author of a long, careful pull \
             request is then told they wrote no acceptance bar"
        );
    }

    // THE SHADOW RULE, which is the argument rule followed one line up. A `let`
    // that rebinds the parameter from itself is the same truncation as an
    // inline slice, written where `truncated_argument` cannot see it — and
    // BOTH SIDES are exercised here, because a rule that read a harmless
    // rebinding, or an unrelated `let` of the same name in the file's own test
    // module, as a clamp would be the fabricated accusation this file forbids.
    for (wiring, root, shadowed) in [
        (
            "        let product_bar_status = product_bar::judge(pr_body);\n",
            "pr_body",
            false,
        ),
        (
            "        let pr_body = pr_body.trim();\n\
             \x20       let product_bar_status = product_bar::judge(pr_body);\n",
            "pr_body",
            false,
        ),
        (
            "        let pr_body = pr_body.as_str();\n\
             \x20       let product_bar_status = product_bar::judge(pr_body);\n",
            "pr_body",
            false,
        ),
        // Not a shadow of the parameter at all: an unrelated binding, which is
        // what a fixture in the file's own `#[cfg(test)]` module looks like.
        (
            "        let pr_body = \"## Problem\\n\\nreal\\n\";\n",
            "pr_body",
            false,
        ),
        (
            "        let pr_body = &pr_body[..4000.min(pr_body.len())];\n\
             \x20       let product_bar_status = product_bar::judge(pr_body);\n",
            "pr_body",
            true,
        ),
        (
            "        let pr_body: String = pr_body.lines().take(50).collect();\n\
             \x20       let product_bar_status = product_bar::judge(&pr_body);\n",
            "pr_body",
            true,
        ),
        (
            "        let body = truncate_body(body);\n\
             \x20       let product_bar_status = product_bar::judge(body);\n",
            "body",
            true,
        ),
        // The clamp written under a harmless rebinding, which a guard reading
        // only the FIRST `let` of that name would walk straight past.
        (
            "        let pr_body = pr_body.trim();\n\
             \x20       let pr_body = &pr_body[..4000.min(pr_body.len())];\n\
             \x20       let product_bar_status = product_bar::judge(pr_body);\n",
            "pr_body",
            true,
        ),
        // THE CLAMP UNDER A NEW NAME, which shadows nothing and which the
        // rebinding rule alone cannot see: the initialiser never mentions
        // `body_excerpt`, so the "is this a shadow of itself" filter skipped it,
        // while `root_ident("body_excerpt")` still contains "body" and every
        // other rule reads the argument as a plain path.
        (
            "        let body_excerpt = &body[..2000];\n\
             \x20       let cert = evaluate_pre_merge_gates(body_excerpt);\n",
            "body_excerpt",
            true,
        ),
        (
            "        let pr_body: String = meta.body.lines().take(50).collect();\n",
            "pr_body",
            true,
        ),
        // And the mirror the new arm must not fail: an ordinary, correct way to
        // spell "the body this pull request carries", which drops none of the
        // text and is not a plain path. The strict rule would reject it, so the
        // new arm applies the EFFECT rule only.
        (
            "        let pr_body = meta.body.unwrap_or_default();\n",
            "pr_body",
            false,
        ),
        (
            "        let pr_body = pr.body.clone().unwrap_or_default();\n",
            "pr_body",
            false,
        ),
        // A `#[cfg(test)]` fixture, whose markdown carries both a bracket and
        // the word this rule keys on. Blanking string literals is what keeps it
        // off the accusation list.
        (
            "        let body = \"## Done when\\n\\n- [ ] the body is under 5ms\\n\";\n",
            "body",
            false,
        ),
    ] {
        assert_eq!(
            shadowed_argument(wiring, root).is_some(),
            shadowed,
            "shadowed_argument({wiring:?}, {root:?}) misjudged the rebinding. A rule \
             that calls a harmless one a truncation accuses a correct wiring of the \
             defect it does not have; one that misses a real clamp reinstates \
             `judge(&body[..N])` one line above the call it is pointed at"
        );
    }

    // And the identifier reader the shadow rule rests on: `pr_body` inside
    // `pr_body_len` is not the same value, and treating it as one would turn an
    // unrelated local into an accusation.
    for (expr, name, mentioned) in [
        ("&pr_body[..4000]", "pr_body", true),
        ("pr_body.trim()", "pr_body", true),
        ("pr_body_len.min(4000)", "pr_body", false),
        ("my_pr_body", "pr_body", false),
        ("\"## Problem\"", "pr_body", false),
    ] {
        assert_eq!(
            mentions_ident(expr, name),
            mentioned,
            "mentions_ident({expr:?}, {name:?}) misread whether the initialiser is \
             derived from the parameter it shadows"
        );
    }

    // THE EFFECT HALF OF THE ARGUMENT RULE ON ITS OWN, which is the only half
    // that can be applied one hop further up the value chain, where the caller
    // legitimately writes something that is not a plain path. Both sides: a rule
    // that called `&meta.body.unwrap_or_default()` a truncation would accuse
    // three correct call sites in this repository of the defect they do not
    // have, and a rule that called `&body[..4000]` whole lets the clamp through.
    for (arg, truncating) in [
        ("body", false),
        ("&pr_body", false),
        ("&meta.body.unwrap_or_default()", false),
        ("&pr_meta.body.unwrap_or_default()", false),
        ("&pr.body.clone().unwrap_or_default()", false),
        ("&body[..body.len().min(4000)]", true),
        ("&body[..4000]", true),
        ("body_excerpt.lines().take(50).collect::<String>()", true),
        (
            "&meta.body.unwrap_or_default().chars().take(2000).collect::<String>()",
            true,
        ),
    ] {
        assert_eq!(
            truncating_token_in(arg).is_some(),
            truncating,
            "truncating_token_in({arg:?}) misjudged the argument"
        );
    }

    // THE CALLER-HOP BINDING RULE, which is the shadow rule applied one hop
    // further up: at a call site the argument's spelling says nothing about the
    // value, and `let pr_body = String::new();` above the call satisfies every
    // spelling rule while handing the gate the empty string. BOTH SIDES, and the
    // clean half is not decoration — it is the real binding at
    // src/webhook/webhook_handlers.rs, and a rule that reported it would
    // fabricate an accusation against the one correct call site in this
    // repository that reaches the body through a local.
    for (src_text, root, defective) in [
        (
            "        let pr_body = pr.body.unwrap_or_default();\n",
            "pr_body",
            false,
        ),
        (
            "        let pr_body = pr.body.clone().unwrap_or_default();\n",
            "pr_body",
            false,
        ),
        (
            "        let pr_body = meta.body.unwrap_or_default();\n",
            "pr_body",
            false,
        ),
        (
            "        let pr_body = fetch_pr_body(&state, repo, pr.number).await?;\n",
            "pr_body",
            false,
        ),
        // The root is bound by no `let` here at all: it is a parameter of the
        // enclosing function, and this rule stops at one hop.
        ("        let repo_dir = clone(repo);\n", "pr_body", false),
        ("        let pr_body = String::new();\n", "pr_body", true),
        ("        let pr_body = String :: new ();\n", "pr_body", true),
        (
            "        let pr_body = Default::default();\n",
            "pr_body",
            true,
        ),
        ("        let pr_body = pr.title.clone();\n", "pr_body", true),
        ("        let pr_body = \"\".to_string();\n", "pr_body", true),
        (
            "        let pr_body = pr.body.unwrap_or_default()[..2000].to_string();\n",
            "pr_body",
            true,
        ),
        // The last binding before the call is the one in effect, so a harmless
        // one above a clamp does not clear it.
        (
            "        let pr_body = pr.body.unwrap_or_default();\n\
             \x20       let pr_body = String::new();\n",
            "pr_body",
            true,
        ),
    ] {
        assert_eq!(
            caller_binding_defect(src_text, root).is_some(),
            defective,
            "caller_binding_defect({src_text:?}, {root:?}) misjudged what the local was \
             bound to. A rule that reads `pr.body.unwrap_or_default()` as a defect \
             accuses the one correct call site that reaches the body through a local; \
             one that reads `String::new()` as the body lets the empty string reach \
             the gate under the body's own name, and every pull request through that \
             path is told it wrote neither artifact"
        );
    }

    // THE BODY-NAMING RULE, which is how a call site says it is passing the
    // change's body rather than something else in scope. Every identifier, not
    // just the root: at three of this repository's five call sites the body is
    // reached through a field on the metadata struct, so a root-only rule would
    // report `meta` and fabricate an accusation against correct code.
    for (expr, names_body) in [
        ("body", true),
        ("&pr_body", true),
        ("&meta.body.unwrap_or_default()", true),
        ("&pr_meta.body.unwrap_or_default()", true),
        ("body_excerpt", true),
        ("&pr.title", false),
        ("&pr.head_sha", false),
        ("&review_resp.verdict", false),
        ("&base_branch", false),
        ("pr.number", false),
        ("false", false),
        ("\"\"", false),
        ("\"main\"", false),
    ] {
        assert_eq!(
            names_the_change_body(expr),
            names_body,
            "names_the_change_body({expr:?}) misread whether the call site is passing \
             the change's body"
        );
    }

    // THE ARGUMENT PARSER the call-site scans rest on, on both sides. It reads
    // one argument per line, and a call written any other way has to make it say
    // so: a scan that quietly finds no arguments reports every call site correct
    // without looking at one.
    let multiline = "    execute_pr_review(\n\
                     \x20       state,\n\
                     \x20       repo,\n\
                     \x20       &meta.body.unwrap_or_default(),\n\
                     \x20       true,\n\
                     \x20   )\n\
                     \x20   .await\n";
    assert_eq!(
        one_argument_per_line(multiline, 4, "execute_pr_review("),
        Ok(vec![
            "state".to_string(),
            "repo".to_string(),
            "&meta.body.unwrap_or_default()".to_string(),
            "true".to_string(),
        ]),
        "one_argument_per_line misread a call written one argument per line"
    );
    let collapsed = "    execute_pr_review(state, repo, body, true).await\n";
    assert!(
        one_argument_per_line(collapsed, 4, "execute_pr_review(").is_err(),
        "a collapsed call must be reported as unreadable rather than parsed as though \
         it carried no arguments at all"
    );

    // AND THE SIGNATURE READER POINTED AT THE REAL PIPELINE ENTRY POINT, for the
    // same reason it is pointed at the real evaluator above: the call-site scan
    // decides WHICH argument position is the body from this signature, so a
    // reader that finds nothing there would compare position zero on every call
    // site and answer without looking.
    let pipeline_signature = signature_of(
        &without_line_comments(&source("src/webhook/pipelines/review.rs")),
        "pub async fn execute_pr_review(",
    )
    .expect("src/webhook/pipelines/review.rs declares execute_pr_review");
    let pipeline_parameters = signature_parameters(&pipeline_signature);
    for parameter in ["state", "repo", "pr_number", "title", "body"] {
        assert!(
            pipeline_parameters.iter().any(|p| p == parameter),
            "src/webhook/pipelines/review.rs no longer declares execute_pr_review with \
             a {parameter:?} parameter, one per line, so the call-site scan is not \
             reading the real signature any more. Found: {pipeline_parameters:?}"
        );
    }

    // THE CHAINING RULE. Cutting the call out also cuts out everything the
    // residue could have caught about what is done to the RESULT, and
    // `.softened()` mapping Failed onto Warning certifies every change while
    // every behavioural test in this file stays green.
    for (tail, chained) in [
        (";", None),
        (",", None),
        (")", None),
        ("  ;\n", None),
        (",\n            unmeasured_gates: Vec::new(),", None),
        (".softened();", Some('.')),
        (".or(GateStatus::Passed),", Some('.')),
        (".map(soften);", Some('.')),
    ] {
        assert_eq!(
            post_processing_after(tail),
            chained,
            "post_processing_after({tail:?}) misjudged what the source does with the \
             verdict the instant the call closes"
        );
    }

    // And both new rules end to end, over whole wirings — the three correct
    // spellings this file promises an implementer, and the two fail-opens the
    // rules exist for.
    for (wiring, clean) in [
        (
            "let product_bar_status = product_bar::judge(pr_body);\n",
            true,
        ),
        (
            "let product_bar_status = product_bar::judge(&pr_body);\n",
            true,
        ),
        (
            "let product_bar_status: GateStatus = product_bar::judge(pr_body);\n",
            true,
        ),
        (
            "            product_bar_status: product_bar::judge(pr_body),\n",
            true,
        ),
        (
            "let product_bar_status =\n    \
             product_bar::judge(&pr_body[..2000.min(pr_body.len())]);\n",
            false,
        ),
        (
            "let product_bar_status = product_bar::judge(pr_body).softened();\n",
            false,
        ),
        // THE CALL RUSTFMT WRAPPED, in both its spellings. The captured
        // argument used to be the raw text between the parentheses, so this
        // shape reached `truncated_argument` as `"pr_body,"` — not a plain
        // path, no adapter suffix to strip — and the guard reported a correct
        // wiring as a truncation. Writing the fully qualified path inside the
        // deeply indented report literal is enough to make rustfmt produce it,
        // so an implementer met it as an unwinnable settled test rather than as
        // a defect. See `calls_at_anchor`.
        (
            "            product_bar_status: product_bar::judge(\n                \
             pr_body,\n            ),\n",
            true,
        ),
        (
            "            product_bar_status: product_bar::judge(\n                \
             &pr_body,\n            ),\n",
            true,
        ),
        // And the mirror, so the trim cannot swallow the defect with the
        // formatting: the same wrapped call carrying a slice is still a
        // truncation.
        (
            "            product_bar_status: product_bar::judge(\n                \
             &pr_body[..2000],\n            ),\n",
            false,
        ),
    ] {
        let calls = product_bar_judge_calls(wiring);
        assert_eq!(
            calls.len(),
            1,
            "exactly one judge call in {wiring:?}; got {calls:?}"
        );
        let ok = truncated_argument(&calls[0].args).is_none()
            && post_processing_after(&calls[0].tail).is_none();
        assert_eq!(
            ok, clean,
            "the tightened wiring guards misjudged {wiring:?}: argument {:?}, tail \
             {:?}",
            calls[0].args, calls[0].tail
        );
    }
}
