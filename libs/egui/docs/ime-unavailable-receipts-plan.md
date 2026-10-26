# IME receipts when surrounding text is unavailable

Local amendment for review before implementation. This does not authorize publication and no code is
implemented by this document. It revises the earlier plan in response to
`egui037-unavailable-ime-receipts-plan-review-20261024-root-v3.md` and to the current WordCraft
sources, not to the previous plan's assumptions.

## Verdict and chosen design

Keep the private optional revision on unavailable stamps. A caller-supplied revision is still not
proof of an edit. A helper accepts an unavailable receipt only when all of these are true:

1. the producer supplies an engine-owned outcome saying the command committed IME-visible content,
   did not perform an automatic rewrite, and was explicitly classified;
2. the engine supplies checked, process-monotonic revision stamps around the complete request;
3. the stamps continue the frame's last observed baseline exactly; and
4. for `Unavailable -> Unavailable`, both revisions are present and `before < after`.

The outcome must distinguish three facts that the old `content_changed` flag conflated:

- `content_changed`: committed content changed (equation tree/linear flag, or body text). Only this
  gates an IME receipt.
- `state_changed`: any committed document state changed, including equation style/representation
  (`jc`, `display`, OMML, linear cache, lazy node normalization) or formatting. This advances the
  revision and therefore poisons a mixed application/IME frame even though it cannot mint a receipt.
- `transformed`: an automatic engine rewrite (equation AutoCorrect/build-up, body smart quote,
  AutoCorrect, AutoFormat, list/dash autoformat) ran.

The chosen equation-delete implementation is one new engine command,
`equation.deleteSurrounding`, that simulates the requested delete steps on a local equation tree and
caret, refuses the whole request if any requested step makes no progress or the count is
unsupported, and commits once. It replaces the current UI loop of up to 64 `Session::run` calls. The
comparison is in “Why the transactional equation command” below.

`Session::rev()` remains only a layout/render-cache revision. It is not receipt evidence.

## Why the previous caller-revision plan is rejected

The current sources still do not provide the evidence the oldest plan assumed.

- `Session::rev()` is a wrapping layout revision (`crates/engine/src/session.rs`, `touch`). It
  advances for no-ops and style-only work, misses selection-only commands, and wraps.
- `equation.type` performs Math AutoCorrect and build-up inside its loop
  (`crates/engine/src/cmd/equation.rs`), but its result has no `changed`/`transformed` fact.
- `delete_in_equation` (`crates/ui-egui/src/keys.rs`) runs up to 64 separate commands and returns
  on the first error; earlier deletions remain applied.
- `delete_surrounding_common` refuses cross-story/cross-paragraph selections, but its current
  `deleted` flag is set from `Ok(())`, not from an actual changed deletion.
- The current receipt code returns early on `before == after` without poisoning the frame chain
  (`crates/egui/src/ime_context.rs`), so an invalid later receipt can leave an earlier
  `InputMethod` cause intact.
- `with_eq` lazily materializes `math.nodes` from `linear` before running its closure
  (`crates/engine/src/cmd/equation.rs:105-119`). A comparison taken before that parse would report a
  no-op backspace as content change.
- `equation.justify` and `equation.display` change `jc`/`display`, and every `with_eq` rewrites the
  linear cache, clears OMML and touches the paragraph, without changing the node tree.
- `equation.convert` with `"all": true` calls `with_eq` once per equation; a per-call outcome would
  let the last equation overwrite an earlier real change.
- Body IME commits are routed through `dialogs_smart_art::type_into` before `type_text`
  (`crates/ui-egui/src/keys.rs:438-450`). A SmartArt edit changes the document without changing the
  body surrounding window.
- `delete_selection` treats `a != b` plus `Ok` as changed. Under Track Changes, `track_delete_as`
  skips already-deleted runs and can return `Ok` without changing visible document content.

## Blocking counterexamples and resolutions

### 1. Lazy equation normalization is not content

`with_eq` must capture its logical before-state **after** the lazy `parse_linear` normalization:

```rust
let normalized = math.nodes.is_empty() && !linear.is_empty() && !math.linear;
if normalized {
    math.nodes = parse_linear(linear);
}
let nodes_before = math.nodes.clone();
let linear_mode_before = math.linear;
```

Then `content_changed = math.nodes != nodes_before || math.linear != linear_mode_before`.
`normalized` contributes only to `state_changed`. An imported equation with empty `nodes` and a
non-empty linear cache, followed by `equation.backspace` at the start, therefore reports
`content_changed = false`, `state_changed = true` (the normalized persisted state changed), and
cannot mint a receipt even though `with_eq` rewrites the cache and clears OMML.

### 2. Style/representation changes must poison the frame

`EquationEffect` has both `content_changed` and `state_changed`. `state_changed` includes:

- lazy node normalization;
- a node-tree or `math.linear` change;
- `math.jc` or `display` changes;
- clearing non-empty `math.omml`;
- a changed linear-cache string.

`Session::run` advances `ime_state_revision` whenever `state_changed` is true, not only when
`content_changed` is true. Thus `equation.justify`/`equation.display` cannot leave the old revision
as the IME receipt's `before`. They do not emit receipts themselves because `content_changed` is
false and they are not IME paths.

### 3. Aggregate equation commands must aggregate subeffects

Every `with_eq` returns `EquationEffect`. Command handlers merge effects with logical OR across all
sub-edits and set `ImeEditOutcome` once after the command:

```rust
fn merge(&mut self, other: EquationEffect) {
    self.content_changed |= other.content_changed;
    self.state_changed |= other.state_changed;
}
```

This is required for `equation.convert --all` (first equation changes, last equation is already in
the requested form), `equation.number`, and any future multi-`with_eq` command. `equation.move` and
`equation.enter` also merge the automatic-rewrite and paragraph-split effects explicitly.

### 4. SmartArt commits are application changes

`type_into` returns a three-way result, not a bool:

```rust
enum SmartArtTypeResult {
    NotHandled,
    ConsumedNoChange, // selected graphic consumes input without editing body text
    Changed,
}
```

When `Changed`, both ordinary text events and IME `Commit` call
`note_ime_application_change` and emit no receipt. `ConsumedNoChange`/`NotHandled` do neither; only `NotHandled` permits body fallback. This closes
the mixed-cause case where the body surrounding window does not change but a SmartArt item did. A
selected graphic with no items must not return `Changed` merely because the Text Pane opened.

## Engine-owned outcome and revision

### `ImeEditOutcome`

Add to `crates/engine/src/lib.rs` and re-export:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImeEditOutcome {
    /// The handler explicitly classified the committed effects.
    pub classified: bool,
    /// IME-visible content changed. Only this gates an IME receipt.
    pub content_changed: bool,
    /// Any committed document state changed, including style/representation.
    pub state_changed: bool,
    /// An automatic engine rewrite was applied.
    pub transformed: bool,
    /// The caret or selection changed. Filled in by `Session::run`.
    pub selection_changed: bool,
}
```

`Session` stores `ime_edit_outcome: ImeEditOutcome`. `Session::run` resets it at the very top,
before registry/disabled/guard early returns. Handlers set the first four fields; on a successful run
`Session::run` fills `selection_changed` by comparing the pre-dispatch body `sel` and equation caret
`(MathEdit::session, MathPos)` with the post-clamp state. On `Err`, reset the outcome to `Default`.

In `Session::run`, after a successful dispatch:

- if `content_changed` is true, force `state_changed = true` (invariant);
- if `!classified && mutates`, set `state_changed = true` conservatively;
- if `state_changed`, allocate a new `ime_state_revision`;
- do not allocate merely because `selection_changed` is true; the lazy caret token covers selection
  and direct UI assignments;
- on `Err`, do not allocate.

The conservative unclassified rule can make a frame `Other`; it cannot mint a receipt because a
receipt requires `classified`.

Add `pub fn ime_edit_outcome(&self) -> ImeEditOutcome`.

### `ime_state_revision`

Add to `crates/engine/src/session.rs`:

```rust
static IME_STATE_REVISION: AtomicU64 = AtomicU64::new(1);

fn next_ime_state_revision() -> Option<u64> {
    IME_STATE_REVISION
        .fetch_update(Relaxed, Relaxed, |n| n.checked_add(1))
        .ok()
}
```

`Session` fields:

```rust
ime_state_revision: Option<u64>,
ime_state_seen: Option<ImeCaretState>,

enum ImeCaretState {
    Body(Selection),
    Equation { session: u64, pos: MathPos },
}
```

`Session::new` initializes both to `None`; the first `ime_state_revision()` call allocates. This
avoids wasting a revision and makes exhaustion behavior easier to test. `set_document`,
`note_body_replaced`, undo/redo and replacement do not reset the revision. `edit_snapshot`/`restore`
do not save or restore either field; add both to the destructuring with `_` so future fields force a
decision.

Public method:

```rust
pub fn ime_state_revision(&mut self) -> Option<u64> {
    self.sync_ime_caret_state();
    self.ime_state_revision
}
```

`sync_ime_caret_state` builds the current `ImeCaretState` from `self.sel` and `self.math`. If it
differs from `ime_state_seen`, it stores the new state and allocates a new revision. Comparison is
only over selection paths/offsets and `(math.session, math.pos)`; it never walks the document. This
catches `select.range`, mouse selection and direct `session.sel` assignments. If a selection moves
and returns to the exact previous state before either stamp is taken, no new final state exists and
the old revision remains valid by design.

The counter is process-monotonic and checked. Exhaustion returns `None`. It is never wrapping,
reset, restored, or reused. Reuse prevention is an engine invariant from the monotonic allocator; it
is **not** a helper check. A `#[cfg(test)]` local helper over an `AtomicU64` tests the checked
allocation boundary without forcing a process-global exhaustion.

## Equation command outcomes

In `crates/engine/src/cmd/equation.rs`:

```rust
#[derive(Clone, Copy, Debug, Default)]
struct EquationEffect {
    content_changed: bool,
    state_changed: bool,
}

impl EquationEffect {
    fn merge(&mut self, other: Self) {
        self.content_changed |= other.content_changed;
        self.state_changed |= other.state_changed;
    }
}
```

`with_eq` returns `(R, EquationEffect)`. It captures after lazy normalization and before the closure:

- `nodes_before`, `linear_mode_before`, `jc_before`, `display_before`, `cached_linear_before`,
  `omml_before_nonempty`;
- runs the closure;
- computes `new_linear`;
- `content_changed = math.nodes != nodes_before || math.linear != linear_mode_before`;
- `state_changed = normalized || content_changed || math.jc != jc_before
  || *display != display_before || omml_before_nonempty || new_linear != cached_linear_before`;
- assigns `*linear = new_linear`, clears `math.omml`, and calls `para.touch()` as today.

`para.touch()` alone is not state change evidence; it is already implied by any successful
`Session::run`.

Command instrumentation:

- `equation.type`: return early as a classified no-op for empty text, before `with_eq` and before
  `set_literal`, in both linear and built-up modes. For non-empty text, track `transformed` from
  `me::autocorrect`/`me::build_up` returning `Some` during the loop. Set the session outcome from the
  merged effect once after `with_eq`.
- `equation.backspace`/`equation.delete`: classified; content/state from `with_eq`;
  `transformed = false`. The empty-equation branch that removes the whole object is
  `content_changed = true, state_changed = true`.
- `equation.move`: keep it pure for ordinary navigation. Compute AutoCorrect/build-up on a local
  copy. If the tree changed, commit once with `with_eq`; set `transformed = true` and the effect.
  Navigate on the local tree so no error can occur after a commit. Any failure leaves the document
  untouched (no `EditSnapshot` restoration needed). Caret-only moves set neither content nor state.
- `equation.enter`: merge AutoCorrect/build-up/number effects; set `transformed` when any automatic
  rewrite fired. A paragraph split is content/state change but not an IME receipt path.
- `equation.justify`, `equation.display`, `equation.set`, `equation.convert`,
  `equation.structure`, `equation.insertStructure`, `equation.insertSymbol`: classify from the
  merged `EquationEffect`. Justify/display are typically `state_changed = true,
  content_changed = false`.
- `equation.convert --all`: initialize one `EquationEffect`, merge every `with_eq`, set the outcome
  once at the end. Do not let the last equation overwrite earlier changes.
- Outside-equation insertion branches that create a new equation set both flags directly.

### `equation.type` length cap

Add a shared engine constant `MAX_EQUATION_TYPE_CHARS = 10_000` and apply it in **both** branches:
the built-up loop and the `math.linear` branch. The linear branch currently inserts the whole string;
it must use the same `chars().take(MAX_EQUATION_TYPE_CHARS)` policy. The UI refuses an IME commit
whose character count exceeds the same cap before running the command, interrupts, and emits no
receipt. This is a real cap in both modes, not a claim that only the built-up branch truncates.

## Transactional equation delete-surrounding command

Add `equation.deleteSurrounding` to the equation command specs:

```rust
CommandSpec::new(
    "equation.deleteSurrounding",
    "Delete Around in Equation",
    "Equation",
    delete_surrounding,
)
.when(editing)
.params(r#"{"before": n, "after": n}"#)
```

The counts remain `MathPos` unit steps, matching the existing `equation.backspace`/`equation.delete`
loop semantics. Add `pub const MAX_EQUATION_DELETE_UNITS: usize = 64;` in the engine; the UI uses the
same constant.

Algorithm:

1. Require `s.math`, read `before`/`after`, and reject `before + after == 0` and either count above
   the cap with `CmdError::Params`. No document edit occurs on refusal.
2. Clone the normalized equation tree via `nodes_of(s, &m.at)` and clone the caret `m.pos` into
   locals. Do not edit the document yet.
3. For each `before` step: clone the local tree and caret, call `me::delete_back`, and treat the step
   as progress when either the tree or caret changed. If neither changed, the request cannot be
   satisfied: return `CmdError::Failed` without touching the document.
4. For each `after` step: the same with `me::delete_forward`.
5. Track `content_changed` when any step changed the tree, not merely the caret. If every step only
   moved the caret, refuse the request. A request that needs a step from an already empty tree is
   likewise refused; the empty-equation object is never removed by this command.
6. If at least one tree change occurred, commit exactly once with
   `with_eq(s, &m.at, |math, _| math.nodes = nodes)`, set `s.math.pos = pos`, and set the session
   outcome from the returned effect with `transformed = false`.
7. Return `{"before": before, "after": after, "changed": true}`. Refusals are `Err` and have no
   outcome; the UI treats them as no receipt and interrupts the composition.

Consequences:

- `DeleteSurrounding` on an empty equation does nothing and emits no receipt.
- A request that exactly deletes the last units leaves an empty equation object, matching one
  `equation.backspace`/`delete` per unit in the current loop.
- A request that would require an additional step after the tree is empty is refused as a whole,
  rather than deleting the object or applying a prefix.
- The document is untouched until the final commit. There is no partial deletion and no UI snapshot
  restoration of `math`/`sel`.
- The request is one undo step, not up to 64. The IME caller calls `join_next_undo()` before the one
  `run`, preserving the typing/composition group.

### Why the transactional command, not the UI loop

Current UI loop:

- up to 64 `Session::run` calls;
- each mutating `run` clones the document once for `before_doc` and once for `checkpoint`, plus
  history/selection snapshots;
- each step records macro/Repeat state, `last_command`, `note_edit`, Go Back marks and
  `also_selected` changes;
- a later refusal leaves a UI `EditSnapshot` restore that deliberately omits `last_command`,
  `recording`, `edit_marks`, `also_selected`, `typing`, `autocap`, canvas join/preedit and other
  fields; the old plan incorrectly called that “exactly the whole request”;
- no per-step content-change fact exists.

Transactional command:

- one `Session::run`, one `checkpoint`, one `before_doc` document clone, one undo step;
- local tree/caret simulation with no document mutation until validation succeeds;
- explicit all-or-nothing refusal and one content/state effect;
- one place to test counts above the cap, empty equations, partial satisfaction and caret movement;
- costs: a new public engine command, a local tree clone, and a per-step local tree clone for
  progress detection. These are bounded by equation size × 64, not document size × 64.

The transactional command is chosen. It is the smaller coherent correctness surface and removes the
up-to-64 whole-document command loop. The one-command form also makes revision, receipt and undo
behavior comprehensible.

A refused mutating command still goes through `Session::run`'s pre-dispatch macro/`last_command`
recording because that happens before dispatch. This plan narrows the guarantee: on refusal, the
document, selection, math session, undo/redo, typing group and dirty state are unchanged; a macro
and Repeat may contain the attempted no-op command. Fixing that generic `Session::run` ordering is
outside this IME plan.

## Body command outcomes

### `delete_selection` and Track Changes

Do not infer content change from `a != b` and `Ok`. Add an effect-returning helper:

```rust
struct SelectionDelete {
    at: Pos,
    content_changed: bool,
    state_changed: bool,
}

fn delete_selection_effect(s: &mut Session) -> Result<SelectionDelete, CmdError>
```

Keep `delete_selection` as a wrapper returning `at` for existing callers. `text.insert`,
`text.backspace` and `text.delete` use the effect to set the outcome.

For Track Changes:

- Extract a preflight from the conditions used by `track_delete_as`: any non-deleted run intersects
  the range, or a paragraph mark is deletable/markable.
- `track_delete` preflights first. If nothing would change, return the original caret without
  creating a revision record. This also prevents an unused Delete revision from being appended.
- `track_delete_as` returns `TrackDeleteEffect { at, changed }`; call sites in `moves.rs`,
  `table.rs` and `column.rs` may ignore the flag. `delete_selection_effect` uses it.
- An all-already-deleted range reports `content_changed = false, state_changed = false`, no
  revision, no receipt.

For an untracked `delete_range`, a valid non-empty range changes stored document text; report
content/state change. If the final available snapshot does not change (for example, hidden deleted
runs), the producer will not certify because the stamp pair is equal; it reports an application
change instead.

### `text.insert`

- Set `classified = true` at handler entry.
- Empty text at a collapsed caret with no deletion effect is a classified no-op.
- `insert_text` reports a non-empty insertion as content/state change; a deletion of a selection
  contributes through `delete_selection_effect`.
- Mark `transformed` in the handler when smart quote `autoformat` returns a replacement, when
  `list_autoformat` returns true, when `autocorrect`/`dash_autoformat` changed the document (make
  `dash_autoformat` return a bool and compare the undo label where needed). `text.insert` is an
  application path, not an IME receipt path, but the outcome must not look like an IME edit.
- The UI keeps its `inserted_literal_at` and `AutoFormat` safeguards as defense in depth and ORs
  them into `transformed`.

### `text.backspace` / `text.delete`

- A non-collapsed selection path uses `delete_selection_effect` and is classified from its effect.
- Other branches (paragraph joins, list/indent changes, table-cell moves, section-break removal)
  remain unclassified unless the implementer explicitly classifies them. They cannot mint a receipt;
  the conservative `Session::run` revision advance makes the frame `Other`.
- No IME receipt path infers success from `Ok(())` alone.

### `delete_surrounding_common` (body)

Keep cross-story/cross-paragraph refusal. For each non-empty requested side:

1. select the range, join undo as today;
2. run `text.backspace`;
3. read `ime_edit_outcome` immediately;
4. require `Ok`, `classified`, `content_changed` and `state_changed`. A classified no-op, an
   unclassified command, a transformed command, or an error rolls back the whole request and emits
   no receipt.

Merge the per-side outcomes. Only a fully satisfied request captures the after stamp and publishes
one receipt through the central producer gate. This rejects a partially applied body request instead
of certifying it.

`edit_snapshot`/`restore` covers document, selection, undo/redo, typing group, dirty and body epoch.
The rollback also restores the original selection, `pending`, and `goal_x`, and clears `ime_join_text`. It does not restore
`last_command`, macro recording, Go Back marks, `also_selected`, `typing`/`autocap` or canvas
preedit. State that narrow guarantee in code comments and test exactly those retained fields rather
than claiming an exact whole-request restore.

## Stamp and tracker changes

In `crates/egui/src/ime_context.rs`.

### Stamp shape

```rust
#[derive(Clone, Debug)]
pub struct ImeMutationStamp {
    snapshot: IMESurroundingText,
    unavailable_revision: Option<u64>,
}
```

Implement `PartialEq`/`Eq` manually:

| before | after | equal? | receipt valid? |
|---|---|---|---|
| `Available(a)` | `Available(b)` | `a == b`; revision ignored | requires `a != b` |
| `Unavailable` | `Unavailable` | both `None` or both `Some` and equal | requires both `Some` and `before < after` |
| `Unavailable` | `Available` | never equal | unavailable side must be `Some` |
| `Available` | `Unavailable` | never equal | unavailable side must be `Some` |

Missing revision on either unavailable endpoint, a one-sided revision, an equal pair, a decreasing
pair, or an invalid mixed transition poisons the frame. Available stamps with differing private
revisions still compare equal, preserving the bounded text-plus-selection contract.

Constructors/accessors:

- `unavailable_at_revision(revision: u64)`;
- `unavailable_revision() -> Option<u64>`;
- existing `unavailable()` with `None`;
- `from_committed(..)`/`from_surrounding_text(..)` with `None`;
- `surrounding_text()`.

`from_surrounding_text` still turns malformed/oversized available payloads into `Unavailable(None)`.

### Observation

`ImeObservation` gets a private `unavailable_revision: Option<u64>` and builder
`unavailable_revision(revision: u64)`. Adding a private field breaks external struct-literal
construction. This module is new/unreleased in the migration, so the break is acceptable; the
builder and all in-tree callers remain source-compatible.

`observe` computes the bounded snapshot first:

- Password: `Unavailable`, revision discarded;
- computed `Available(payload)`: `Available(payload)`, no revision;
- computed `Unavailable` for any other reason: `Unavailable(Some(rev))` when the observation
  supplied a revision, else `Unavailable(None)`.

`ImeContextUpdate` and `IMEOutput` are unchanged; no revision is published on the wire.

### Baseline, chain and validation

`ContextState::baseline` becomes `ImeMutationStamp`. `ImeReceiptChain` stores
`first_before: ImeMutationStamp` and `last_after: ImeMutationStamp`. `continues`, `is_exact` and
`push` compare/push stamps. `observe` computes `changed = previous_stamp != new_stamp`, so a
revision-only unavailable change is a changed observation, not an unchanged one.

`note_input_method_apply`:

1. lookup viewport/context; reject absent/unfocused/target mismatch/password without touching
   another target;
2. `ensure_frame`;
3. validate the transition table above; on invalid, poison
   (`origin_other = true`, `last_cause = Other`, `receipt_chain = None`) and return false. This
   includes `before == after`;
4. check chain continuation: no chain → `before == context.baseline`; chain →
   `first_before == baseline && last_after == before`; on mismatch poison and return false;
5. push `after`, increment the saturating diagnostic count, return true.

`note_application_change` uses the same poison helper. The helper does not keep a global high-water
mark; reuse prevention is only the engine's monotonic allocator. Do not claim a helper-level reused
pair check.

A helper call with an equal/invalid receipt after an accepted receipt still clears the chain. The
producer must not make such a call for a genuine classified no-op; see below.

## WordCraft producer integration

Paths are relative to `../wordcraft-implementation`.

### Central receipt gate

Add one pure decision function in `crates/ui-egui/src/keys.rs` (or a small private module):

```rust
enum ImeProof { NoMutation, UnprovenMutation, Certified }

fn classify_ime_proof(
    outcome: ImeEditOutcome,
    before: &ImeMutationStamp,
    after: &ImeMutationStamp,
) -> ImeProof
```

Rules:

- `!classified`: `UnprovenMutation` if `state_changed`, else `NoMutation`.
- `transformed`: `UnprovenMutation`.
- `classified && state_changed && !content_changed`: `UnprovenMutation`, including normalization and style-only changes.
- `content_changed`: require the stamp table to prove the transition; if not, `UnprovenMutation`.
  (`Certified` otherwise.)
- no content and no state: `NoMutation` if `before == after`, else `UnprovenMutation` (a caret
  change during the request is still not a receipt).
- mixed availability requires the unavailable side's revision; `Unavailable -> Unavailable`
  requires both revisions and `before < after`; available -> available requires unequal stamps.

`publish_ime_receipt(app, before, after, outcome)`:

- `NoMutation`: do nothing; no application signal;
- `UnprovenMutation`: call `note_ime_application_change` and emit no receipt;
- `Certified`: call `note_ime_receipt`.

This centralizes revision-presence gating in the producer. `note_ime_receipt` remains the structural
tracker check.

### `ime_stamp` / `observe_ime`

- Equation target: `session.ime_state_revision().map(ImeMutationStamp::unavailable_at_revision)` or
  `unavailable()` when exhausted.
- Body unavailable (`ime_window == None`): the same revision stamp.
- Body available: existing `from_committed` available stamp.
- `observe_ime` sets `.unavailable_revision(rev)` only for the equation unavailable branch and the
  body-unavailable branch when `ime_state_revision()` is `Some`. Never for available text or
  password.
- `ime_window` keeps its `(session.rev(), cursor, anchor)` cache. The new revision is not a cache
  key.

### Equation IME `Commit(text)`

- Refuse before running if `text.chars().count() > MAX_EQUATION_TYPE_CHARS`: interrupt the
  composition and emit no receipt. No prefix is committed in either linear or built-up mode.
- Capture `before = ime_stamp(app)`, run `equation.type`, read the outcome immediately, capture
  `after`.
- On `Err`: keep the existing interrupt and emit no receipt.
- On `Ok`: call `publish_ime_receipt`. A transformed automatic rewrite calls
  `note_ime_application_change`; a classified no-op does nothing; a revision-exhausted change is
  unproven and calls the application signal.
- Equation `Event::Text` and `Paste` remain application typing and never take a receipt.

### Equation `DeleteSurrounding`

Replace `delete_in_equation`'s loop with:

```rust
if before > MAX_EQUATION_DELETE_UNITS || after > MAX_EQUATION_DELETE_UNITS {
    app.canvas.ime_interrupt_now = true;
    return;
}
if before == 0 && after == 0 { return; }
app.canvas.ime_preedit.clear();
app.canvas.ime_preedit_range = None;
if app.session.typing_open() { app.session.join_next_undo(); }
let before_stamp = ime_stamp(app);
match app.run("equation.deleteSurrounding", json!({"before": before, "after": after})) {
    Ok(_) => {
        let outcome = app.session.ime_edit_outcome();
        let after_stamp = ime_stamp(app);
        publish_ime_receipt(app, before_stamp, after_stamp, outcome);
    }
    Err(_) => {
        app.canvas.ime_interrupt_now = true;
    }
}
```

Every refusal interrupts; the transactional command leaves the document untouched. No UI
`EditSnapshot`, `math` save/restore or canvas join/preedit restore is needed for equation deletion.
`DeleteSurroundingBytes` inside an equation remains ignored.

### Body `type_text` and `delete_surrounding_common`

`type_text`:

- keep the existing receipt only for an actual IME commit;
- capture `before` before `run`, read `ime_edit_outcome` after;
- OR the existing UI smart-quote/AutoFormat detection into `transformed`;
- call `publish_ime_receipt` for IME; for non-IME ordinary typing, keep existing behavior (the
  bounded body snapshot itself exposes changes). SmartArt is handled separately below because its
  body snapshot may not change.

`delete_surrounding_common` follows the body outcome rules above: all-or-nothing on non-empty
sides, `content_changed` required, aggregate outcome, one receipt only after full success, exact
available/unavailable stamps checked by the central gate.

### SmartArt

Change `type_into` to return `SmartArtTypeResult`. In both `Event::Text` and IME `Commit`:

- `Changed` → `note_ime_application_change`; no receipt for IME; continue;
- `ConsumedNoChange` → consume the event; do not edit the body and do not emit a receipt.
- `NotHandled` → existing body fallback (`type_text` for IME or ordinary body typing).
- A selected graphic command failure also consumes the event and interrupts composition; it never falls back to the body.

A selected SmartArt graphic with no items returns `ConsumedNoChange` and does not poison a frame.
A real SmartArt item edit returns `Changed` and forces `Other`, even if the body surrounding window
is unchanged.

## Operation scope

| Case | Observation | Successful mutation receipt |
|---|---|---|
| Body, one paragraph, small selection | `Available(text)` | `Available -> Available`, unequal exact text/selection, `content_changed && !transformed` |
| Body, wide in-paragraph selection | `Unavailable(rev)` | delete-only → `Available(text)`; commit replacing it → `Available(text)` |
| Body, cross-story/cross-paragraph | `Unavailable(rev)` | `DeleteSurrounding` refused, no receipt; commit may be `Unavailable -> Available` with engine evidence |
| Body request with one side unsatisfied | unchanged/other | whole request rolled back, no receipt |
| Track Changes, all requested range already deleted | `Available(text)` | no change, no revision, no receipt |
| Equation, satisfied transactional delete | `Unavailable(rev)` | `Unavailable -> Unavailable`, both revisions, `before < after` |
| Equation, empty or unsatisfiable delete | `Unavailable(rev)` | command `Err`, no commit, no receipt, composition interrupted |
| Equation, automatic type rewrite | `Unavailable(rev)` | no receipt; application change → `Other` |
| SmartArt item edited by IME | body snapshot may be unchanged | no receipt; application change → `Other` |
| Password | `Unavailable`, revision discarded | always rejected |
| Replacement/new equation/body swap | owner/epoch changes | revision cannot substitute; composition interrupted |

## Revision exhaustion

- `ime_state_revision()` returns `None`; `ime_stamp` emits `Unavailable(None)`;
- no unavailable receipt can be certified;
- if the command committed a mutation but the stamps cannot prove it (exhaustion, missing endpoint,
  equal/non-increasing revisions), call `note_ime_application_change` and emit no receipt;
- if the command was a classified no-op with no state/selection change, do nothing and let an
  earlier accepted receipt remain the cause;
- available-text receipts still work because they need no revision;
- a later observation with an unproven revision change reports `Other`.

## Performance and memory

- One `Option<u64>` and one bounded selection/equation-caret token per `Session`; no document hash.
- One atomic allocation per committed state change, not per UI pass or observation.
- `Session::run` adds a small caret comparison and trusts explicit handler outcomes.
- `with_eq` clones the equation node tree once for its logical before-state. `equation.deleteSurrounding`
  additionally clones the local tree per requested step to detect no progress, so the worst case is
  O(64 × equation size), plus the existing `tidy` walk, not O(64 × document size).
- The transactional equation delete uses one `Session::run`: one `checkpoint` and one `before_doc`
  document clone, instead of up to 64 of each in the UI loop.
- The bounded surrounding window remains at most 4000 bytes. The receipt chain retains two bounded
  stamps. `ImeMutationStamp` equality is O(1) for unavailable and O(window bytes) for available.
- No whole-document hash or whole-document comparison is introduced.

## Alternatives considered

1. **Keep `Other`.** Safe but loses the requested change-cause contract. Rejected because the engine
   outcome now makes a bounded proof possible.
2. **Caller-supplied `Session::rev()` or an application counter.** Rejected: no-ops, automatic
   rewrites, selection-only application changes and partial deletion remain indistinguishable.
3. **Engine outcome without a revision.** Rejected for unavailable states: a caller can move the
   selection between observations and the final unavailable snapshot cannot show it.
4. **Bounded equation-tree fingerprint.** Rejected: hashing/collision assumptions and no selection
   coverage; the local effect already supplies the needed facts.
5. **Whole-document hash.** Rejected: unbounded work and memory per pass.
6. **Current UI 64-step equation delete loop.** Rejected: up to 64 whole-document snapshots, partial
   application, incomplete rollback fields, no per-step content fact.
7. **Chosen: engine outcome + checked monotonic revision + transactional equation delete command.**
   Bounded, fail-closed, explicit, and compatible with available-text receipts.

## Implementation gates and regression cases

### egui helper tests

1. `Unavailable(10) -> Unavailable(11)` is `InputMethod`.
2. Equal (`10 -> 10`) and decreasing (`11 -> 10`) revisions are rejected and poison the chain.
3. Missing revision on either unavailable side is rejected and poisoned.
4. One-sided unavailable revision in either mixed transition is rejected and poisoned.
5. Available stamps with differing private revisions compare equal; available text/selection
   differences still drive the receipt.
6. Mixed `Unavailable(rev) -> Available(text)` and reverse are accepted only with the unavailable
   revision and exact final observation.
7. A helper call with an equal/invalid receipt after an accepted receipt clears the chain and the
   later unchanged observation is `Other`.
8. A genuine classified no-op receipt is not called by the producer; an earlier accepted receipt
   remains `InputMethod` when the final snapshot is unchanged.
9. Mixed application-then-IME and IME-then-application in both orders remain `Other`; multiple
   receipts remain `InputMethod` only when the final snapshot matches the chain.
10. Wrong owner/epoch, focus loss, replacement, password purpose and viewport transitions reject
    receipts; password observations retain no revision.
11. Two thousand receipts retain one compressed chain and exactly two stamps.
12. Revision-only unavailable changes count as changed observations without a receipt.

### Engine tests

1. Built `x+y`, caret at `MathPos { path: [], off: 0 }`, `equation.backspace`: classified,
   `content_changed == false`, `state_changed == false`, revision unchanged.
2. Imported equation with empty `nodes`, non-empty `linear`, `math.linear == false`, caret at start,
   `equation.backspace`: `content_changed == false`, `state_changed == true`, revision advances, no
   receipt can be minted; the lazy normalization is captured after `parse_linear`.
3. `equation.type("")` at a collapsed caret is a classified no-op with no `with_eq`, no
   normalization, no `set_literal`, no revision advance, in both linear and built-up modes.
4. `equation.type("a/b ")`: `content_changed == true`, `transformed == true`; no receipt.
5. `equation.type("\\alpha ")`: `transformed == true`; no receipt.
6. `equation.justify` and `equation.display`: `content_changed == false`,
   `state_changed == true`, revision advances; no receipt.
7. Two-equation `equation.convert --all` where the first changes and the last is already in the
   requested form: one outcome with `content_changed == true, state_changed == true`.
8. `equation.move`: a build-up move commits once, marks `transformed`; an error before commit leaves
   document and caret unchanged.
9. Transactional `equation.deleteSurrounding`:
   - one undo step and one document command for a multi-unit request;
   - a first step that changes and a second that cannot progress returns `Err`, with document,
     selection, math session, history and dirty unchanged;
   - counts above the cap are refused before any deletion;
   - an empty equation refuses and leaves the object;
   - exactly deleting the last units leaves an empty equation object;
   - a one-step delete updates `math.pos` and commits content.
10. A refused equation delete leaves `last_command`/macro recording as the only pre-dispatch side
    effects; test those explicitly rather than claiming whole-request restoration.
11. Track Changes, all requested text already deleted: no content/state change, no new revision
    record, no receipt.
12. Body `delete_surrounding_common` with a first side changed and a second side classified no-op:
    whole request rolled back; document/selection/undo restored; the documented non-restored fields
    are asserted as retained.
13. Selection-only `select.range` and a direct `session.sel = ...` advance `ime_state_revision()`
    on the next call; a receipt whose `before` carries the old revision fails chain continuity.
14. Checked revision allocator at `u64::MAX`: returns `None` without wrapping; a local `AtomicU64`
    helper covers the boundary. Producer gating tests with `Unavailable(None)` cover equation
    commit, equation delete and wide-selection body delete without forcing process-global state.
15. `equation.type` over `MAX_EQUATION_TYPE_CHARS` in linear and built-up modes commits no prefix;
    an IME commit over the cap is refused before the command.
16. `text.insert` marks smart quote, list, dash and AutoCorrect work as `transformed`.
17. `type_into` returns `Changed` for a real SmartArt item edit and `ConsumedNoChange` for a graphic with no
    items; an IME commit in either case emits no receipt and a changed case forces `Other`.

### UI integration tests

1. Actual equation producer: a content-changing non-transformed IME commit emits one receipt and
   `InputMethod`; a no-op `DeleteSurrounding` at equation start emits none.
2. `a/b ` build-up and `\\alpha ` AutoCorrect emit `Other`.
3. Wide in-paragraph delete-only selection: unavailable before, available after, `InputMethod` only
   when the transaction changed content and the final text matches.
4. Cross-block `DeleteSurrounding` is refused; no deletion or receipt.
5. Application selection move between observation and an IME receipt is `Other`.
6. Mixed ordinary typing/application changes and IME commits in both orders are `Other`; multiple
   IME receipts are `InputMethod` only on exact final state.
7. Classified no-op after an accepted receipt in the same frame keeps the earlier `InputMethod`
   cause; an unproven mutation after it forces `Other`.
8. SmartArt IME commit after an accepted body receipt forces `Other`; empty SmartArt does not.
9. Password path with a revision is rejected and retains no text/revision.
10. Same-ID equation session replacement increments the target epoch and interrupts the
    composition; an old receipt cannot be reused.

### Review gates

- A fresh independent plan review precedes implementation.
- A different fresh implementation review follows executed managed checks.
- Native Wayland/macOS/Windows qualification remains separate from headless tests.
- No Cargo/build/test/fmt/clippy/metadata/VM/resource/commit/push/PR action is part of this plan
  edit.

## Remaining design gaps and intentional unsupported behavior

1. Cross-block deletion remains unsupported; only observation and a later commit are specified.
2. `equation.deleteSurrounding` deletes `MathPos` unit steps, not a reversible mapping from visible
   equation characters. A request that needs a step from an empty tree is refused; object removal is
   not part of this command.
3. Body IME deletion is not transactional at the engine level. It uses two commands plus the
   existing `EditSnapshot` rollback, and its non-restored fields are documented and tested.
4. A refused mutating command may remain in macro recording/`last_command` because `Session::run`
   records before dispatch. The document itself is unchanged. Fixing this generic behavior is
   outside this plan.
5. Direct document mutation outside `Session::run` is not revision-tracked; the plan assumes the
   existing rule that all document edits go through commands.
6. The conservative unclassified-command rule advances the revision for some no-ops. That can make
   a frame `Other`; it can never mint a receipt. This is an intentional safety/UX tradeoff.
7. `ImeObservation` gains a private field and therefore loses external struct-literal construction.
   The module is unreleased; if source compatibility becomes required, use `#[non_exhaustive]` plus
   a constructor rather than exposing the revision.
8. A rolled-back/failed IME request may consume revisions and make the next frame `Other`; revisions
   are never rolled back to avoid reuse.


## Final v5 review dispositions (implementation contract)

The independent v5 review is recorded in
`egui037-unavailable-ime-plan-adversarial-20261024-root-v5.md` in the migration evidence.
Its two blockers are resolved by these requirements, which supersede any earlier ambiguous prose.

1. Available body stamps intentionally ignore the unavailable revision. Therefore a revision
   increment alone cannot classify a mixed formatting/IME frame. Add an engine-owned pending
   application-change signal, set centrally for successful **non-IME** runs whose outcome is
   `content_changed || state_changed || (!classified && spec.mutates)`. Selection-only changes
   remain covered by the observed stamp. The signal is not cleared by a later command.
   Expose an explicit `run_ime` transaction entry point; nested commands inherit its origin.
   Restore the previous origin on every return, including unknown/disabled/refused commands.
   Normal `run` remains application-origin outside that explicit scope. The signal can be a
   pending bool with a take method (one UI owns each Session); it needs no wrapping counter.
   Drain it into `note_ime_application_change` before publishing any receipt and at final
   `observe_ime`. Do not drain it when no Context can receive it. Thus a command before OR
   after the receipt forces `Other` even if it does not change the available text stamp.
   The UI's IME command wrapper must preserve the normal App::run side effects. Engine
   transactions that roll back must not leave a successful application-change signal.
   Add both-order real UI tests for `para.alignCenter` and an accepted IME commit; both must
   report `Other`. An IME-only literal commit must still report `InputMethod`.
2. SmartArt selected-but-empty is `ConsumedNoChange`, never `NotHandled`. It consumes input
   without body insertion, receipt or spurious application edit. A selected command failure
   consumes/interrupts too. Only absence of a SmartArt target permits body fallback. Tests
   must verify body/document contents as well as cause.
3. Allocation failure explicitly stores `ime_state_revision = None`; do not retain stale Some.
4. Body deletion rollback restores `pending` and `goal_x` in addition to the documented snapshot.
   A refused equation deletion consumes `join_next_undo`; document that alongside macro/Repeat
   side effects instead of claiming whole-Session rollback.

Implement this reviewed contract now, then run focused regression tests and obtain a fresh
implementation review. Do not repeat broad planning as a substitute for implementation.
