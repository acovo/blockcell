# Agent Runtime and Learning Systems Review

**Date:** 2026-07-24
**Status:** Module 5 Task 6 skill-evolution lifecycle review complete; M16-M25 remediated
**Plan:** `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`

## Verification baseline

- `cargo test -p blockcell-storage -- --nocapture`: 59 passed, 0 failed.
- `cargo test -p blockcell-agent runtime -- --nocapture`: 122 passed, 0 failed; 492 filtered out.
- `cargo test -p blockcell-agent subagent -- --nocapture`: 5 passed, 0 failed; parent-cancellation terminal state is covered separately by the TaskManager regression test below.
- `cargo test -p blockcell-agent memory_system -- --nocapture`: 21 passed across unit and integration targets, 0 failed after adding panic-cleanup coverage.
- `cargo test -p blockcell-agent response_cache -- --nocapture`: 22 passed, 0 failed after adding the zero-capacity contract test.
- `cargo test -p blockcell-agent --all-targets`: 630 unit tests plus 56 integration tests passed, 0 failed after the R17-R19 fixes.
- `cargo fmt --all -- --check`: passed after the R17-R19 fixes.

Passing tests establish the current baseline. The remediated cross-channel/account routing,
message-task panic, and deferred-review cleanup cases now have regression coverage. The
non-WebSocket error-delivery and streamed-provider success-accounting cases now also have
regression coverage.

Final remediation verification:

- `cargo test -p blockcell-storage -p blockcell-tools -p blockcell-agent --all-targets`: all passed (Agent 609, Storage 62, Tools 408, plus integration targets).
- `cargo test -p blockcell --bin blockcell websocket -- --nocapture`: 14 passed, 0 failed.
- `cargo fmt --all -- --check`: passed.

Module 5 Task 4 review verification (review-only; no implementation changes):

- `cargo test -p blockcell-storage -- --nocapture`: 68 passed, 0 failed after the M4-M8 fixes.
- `cargo test -p blockcell-agent memory -- --nocapture`: 150 focused unit tests and 10 memory integration tests passed, 0 failed.
- `cargo test -p blockcell-tools memory -- --nocapture`: 24 passed, 0 failed.
- `cargo test -p blockcell --bin blockcell memory -- --nocapture`: 7 passed, 0 failed.

Module 5 Task 5 review verification (review-only; no production-code changes):

- `cargo test -p blockcell-agent ghost -- --nocapture`: 33 passed, 0 failed after the M9-M15 fixes.
- `cargo test -p blockcell-agent learning -- --nocapture`: 37 passed, 0 failed after sharing the coordinator.
- `cargo test -p blockcell-agent skill_file_store -- --nocapture`: 29 passed, 0 failed.
- `cargo test -p blockcell-storage ghost_ledger -- --nocapture`: 6 passed, 0 failed.
- `cargo test -p blockcell-agent runtime -- --nocapture`: 127 passed, 0 failed.

Module 5 Task 6 review verification (review-only; no production-code changes):

- `cargo test -p blockcell-skills -- --nocapture`: 113 unit tests and 6 integration tests passed, 0 failed.
- `cargo test -p blockcell-scheduler ghost:: -- --nocapture`: 7 passed, 0 failed.
- `cargo test -p blockcell-scheduler evolution -- --nocapture`: 0 matching tests; the evolution workers currently have no focused unit coverage.
- `cargo test -p blockcell-scheduler -- --nocapture`: 53 passed, 2 failed in pre-existing `consolidator` lock-cleanup tests outside Task 6 (`test_dream_releases_lock_when_commit_backup_recovery_fails` and `test_dream_cleans_state_and_lock_when_staging_prepare_fails`).

Module 5 Task 6 remediation verification:

- `cargo test -p blockcell-skills -- --nocapture`: 123 unit tests and 6 integration tests passed, 0 failed.
- `cargo test -p blockcell-storage evolution_workflow -- --nocapture`: 5 passed, 0 failed.
- `cargo test -p blockcell-scheduler -- --nocapture`: 55 passed, with the same 2 pre-existing `consolidator` lock-cleanup failures outside Task 6.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

## Runtime lifecycle map

```text
InboundMessage
  -> AgentRuntime::run_loop
  -> steering existing task OR create msg_<uuid> task
  -> run_message_task creates a per-message AgentRuntime
  -> process_message / process_message_inner
  -> context + provider/tool loop + memory hooks
  -> session persistence + outbound/event delivery
  -> TaskManager completed/failed state
  -> task_done channel removes active runtime maps
```

Shared objects passed into the per-message runtime include the provider pool, tool registry, task manager, outbound/confirmation/event channels, structured memory handle, capability registry, and evolution engine. File-memory and skill stores are reopened for each per-message runtime and coordinate through their filesystem lock directories.

## Tool authorization map

```text
Model ToolCallRequest(name, arguments)
  -> disabled tool/skill toggles
  -> ToolPolicy evaluate + audit (allow / ask / deny)
  -> dangerous exec/file_ops confirmation
  -> extracted path accesses + PathPolicy/session authorization
  -> ToolContext channel/user permissions
  -> ToolRegistry lookup + schema validation + required-permission check
  -> concrete Tool::execute
```

The runtime performs policy and path gates before constructing `ToolContext`; the registry then
performs the final tool lookup, parameter validation, and permission-subset check. Spawn-capable
tools receive the current runtime abort token and origin session through `RuntimeSpawnHandle`.

## Confirmed findings

### R1 — High: Steering and active-task routing collide across channels and accounts

**Locations:**

- `crates/agent/src/runtime/run_loop.rs:19-22`
- `crates/agent/src/runtime/run_loop.rs:288-302`
- `crates/agent/src/runtime/run_loop.rs:400-438`
- `crates/agent/src/steering.rs:6-12`
- `crates/core/src/message.rs:20-31`

**Trigger:** Two conversations owned by the same Agent have the same raw `chat_id`, but differ by channel or `account_id`. While conversation A has an active task, conversation B sends a text-only message.

**Control flow:** Persistent session identity correctly uses `InboundMessage::session_key()`, which incorporates channel and account. The runtime's `active_chat_tasks`, `active_steering_senders`, and `SteeringSessionKey`, however, use only raw `chat_id` (plus `agent_id` in the shared registry). `run_loop` therefore finds A's sender using B's `chat_id` and injects B's content into A's running LLM history. Cancellation and replacement paths use the same incomplete key and can cancel A when B starts or cancels work.

**Impact:** Cross-conversation content disclosure, instruction injection into the wrong task, incorrect cancellation, and corrupted session/task ownership. This is especially relevant to Gateway deployments with multiple channels or multiple accounts for one channel.

**Repair direction:** Introduce one canonical active-conversation key derived from at least `agent_id + channel + account_id + chat_id`, preferably reusing the persisted session identity contract. Use it consistently in all four active maps, the task-done message, cancellation lookup, and `SteeringSessionKey`. Add a test with identical `chat_id` values on two channels and another with two account IDs.

**Resolution:** Fixed. `ActiveConversationKey` now combines `agent_id` with `InboundMessage::session_key()` and is used consistently by runtime active maps, cleanup/cancellation paths, the shared steering registry, and WebSocket routing. Regression tests: `active_conversation_key_separates_channels_with_same_chat_id`, `active_conversation_key_separates_accounts_with_same_channel_and_chat_id`, and `active_ws_chat_routes_to_steering_channel`.

### R2 — Medium: A panicking message task remains Running and skips cleanup/receipts

**Locations:**

- `crates/agent/src/runtime/run_loop.rs:21-23`
- `crates/agent/src/runtime/run_loop.rs:86-100`
- `crates/agent/src/runtime/run_loop.rs:439-462`
- `crates/agent/src/runtime/message_task.rs:184-224`
- `crates/agent/src/task_manager.rs:818-831`

**Trigger:** Any panic escapes `run_message_task`, for example from an unexpected provider/tool/runtime invariant failure.

**Control flow:** The spawned closure sends `task_done` only after `run_message_task(...).await` returns. Its `JoinHandle` is stored but never awaited by a guard task. A panic therefore skips the send, skips `set_failed`, skips completion-receipt resolution, and leaves the active maps populated. Periodic cleanup removes only terminal tasks, so the TaskManager record remains `Running` indefinitely. Typed/forked-agent paths already have explicit JoinHandle panic guards, showing the expected lifecycle pattern is available elsewhere in the same module.

**Impact:** Stuck Running tasks, stale steering registrations, missing error delivery, and cron completion receipts waiting until their outer timeout.

**Repair direction:** Wrap the message task with a supervisor that awaits the JoinHandle, converts panic/cancellation into terminal task state, resolves or cancels any message receipt, and always sends a cleanup event. Keep task state transition and active-map cleanup idempotent.

**Resolution:** Fixed. Active state stores `AbortHandle`; a separate supervisor awaits every worker JoinHandle, marks panic failures, completes or cancels receipts, and always emits the cleanup notification. Regression test: `message_task_supervisor_marks_panics_failed_and_emits_cleanup`.

### R3 — Medium: Deferred learning-review reservations leak on empty responses and early errors

**Locations:**

- `crates/agent/src/runtime/process_message_inner.rs:639-653`
- `crates/agent/src/runtime/process_message_inner.rs:1264-1280`
- `crates/agent/src/runtime/process_message_inner.rs:1286-1289`
- `crates/agent/src/runtime/process_message_inner.rs:1548-1561`
- `crates/agent/src/learning_coordinator.rs:268-375`
- `crates/agent/src/learning_throttle.rs:40-69`

**Trigger:** A memory or skill nudge reserves a throttle slot, after which the turn ends with an empty `final_response` (the message-tool short-circuit explicitly clears it), or returns an error through a `?` path before `spawn_review`.

**Control flow:** `check_memory_nudge` and `check_skill_nudge` call `try_start_review`, incrementing `active_reviews`. Completion is balanced only by `LearningReviewCompletionGuard`, which exists inside `spawn_review`. The runtime calls `spawn_review` only when `final_response` is non-empty. Empty-response and early-error paths have no cancellation guard and do not call `review_completed`/`cancel_review`.

**Impact:** Each affected turn permanently consumes a review slot. With the configured concurrency limit of two, two such turns disable subsequent nudge reviews for the lifetime of that runtime.

**Repair direction:** Represent the throttle reservation with an RAII permit created when the nudge is accepted. Transfer the permit into the spawned review; dropping it on any other return path must release without starting cooldown. Add empty-response, skill-error, and aborted-turn tests.

**Resolution:** Fixed. `LearningReviewReservationGuard` cancels an unspawned reservation without cooldown and converts to a completion guard only after the review task starts. This covers empty responses, `?` returns, and dropped futures. Regression test: `pending_review_reservation_releases_slot_when_dropped_before_spawn`.

### R4 — Medium: Runtime errors are not delivered to non-WebSocket external channels

**Locations:**

- `crates/agent/src/runtime/message_task.rs:154-170`
- `crates/agent/src/runtime/message_task.rs:213-253`
- `bin/blockcell/src/commands/gateway/outbound.rs:6-27`
- Representative error exits: `crates/agent/src/runtime/process_message_inner.rs:109-110`, `crates/agent/src/runtime/process_message_inner.rs:240-248`, `crates/agent/src/runtime/process_message_inner.rs:804-849`, and `crates/agent/src/runtime/process_message_inner.rs:2031-2041`

**Trigger:** A Telegram, Slack, Discord, or other non-WebSocket message reaches a fallible runtime branch after `AgentRuntime` construction, such as session loading, forced-skill resolution, model-selected skill execution, or final session persistence, and `process_message` returns `Err`.

**Control flow:** `run_message_task` broadcasts a JSON `error` event, marks the task failed, and completes an optional receipt, but it does not send an `OutboundMessage`. Gateway external-channel delivery consumes `outbound_tx` and calls `ChannelManager::dispatch_outbound_msg`; runtime `event_tx` is the WebSocket event path and is intentionally not bridged to external channels. The earlier `AgentRuntime::new` failure branch demonstrates the expected behavior by sending `❌ {error}` through `outbound_tx`.

**Impact:** The task is internally recorded as failed, but an external-channel user receives no terminal reply and experiences a silent timeout. Operational logs and WebSocket observers may see the error while the originating Telegram/Slack/Discord conversation does not.

**Repair direction:** In the `process_message` error branch, send an account-aware error `OutboundMessage` for channels whose terminal response is delivered through `outbound_tx`, while retaining the filtered WebSocket error event. Centralize terminal failure delivery so runtime-construction failures, processing failures, panics, and cancellations follow one idempotent channel-routing contract. Add a message-task regression test that injects a deterministic `process_message` error and asserts one external outbound error, plus a WebSocket test that asserts no duplicate terminal event.

**Resolution:** Fixed. Message-task failures now retain the WebSocket error event and send one account-aware `❌` outbound reply only for external channels. Regression tests: `message_task_failure_delivers_error_to_external_channel` and `message_task_failure_uses_event_only_for_websocket`.

### R5 — Medium: Explicit stream completion skips provider success accounting

**Locations:**

- `crates/agent/src/runtime/message_dispatch.rs:68-168`
- `crates/agent/src/runtime/message_dispatch.rs:193-220`
- `crates/providers/src/pool.rs:25-44`
- `crates/providers/src/pool.rs:339-355`
- `crates/providers/src/pool.rs:477-519`

**Trigger:** A provider has one or more recorded transient/server failures, then completes a normal streaming call using the standard `StreamChunk::Done` event, and later encounters another transient/server failure.

**Control flow:** The `StreamChunk::Done` arm constructs the response and immediately returns `Ok` without calling `provider_pool.report(pool_idx, CallResult::Success)`. The fallback path for providers that close cleanly without `Done` does report success. Provider-pool success reporting both increments the routing success count and resets `transient_fail_count`; omitting it means failures separated by successful streamed calls are still accumulated as if consecutive.

**Impact:** A healthy provider can enter cooldown after non-consecutive failures, reducing availability or causing a single-provider deployment to report no healthy provider. Success-based routing statistics also undercount the dominant normal streaming path, weakening `LatencyFirst` selection behavior (currently implemented using success count).

**Repair direction:** Report `CallResult::Success` exactly once immediately before returning from every successful streaming completion path. Add a provider-pool-visible regression test covering failure, `Done` success, then additional failures, and assert that the success resets the failure sequence and increments success statistics.

**Resolution:** Fixed. The explicit `StreamChunk::Done` path now reports `CallResult::Success` before returning, resetting the transient failure sequence and incrementing success statistics. Regression test: `explicit_stream_done_reports_provider_success`.

### R6 — Medium: Tool-policy confirmation can bypass hard path-policy denial

**Locations:**

- `crates/agent/src/runtime/tool_exec.rs:214-224`
- `crates/agent/src/runtime/tool_exec.rs:302-308`
- `crates/agent/src/runtime/path_security.rs:152-188`
- `crates/agent/src/runtime/path_security.rs:230-267`
- `crates/core/src/path_policy.rs:195-242`
- `crates/agent/src/runtime/tests.rs:4630-4674`

**Trigger:** An administrator configures a matching `tool_policy` rule with decision `ask` for a filesystem-bearing tool, the model supplies a path that `PathPolicy` would deny (including built-in protected paths such as `~/.ssh`), and the user approves the tool-policy confirmation.

**Control flow:** A successful tool-policy `ask` returns `ProceedConfirmed`. Before the path gate runs, `execute_tool_call` extracts every referenced path and inserts its directory plus operation into `authorized_dirs`. `check_path_permission` checks this session authorization before evaluating `PathPolicy`, so it returns success for the newly cached directory and never reaches the built-in/user deny rule. The existing `tool_policy_ask_confirmation_skips_duplicate_path_confirmation` test confirms that approval intentionally suppresses the later path-policy confirmation; the same ordering also suppresses hard denial.

**Impact:** Adding an interactive tool-policy guard can weaken the independent path-security boundary. A user approval intended to authorize one tool invocation overrides a path policy documented to treat built-in sensitive paths as always denied, permitting reads/writes/exec operations that the path policy would otherwise block.

**Repair direction:** Preserve hard-deny precedence. Evaluate all extracted paths against `PathPolicy` before caching any approval; reject immediately on `Deny`, and use the tool-policy approval only to satisfy paths whose result is `Confirm`. Cache only those confirmed path/operation pairs after the hard-deny pass. Add regression tests for a tool-policy `ask` combined with both a built-in sensitive-path deny and an explicit user deny rule.

**Resolution:** Fixed. PathPolicy is now evaluated for hard denial before workspace or cached authorization. ToolPolicy approval satisfies only `Confirm` outcomes and is cached only after the deny pass, preserving one-confirm behavior without weakening protected paths. Regression test: `tool_policy_ask_cannot_override_builtin_path_deny`.

### R7 — High: Fork-mode agents bypass runtime tool and path authorization

**Locations:**

- `crates/tools/src/agent.rs:124-149`
- `crates/agent/src/runtime.rs:2315-2321`
- `crates/agent/src/runtime/lightweight_handle.rs:185-292`
- `crates/agent/src/forked/agent.rs:74-128`
- `crates/agent/src/forked/agent.rs:554-583`
- `crates/agent/src/forked/agent/event.rs:110-282`
- `crates/agent/src/forked/agent/tool_exec.rs:3-25`
- `crates/agent/src/forked/agent/tool_exec.rs:305-341`
- `crates/agent/src/forked/agent/tool_exec.rs:470-579`

**Trigger:** The lead model invokes the `agent` tool without `subagent_type`, entering synchronous fork mode, and the forked model selects `write_file`, `edit_file`, or `exec` despite the prompt describing its Bash access as read-only.

**Control flow:** Message runtimes expose `LightweightRuntimeHandle` to the `agent` tool. Its fork builder disallows only `agent` and `spawn`, then supplies the standard fork schemas, which include file editing, file writing, and shell execution. No `can_use_tool` callback is supplied, so the builder defaults to `ToolPermission::Allow`. Forked tools execute in a separate dispatcher rather than `AgentRuntime::execute_tool_call`; consequently they do not evaluate ToolPolicy, PathPolicy, interactive confirmations, disabled-tool toggles, or `ToolRegistry` permission requirements. With no isolated `working_dir`, absolute paths are accepted and relative paths use the process working directory. The shell denylist covers only a small set of catastrophic command patterns and is not an authorization boundary.

**Impact:** A nested model call can perform filesystem writes or shell mutations that the parent runtime would deny or ask the user to confirm. This bypasses configured sensitive-path rules and channel/user permission policy, and turns prompt injection or model misbehavior inside fork mode into host/workspace modification capability.

**Repair direction:** Make fork capabilities structural rather than prompt-only. For the default fork route, pass an explicit read-only whitelist and deny `write_file`, `edit_file`, and `exec` unless a separately authorized mode requires them. Route any mutating fork operation through a parent-provided authorization callback that enforces the same disabled toggles, ToolPolicy, PathPolicy, confirmation, and origin permissions as `execute_tool_call`. Require an isolated working directory where applicable and fail closed when no authorization callback is installed. Add tests proving default fork schemas are read-only and that nested writes/exec cannot bypass a parent deny rule.

**Resolution:** Fixed. Default fork mode now uses one canonical runtime-enforced disallow list covering spawn, shell execution, file editing, and file writing; the same list filters advertised schemas and rejects forged/undeclared calls in the fork dispatcher. Typed agents retain their explicit configured capabilities. Regression test: `default_fork_capabilities_are_structurally_read_only`.

### R8 — Medium: Background-agent cancellation changes task state but does not stop active provider or tool work

**Locations:**

- `crates/agent/src/task_manager.rs:700-745`
- `crates/agent/src/runtime/subagent.rs:76-82`
- `crates/agent/src/runtime/subagent.rs:116-165`
- `crates/agent/src/runtime/fork_spawn.rs:276-279`
- `crates/agent/src/runtime/fork_spawn.rs:389-443`
- `crates/agent/src/forked/agent/run.rs:197-225`
- `crates/agent/src/forked/agent/run.rs:247-264`
- `crates/agent/src/forked/agent/run.rs:406-420`
- `crates/agent/src/forked/agent/tool_exec.rs:187-215`

**Trigger:** A user cancels a running ordinary subagent or typed agent while it is awaiting a provider response or executing a long-running forked tool, including an allowed `exec` command.

**Control flow:** `TaskManager::cancel_task` immediately marks the task `Cancelled` and cancels its registered `AbortToken`, but it does not own or abort the background task's `JoinHandle`. Ordinary subagent execution installs the token on a nested `AgentRuntime`, whose message/provider/tool loop never checks it. Forked/typed execution checks the token only before each turn and during provider-acquisition retry; both `provider.chat(...).await` and `execute_forked_tool(...).await` are uninterruptible by that token. The shell path wraps `Command::output()` in a timeout without `kill_on_drop`, so cancellation is not observed during the command and the timeout can drop the wait future without guaranteeing termination of the spawned process. Ordinary subagents subsequently call `set_completed`/`set_failed`; those state updates correctly preserve `Cancelled`, but the code still records delegation learning and persists/delivers the late result or failure. Typed agents suppress late result delivery after re-reading task state, but any tool side effects have already happened.

**Impact:** The UI/task API can report successful cancellation while provider usage and filesystem/shell side effects continue. A cancelled ordinary subagent can later send a contradictory completion/failure message and persist it into the parent session. Long-running child processes may outlive both cancellation and the nominal tool timeout.

**Repair direction:** Give each background agent a cancellation supervisor that owns its `JoinHandle`, and make provider/tool waits cancellation-aware with `tokio::select!`. Pass the token into `execute_forked_tool`; for subprocesses use an owned `Child`, `kill_on_drop(true)` or explicit process-group termination, and await cleanup. Before all ordinary-subagent learning, persistence, and delivery, re-check terminal task state and suppress work after cancellation. Add deterministic tests for cancellation during a pending provider, during a shell command, and before late result delivery.

**Evidence:** Control-flow proof above. Existing cancellation tests verify only token propagation and task-state protection; `cargo test -p blockcell-agent cancel -- --nocapture` passed 7 tests but contains no pending-provider/tool cancellation test.

**Resolution:** Fixed. `AbortToken` now provides a race-safe async cancellation wait. Ordinary subagents race cancellation against their message future and return before learning, persistence, or delivery. Forked/typed agents race cancellation against provider and tool futures, and forked shell commands use `kill_on_drop(true)` so dropping a cancelled or timed-out command future terminates its child. Regression tests: `forked_agent_cancels_pending_provider`, `forked_agent_cancellation_kills_running_shell_tool`, `cancelled_wait_completes_after_local_cancel`, and `cancelled_wait_completes_after_parent_cancel`.

### R9 — Medium: A full steering queue blocks the global inbound loop and prevents cancellation

**Locations:**

- `crates/agent/src/runtime/run_loop.rs:275-334`
- `crates/agent/src/runtime/run_loop.rs:416-425`
- `crates/agent/src/runtime/process_message_inner.rs:672-705`
- `crates/agent/src/runtime/message_dispatch.rs:299-332`
- `crates/agent/src/steering.rs:49-85`

**Trigger:** An active conversation remains inside provider streaming, retry sleep, or tool execution long enough to receive more than the steering channel's capacity of 16 additional messages.

**Control flow:** Steering is drained only immediately before the next LLM call. The single runtime inbound loop routes messages with `try_send`; when the queue is full, it awaits `sender.send(...)` inline. Until the active message task reaches the next steering drain, that await cannot complete. Because the same loop processes every conversation and the `/stop` and `/cancel-task` directives, no later inbound message or cancellation command can be handled during this interval.

**Impact:** One busy conversation can stall inbound processing for all conversations served by the runtime. The exact condition that most needs cancellation also prevents cancellation from being consumed, potentially extending the stall to the 300-second stream timeout, a long tool call, or indefinitely for an unbounded provider/tool future.

**Repair direction:** Never await steering backpressure in the global dispatcher. Use a documented bounded policy such as rejecting the newest message with an immediate busy response, coalescing pending steering, or moving per-conversation enqueueing to an isolated task. Process cancellation out of band from steering capacity, and make long provider/tool waits observe steering or cancellation where supported. Add a test that fills one conversation's steering queue and proves another conversation plus a cancellation directive remain responsive.

**Evidence:** Control-flow proof above. `cargo test -p blockcell-agent steering -- --nocapture` passed 7 tests, but the tests cover ordering, identity separation, and closed channels only; none exercises a full queue through the runtime loop.

**Resolution:** Fixed. Steering routing now uses a synchronous bounded outcome and rejects the newest message when the queue is full; the global inbound loop never awaits steering capacity and remains available for other conversations and cancellation directives. Regression test: `steering_queue_full_rejects_newest_without_waiting`.

### R10 — Medium: The active user request is silently truncated to 4,000 characters

**Locations:**

- `crates/agent/src/context.rs:770-812`
- `crates/agent/src/context.rs:942-965`
- `crates/agent/src/runtime/process_message_inner.rs:353-393`

**Trigger:** A user sends more than 4,000 Unicode characters in one request, such as source code, logs, a document, or detailed requirements whose relevant content lies in the removed middle section.

**Control flow:** Context construction always applies `trim_text_head_tail(user_content, 4000)` to the current user message before the first provider call, retaining roughly two thirds from the head and one third from the tail. This limit is independent of the configured token budget, model context window, attachment handling, and Layer-4 compaction. The original untrimmed text is then appended to persisted history, so the current turn sees an incomplete request while a later turn may see the full request.

**Impact:** The model can omit requirements, mis-review code, or answer from a syntactically corrupted document without the user being told that input was discarded. Persisting a different version than the one actually processed also makes later behavior and audit/replay inconsistent.

**Repair direction:** Budget the current user turn by estimated tokens after reserving system/tool/output space. Preserve it intact whenever it fits; otherwise use an explicit oversize-input contract such as attachment-backed retrieval, chunking, or a user-visible rejection. Persist metadata describing exactly what was sent to the provider if any transformation is unavoidable. Add a regression test with a unique marker in the middle of a request longer than 4,000 characters.

**Evidence:** Direct control-flow proof. The context test suite has no long-current-message case; `cargo test -p blockcell-agent context -- --nocapture` passed 28 tests without exercising this limit.

**Resolution:** Fixed. Current text and multimodal user messages now preserve `user_content` exactly instead of applying a fixed character trim. Regression test: `preserves_long_current_user_input`.

### R11 — Medium: Compact retention can create orphaned tool-result messages

**Locations:**

- `crates/agent/src/runtime/compaction.rs:65-75`
- `crates/agent/src/runtime/compaction.rs:162-176`
- `crates/agent/src/runtime/compaction.rs:92-134`
- `crates/agent/src/runtime/process_message_inner.rs:1167-1265`
- Compare safe-boundary logic: `crates/agent/src/context.rs:882-940`

**Trigger:** Mid-loop compaction runs after an assistant emits multiple tool calls and their tool results have been appended, while `keep_recent_messages` selects only a suffix of that assistant/tool group. With the default value of two, three tool results are sufficient for the retained suffix to contain only orphaned `tool` messages.

**Control flow:** `execute_layer4_compact` retains the last N individual messages with `rev().take(N)` and does not align the start to a complete user/assistant/tool boundary. `rebuild_messages_after_compact` then emits a compact system message, a synthetic continuation user message, and that raw suffix. The main loop immediately continues and sends this sequence to the provider. The separate context-history path already recognizes that a leading tool result or an assistant call missing any result is invalid and skips to a safe start, but compact retention does not reuse equivalent logic.

**Impact:** OpenAI-compatible providers can reject the next request because `tool_call_id` has no preceding assistant tool call. The malformed compacted form is also saved to session history, so the conversation may continue failing after restart until repaired or compacted again.

**Repair direction:** Retain complete conversational units rather than individual messages. Starting from the newest turn, include an assistant tool-call message only with every corresponding tool result, and never begin the retained suffix with a tool message. Share one protocol-boundary helper between normal context slicing and compaction. Add tests for multi-tool rounds with retention limits cutting at every position.

**Evidence:** Deterministic control-flow proof. The compact suite passed 42 unit tests plus 7 integration tests, but no test constructs a retained multi-tool suffix or validates provider message protocol after rebuilding.

**Resolution:** Fixed. Compact retention now selects a protocol-safe suffix: a boundary inside tool results expands backward to the declaring assistant message, while incomplete assistant/tool groups are omitted rather than persisted in malformed form. Regression test: `compact_recent_messages_preserve_complete_tool_group`.

### R12 — Medium: Compact recovery total budgets are not enforced

**Locations:**

- `crates/agent/src/compact/mod.rs:47-85`
- `crates/agent/src/compact/mod.rs:185-252`
- `crates/agent/src/compact/file_tracker.rs:90-109`
- `crates/agent/src/compact/skill_tracker.rs:65-87`
- `crates/core/src/config/memory.rs:606-635`

**Trigger:** A session loads many skills, or an administrator configures a file-recovery total below `max_single_file_tokens × max_files_to_recover`, then compaction builds its recovery message.

**Control flow:** `RecoveryBudget.max_file_recovery_tokens` is never read by `build_recovery_message`; file selection applies only file count and per-file truncation. `max_skill_recovery_tokens` is passed to `get_recent_skills` and `truncate_to_tokens` as a per-skill limit, while every tracked skill is included with no total accumulator cutoff. The function computes `total_tokens` only for logging after content has already been appended. Configuration validation explicitly treats these fields as total recovery budgets and warns about their sum, so runtime behavior violates the documented contract.

**Impact:** Recovery content can exceed the configured file or skill allocation, consume the space Layer 4 was intended to free, retrigger compaction repeatedly, or exceed the provider context window. Operators cannot reliably control post-compact context size through the exposed configuration.

**Repair direction:** Maintain separate remaining budgets for files, skills, and session memory. Admit recent entries only while their actually emitted truncated content fits, partially truncate the final admitted entry if useful, and stop afterward. Estimate the final combined summary, recovery, synthetic continuation, and retained recent messages before accepting compact success. Add tests that set very small total budgets and many files/skills, asserting emitted token estimates stay within each allocation.

**Evidence:** Direct control-flow proof. Existing `test_compact_recovery_token_budget` checks file count and per-item limits only; it does not call `build_recovery_message` with constrained total budgets or assert total emitted size.

**Resolution:** Fixed. File, skill, and session-memory recovery are now assembled independently inside their configured total token allocations, including section headings and Markdown framing. Entries are admitted newest-first and the last fitting entry is estimator-truncated; remaining entries are omitted once the allocation is exhausted. Regression test: `compact_recovery_enforces_total_budgets`.

### R13 — High: Background-task lifecycle results can be delivered to the wrong conversation

**Locations:**

- `crates/agent/src/task_manager.rs:315-381`
- `crates/agent/src/runtime/subagent.rs:29-40`
- `crates/agent/src/runtime/subagent.rs:138-174`
- `crates/agent/src/runtime/wiring.rs:496-531`
- `crates/agent/src/runtime/wiring.rs:533-590`

**Trigger:** Conversation A starts a background subagent task. Before it completes, the same runtime processes a normal inbound message from conversation B, making B the current `main_session_target`. The task then completes or fails and the system-event heartbeat runs.

**Control flow:** `run_subagent_task` records A's `origin_channel` and `origin_chat_id` and enables lifecycle system events. `TaskManager::emit_lifecycle_event` includes those origin fields only in `details`; it always constructs the event with `SystemEvent::new_main_session`, and a completed event's summary embeds the task result. `update_main_session_target` replaces the runtime target whenever another eligible conversation sends a message. Delivery of `EventScope::MainSession` resolves against that mutable latest target, so the lifecycle event from A is routed to B. The separate `deliver_subagent_result_to_origin` path correctly sends the direct result to A, but does not prevent the lifecycle summary copy from going to B.

**Impact:** Cross-conversation disclosure of background-task output or error details, plus misleading task notifications in the unrelated conversation. Gateway agents serving multiple chats are directly exposed to this race.

**Repair direction:** Scope task lifecycle events to their immutable origin with `EventScope::Session` or `EventScope::Channel`, preserving account/session identity as well as channel and chat ID. Summary queue items must retain and flush per target instead of collapsing all scopes into one main-session queue. Add a test that starts a task in chat A, switches the runtime target to chat B, completes the task, and asserts every notification and summary remains addressed to A.

**Evidence:** Deterministic control-flow proof. Existing lifecycle tests assert event kinds only, while runtime notification tests use one fixed main-session target and therefore do not exercise target rotation.

**Resolution:** Fixed. Event-producing tasks now persist their immutable origin account and session key; lifecycle events and summary items carry that scope through grouping and final dispatch. Target rotation no longer changes their destination. Regression tests: `task_lifecycle_event_stays_with_origin_after_target_rotation` and `task_summary_delivery_stays_with_origin_after_target_rotation`.

### R14 — Medium: Events are marked delivered before notification delivery succeeds

**Locations:**

- `crates/agent/src/system_event_orchestrator.rs:66-109`
- `crates/agent/src/runtime/wiring.rs:560-590`
- `crates/agent/src/runtime/wiring.rs:595-637`
- `crates/agent/src/summary_queue.rs:82-109`

**Trigger:** A critical notification or due summary is processed while its WebSocket broadcast has no receiver, its outbound channel is absent or closed, or the runtime shuts down between orchestration and dispatch.

**Control flow:** `SystemEventOrchestrator::process_tick` marks every selected event delivered before returning its delivery decision. The runtime dispatches afterward and ignores both broadcast and outbound send failures. Due summary items are also removed from the queue before dispatch. No failure path clears `delivered`, requeues the summary, or retries the request.

**Impact:** User-visible task failures and system summaries can be lost permanently while the store reports no pending work. Monitoring cannot distinguish successful delivery from an attempted or skipped send.

**Repair direction:** Separate selection from acknowledgement. Mark an event delivered only after the target transport accepts it; retain or requeue on failure and record attempts/backoff. Make dispatch return a delivery outcome, and flush summary items transactionally only after successful send. Add closed-channel and missing-target tests that assert the event remains pending.

**Evidence:** Direct control-flow proof. `cargo test -p blockcell-agent --test system_event_orchestrator -- --nocapture` passed 4 tests, but those tests explicitly expect events to become non-pending during `process_tick` and do not invoke a failing transport.

**Resolution:** Fixed. Orchestration now selects delivery candidates without acknowledging user-visible events. Runtime dispatch returns a transport outcome, marks events delivered only after success, and acknowledges summary items only after their scoped outbound succeeds. Repeated ticks deduplicate summary items by source event ID. Regression tests: `system_event_delivery_failure_keeps_pending` and `summary_delivery_failure_keeps_items`.

### R15 — Medium: Restart recovery silently converts interrupted tasks to Failed

**Locations:**

- `crates/agent/src/task_manager/persistence.rs:64-150`
- `bin/blockcell/src/commands/agent.rs:349-357`
- `bin/blockcell/src/commands/gateway.rs:827-838`
- `crates/agent/src/task_manager.rs:315-381`

**Trigger:** The process restarts while a persisted queued or running background task has `emit_system_events = true`.

**Control flow:** Startup calls `restore_from_disk` before the runtime registers its task event emitter. Recovery changes each unfinished task to `Failed`, writes the new state, and logs it, but does not call the lifecycle emitter, publish a completion event, or route a direct failure notification to the recorded origin. The restored task remains queryable until cleanup, so internal state says Failed while the user-facing notification state remains silent and `notified` remains false.

**Impact:** Users can wait indefinitely for a background task that was interrupted by restart unless they manually inspect `/tasks`. Automation consuming task lifecycle events also never observes the terminal transition.

**Repair direction:** Restore into an explicit interrupted/recovery state or produce a durable failure event after runtime wiring is ready. Replay one idempotent origin-scoped terminal notification, persist its notification/ack state, and cover both agent and gateway startup paths.

**Evidence:** Direct control-flow proof. `test_restore_from_disk_persists_failed_state` confirms the state rewrite but has no event or notification assertion; `cargo test -p blockcell-agent task_manager -- --nocapture` passed all 17 focused tests.

**Resolution:** Fixed. Restored interrupted tasks carry an explicit restart marker. After the matching agent emitter is registered, startup replays one origin-scoped failure lifecycle event, atomically marks the task notified, and persists that notification state. Regression test: `restored_interrupted_task_emits_failure_once`.

### R16 — Medium: Persisted terminal task files survive restarts indefinitely and can block unfinished-task recovery

**Locations:**

- `crates/agent/src/task_manager/persistence.rs:69-147`
- `crates/agent/src/task_manager/persistence.rs:198-241`
- `crates/agent/src/task_manager.rs:818-835`
- `crates/agent/src/task_manager.rs:844-870`

**Trigger:** The process stops after terminal task state has been persisted but before the in-process five-minute cleanup removes its JSON file. This repeats over many executions or restarts.

**Control flow:** `restore_from_disk` scans terminal JSON files but neither loads nor deletes them. Both cleanup paths derive deletion candidates exclusively from the in-memory task map, so skipped terminal files can never be selected after restart. The restore scan stops after 1,000 directory entries, without ordering unfinished files ahead of stale terminal files.

**Impact:** `.blockcell/tasks` grows across restarts, startup repeatedly parses stale records, and once the directory exceeds the scan cap an actually queued/running task can occur after the first 1,000 entries and never be converted to its recoverable failed state. Disk usage and task observability degrade over time.

**Repair direction:** During startup, classify every persisted record: restore interrupted tasks and delete or retain terminal records according to an explicit TTL based on `completed_at`. Apply the scan bound after filtering/prioritization, or sort unfinished records first. Add a restart test with terminal files plus an unfinished file beyond a small injected scan limit.

**Evidence:** Deterministic persistence/cleanup control-flow proof. Existing tests cover same-process file deletion and nonterminal restoration separately, but no test restarts with terminal records or exercises the scan limit.

**Resolution:** Fixed. Startup scanning deletes valid terminal task records immediately and counts only unfinished tasks against the 1,000-task recovery limit, while continuing to process entries one at a time. Regression test: `restore_deletes_terminal_files_without_consuming_limit`.

### R17 — Medium: Parent cancellation leaves ordinary subagents permanently Running

**Locations:**

- `crates/agent/src/runtime.rs:248-284`
- `crates/agent/src/runtime/subagent.rs:42-45`
- `crates/agent/src/runtime/subagent.rs:124-136`
- `crates/agent/src/task_manager.rs:608-702`

**Trigger:** An ordinary background subagent is running when its parent message token is cancelled because the parent message is replaced, explicitly stopped, or the runtime shuts down.

**Control flow:** `RuntimeSpawnHandle::spawn` creates a child token and supervises only JoinHandle failure. `run_subagent_task` creates the task in `Running`, registers that token, and races `token.cancelled()` against `process_message`. The cancellation branch unregisters the token and returns `()`, but does not call `cancel_task`, `set_failed`, or another terminal transition. The outer JoinHandle therefore completes successfully, so its panic-only supervisor also performs no fallback transition.

**Impact:** The task remains `Running` in TaskManager for the rest of the process, continues to appear unfinished in task listings and prompts, and emits no terminal lifecycle event. It is corrected only by restart recovery, which later rewrites it as interrupted/failed.

**Repair direction:** Make cancellation an explicit terminal path before returning, preferably through one idempotent TaskManager method that records `Cancelled`, unregisters the token, persists state, and emits the origin-scoped lifecycle event. The JoinHandle supervisor should also treat unexpected task cancellation/abort as terminal. Add a test that cancels the parent token rather than calling `TaskManager::cancel_task` and asserts the ordinary subagent reaches `Cancelled` exactly once.

**Evidence:** Deterministic control-flow proof. `cargo test -p blockcell-agent subagent -- --nocapture` passed 5 tests, but its abort-token test checks context propagation only and never observes TaskManager state after parent cancellation.

**Resolution:** Fixed. TaskManager now exposes one reason-aware cancellation terminal transition used by both user cancellation and parent-chain cancellation. Ordinary subagents record `Cancelled`, cancel/unregister their token, emit the lifecycle event, and persist the terminal state before returning. Regression test: `set_cancelled_records_reason_and_cancels_token`.

### R18 — Medium: A panicking detached memory extraction can suppress future extraction until process restart

**Locations:**

- `crates/agent/src/runtime/process_message_inner.rs:1664-1786`
- `crates/agent/src/runtime/process_message_inner.rs:1828-1967`
- `crates/agent/src/memory_system/mod.rs:362-466`
- `crates/agent/src/memory_system/mod.rs:545-602`
- `crates/agent/src/memory_system/mod.rs:885-945`

**Trigger:** A detached Session Memory or Auto Memory extraction task panics after its pending marker and journal are created but before the task reaches its tail cleanup.

**Control flow:** Both extraction paths discard their JoinHandle and remove marker/journal files only in normal task control flow. A panic skips that cleanup. The journal records only `owner_pid`; stale detection returns `false` unconditionally when that PID equals the current process, even after the configured stale threshold. Subsequent runtimes in the same long-lived Gateway process therefore interpret the orphaned marker as belonging to a still-running extraction and keep skipping that session or memory type.

**Impact:** Session-memory or one Auto Memory category can stop updating indefinitely in a live process after a single task panic. There is no task failure signal or self-recovery until the whole process exits and later stale-PID recovery becomes possible.

**Repair direction:** Give each extraction a unique owner token plus an in-process liveness registry, or supervise the JoinHandle and perform cleanup on panic. Put marker/journal cleanup in an RAII guard whose ownership token must match before deletion. Stale detection should distinguish a live process from a live extraction task. Add deterministic panic tests for both extraction paths and assert the next evaluation can schedule extraction again.

**Evidence:** Deterministic panic-path proof. `cargo test -p blockcell-agent memory_system -- --nocapture` passed 19 relevant tests, but none panics a detached extraction or tests same-process orphan recovery.

**Resolution:** Fixed. Detached Session Memory and Auto Memory extraction tasks now own an `ExtractionMarkerGuard` that removes marker/journal files during normal return, errors, and panic unwind. The existing Auto Memory cursor-save-failure path explicitly preserves both files for retry. Regression tests: `extraction_marker_guard_cleans_files_during_panic_unwind` and `extraction_marker_guard_preserve_keeps_files`.

### R19 — Low: ResponseCache capacity zero still stores one entry

**Locations:**

- `crates/agent/src/response_cache.rs:205-227`
- `crates/core/src/config/memory.rs:49-54`
- `crates/core/src/config/memory.rs:571-588`

**Trigger:** An operator configures `memorySystem.layer1.cacheMaxPerSession` to `0`, a value accepted without validation, and a cacheable tool result is processed.

**Control flow:** With `max_per_session == 0`, the capacity check is always true, but an empty session map has no oldest entry to remove. The code then unconditionally inserts the new entry and returns a cache stub. Later insertions evict the existing entry and insert another, so the effective capacity is one rather than zero.

**Impact:** The documented/configured attempt to disable per-session response caching does not work, and large content is replaced with a cache reference despite a zero-entry limit. Memory use remains bounded to one entry per active session, so severity is low.

**Repair direction:** Treat zero as disabled and return `None` before generating/inserting a cache entry, or reject/clamp zero during configuration validation with an explicit contract. Add a zero-capacity unit test.

**Evidence:** Direct boundary proof. `cargo test -p blockcell-agent response_cache -- --nocapture` passed 21 tests, with no zero-capacity case.

**Resolution:** Fixed. `maybe_cache_and_stub` now treats a zero per-session capacity as disabled before creating a cache reference or entry. Regression test: `zero_capacity_disables_response_cache`.

### M1 — High in multi-conversation deployments: Structured memory is automatically recalled across session boundaries

**Locations:**

- `crates/storage/src/memory.rs:66-112`
- `crates/storage/src/memory/query.rs:4-119`
- `crates/storage/src/memory/brief.rs:46-195`
- `crates/agent/src/memory_adapter.rs:66-90`
- `crates/agent/src/context.rs:473-488`
- `bin/blockcell/src/commands/gateway.rs:1163-1189`

**Trigger:** One Agent serves multiple conversations. Session A stores a structured memory containing private or session-specific data; session B later submits a semantically related query.

**Control flow:** Records store `channel` and `session_key`, but `QueryParams` has neither field and both raw SQL and hybrid retrieval omit conversation/subject filtering. Context construction automatically calls `generate_brief_for_query` and injects the result into every general/skill prompt for that Agent. Gateway creates one memory store per Agent, not per conversation.

**Impact:** Information written from one conversation can be injected into another conversation's model context and potentially disclosed. A global `dedup_key` can also update a row created by another session while retaining the original row's channel/session attribution because the dedup update does not update those fields.

**Repair direction:** Define an explicit memory ownership model. If memory may be private, add subject/session/tenant identity to the schema contract, unique indexes, query filters, vector metadata, automatic brief generation, and tool operations. Require explicit promotion for Agent-global memories. If single-owner Agent memory is the intended contract, enforce it at channel authorization boundaries and document that limitation prominently.

**Resolution:** Fixed with session-private ownership plus explicit global rows. Session queries return matching rows and rows with `session_key IS NULL`; automatic briefs and `memory_query` pass the current session key. Active dedup uniqueness and updates are session-aware. Regression tests: `query_is_scoped_to_session_and_includes_explicit_global_memory`, `equal_dedup_keys_in_different_sessions_create_distinct_rows`, and `memory_brief_receives_current_session_key`.

### M2 — Medium: Deleted or expired FTS candidates can crowd active memories out of hybrid retrieval

**Locations:**

- `crates/storage/src/memory/query.rs:122-159`
- `crates/storage/src/retriever.rs:34-80`
- `crates/storage/src/memory/schema.rs:91-116`

**Trigger:** More than the finite candidate window of highly relevant matching memories are deleted or expired, while an active matching memory ranks below them.

**Control flow:** `search_fts_candidates` selects and limits candidates without filtering `deleted_at` or `expires_at`. Filtering happens only after the finite candidate list is merged and loaded. Soft deletion updates the row but leaves its searchable title/summary/content in the external-content FTS table. Invalid candidates can therefore consume the entire FTS window before active filtering.

**Impact:** Relevant active memories disappear from search and automatic context briefs even though they are present in canonical storage.

**Repair direction:** Apply active/deleted/expiry predicates before candidate limiting, or over-fetch iteratively until enough valid candidates are obtained. Add a regression test with high-scoring deleted/expired rows ahead of a valid row.

**Resolution:** Fixed. FTS candidate SQL now applies deletion, expiry, session, scope, type, tags, and time filters before ordering and limiting. Regression test: `fts_filters_deleted_candidates_before_applying_candidate_limit`.

### M3 — Medium: Crash-stale learning lock directories block all future file-memory or skill writes

**Locations:**

- `crates/agent/src/memory_file_store.rs:397-438`
- `crates/agent/src/skill_file_store.rs` (`FileWriteGuard` implementation)
- `crates/agent/src/write_guard.rs:90-112`

**Trigger:** The process terminates after creating `.memory_file_store.lockdir` or `.skill_file_store.lockdir` but before the guard's `Drop` runs.

**Control flow:** Cross-process exclusion is implemented by creating a directory and deleting it only in `Drop`. On `AlreadyExists`, the writer retries until a ten-second deadline, but the lock contains no owner PID/lease and there is no stale-owner recovery. `WriteGuard` is process-local; its `lockdir_base` field is explicitly reserved for future use and cannot recover the filesystem lock.

**Impact:** All later learned-memory or skill mutations fail repeatedly until an operator manually removes the stale directory.

**Repair direction:** Store owner identity and lease metadata, check whether the owning process is alive, and safely recover stale locks. Reuse the stronger stale-PID handling already implemented by `auto_memory::CrossProcessLock`, or consolidate on a single lock primitive.

**Resolution:** Fixed. Memory and skill stores now share `OwnerAwareFileLock`, which records PID plus a unique owner token, recovers only dead-owner directories, waits for live owners, and removes a lock only when the token still matches. Regression tests: `recovers_lock_directory_owned_by_dead_pid` and `refuses_to_steal_lock_directory_owned_by_live_pid`.

### M4 — High in multi-conversation deployments: Memory mutation operations do not enforce session ownership

**Locations:**

- `crates/tools/src/memory.rs:571-618`
- `crates/tools/src/lib.rs:275-289`
- `crates/agent/src/memory_adapter.rs:93-123`
- `crates/storage/src/memory/maintenance.rs:5-40`
- `crates/storage/src/memory/maintenance.rs:43-165`
- `crates/storage/src/memory/maintenance.rs:168-213`

**Trigger:** One Agent serves multiple conversations. A caller invokes `memory_forget` with another session's memory UUID, invokes `restore` with that UUID, or invokes `batch_delete` with broad or empty filters.

**Control flow:** Query and upsert tools pass `ctx.session_key`, but the mutation trait exposes single-item delete/restore by ID only and batch deletion without caller ownership. The adapter drops session identity, and the storage SQL has no `session_key` predicate. An empty `batch_delete` filter is explicitly accepted and selects every active row in the Agent-wide store. Explicit global rows are equally mutable by any session that can address them. Stats mode also returns Agent-wide counts without session scoping.

**Impact:** A conversation can soft-delete or restore another conversation's private structured memory, and an unfiltered call can delete all active structured memories for the Agent. This violates the session-private ownership contract introduced for M1 and creates cross-conversation integrity and availability loss.

**Repair direction:** Carry the caller session and an explicit global-memory administration capability through every mutation API. Apply ownership predicates inside the same SQL statement that mutates the row; do not rely on a prior read. Reject empty batch filters for ordinary tool callers and scope every batch operation to the caller. Apply the same policy to restore and expose session-scoped stats to non-administrative callers.

**Evidence:** Direct end-to-end control-flow proof. Existing tests cover unscoped delete/restore and accept an empty `batch_delete`, but do not exercise cross-session mutation denial.

**Resolution:** Fixed. Ordinary memory tools now call session-scoped delete, restore, batch-delete, and stats APIs whose safe defaults reject unsupported scoped access. The adapter applies ownership predicates in the mutation SQL, excludes explicit global rows from ordinary mutation, and rejects empty batch filters. Administrative CLI/maintenance APIs remain explicitly unscoped. Regression tests: `scoped_mutations_reject_other_sessions_and_global_rows`, `batch_delete_is_scoped_to_caller_session`, and `memory_forget_batch_delete_forwards_caller_session`.

### M5 — Medium: Negative `top_k` bypasses the documented 50-result limit

**Locations:**

- `crates/tools/src/memory.rs:267-290`
- `crates/tools/src/memory.rs:305-316`
- `crates/agent/src/memory_adapter.rs:66-87`
- `crates/storage/src/retriever.rs:19-81`
- `crates/storage/src/memory/query.rs:101`
- `crates/storage/src/memory/query.rs:205-209`

**Trigger:** A model or caller supplies a negative integer such as `top_k: -1` to `memory_query`.

**Control flow:** Tool validation accepts every value. `.min(50)` preserves negative integers, after which the adapter casts the `i64` directly to `usize`, turning `-1` into `usize::MAX`. Hybrid retrieval then requests a saturated candidate window, binds `top_k as i64` as `-1` to SQLite FTS (`LIMIT -1` means no limit), requests an effectively unbounded vector result count, and truncates final results at `usize::MAX`.

**Impact:** The caller bypasses the documented maximum of 50 and can scan/return the entire structured-memory corpus, increasing database, vector-search, serialization, prompt, and memory costs. The empty-query raw-SQL path may instead fail on an out-of-range LIMIT literal, making behavior inconsistent by query shape.

**Repair direction:** Validate `top_k` as `1..=50` at the tool boundary, use checked conversion in the adapter, and clamp or reject unsafe values again in the storage API so non-tool callers cannot bypass the contract.

**Evidence:** Deterministic integer-conversion and SQLite LIMIT proof. No test covers zero/negative/over-limit values across tool, adapter, and storage boundaries.

**Resolution:** Fixed. Tool validation accepts only `1..=50`; the adapter uses checked signed-to-unsigned conversion with a safe default; storage preserves explicit zero-result behavior and caps every raw/hybrid query at 50. Regression tests: `test_memory_query_validate` and `negative_top_k_never_becomes_unbounded`.

### M6 — Medium: Global vector candidate truncation can hide valid session memories

**Locations:**

- `crates/storage/src/retriever.rs:34-81`
- `crates/storage/src/retriever.rs:85-109`
- `crates/storage/src/vector.rs:5-31`
- `crates/storage/src/memory/vector_sync.rs:200-212`
- `crates/storage/src/rabitq_index.rs:283-319`
- `crates/storage/src/rabitq_index.rs:347-389`

**Trigger:** A shared Agent store contains more than the finite vector candidate window of highly similar rows owned by other sessions, while the current session has a lower-ranked semantic-only match.

**Control flow:** `VectorMeta` and the RabitQ row contain scope, type, and tags, but not `session_key`. Vector search therefore ranks and truncates globally at `max(top_k * 4, 20)`. Only after this truncation are canonical rows loaded and filtered by `item_matches_query`. FTS can recover lexical matches, but a semantic-only current-session row outside the global vector window is never considered.

**Impact:** Relevant private memories can deterministically disappear from `memory_query` and automatic prompt briefs because unrelated sessions crowd the vector candidate window. The final canonical filter prevents direct disclosure, but retrieval correctness degrades as other sessions accumulate similar content.

**Repair direction:** Persist ownership metadata in the vector index and filter before candidate truncation, or iteratively over-fetch until enough ownership-valid candidates are collected. Cover private plus explicit-global semantics in the vector contract and migrations.

**Evidence:** Direct ranking/filter-order proof. Existing session-isolation coverage exercises canonical/FTS retrieval but not a vector window dominated by other sessions.

**Resolution:** Fixed. Vector metadata and RabitQ rows now carry `session_key`; `VectorFilter` applies session/global ownership plus scope/type/tag filters before truncation. Hybrid retrieval also iteratively over-fetches and validates canonical rows, preserving correctness for legacy vectors that lack ownership metadata and for stale entries. Regression tests: `vector_candidates_are_session_filtered_before_limit` and `rabitq_filters_session_before_result_limit`.

### M7 — Medium: Vector reindex is not a crash-recoverable or concurrency-safe state transition

**Locations:**

- `crates/storage/src/memory/maintenance.rs:350-397`
- `crates/storage/src/memory/maintenance.rs:400-428`
- `crates/storage/src/memory/vector_sync.rs:173-213`

**Trigger:** The process exits after `reindex_vectors` resets the external index and clears the durable sync queue, or a memory is deleted after the reindex snapshot is loaded but before that row is upserted.

**Control flow:** Reindex resets the live index in place, clears every pending vector intent, snapshots active canonical rows, and then rebuilds one row at a time. There is no durable rebuild epoch or complete per-row intent before reset. A crash after queue clearing leaves unprocessed canonical rows with no recovery work. During a concurrent delete, the delete path can remove the vector and clear its queue entry before reindex later upserts the stale snapshot, leaving a deleted vector with no pending delete.

**Impact:** After interruption, semantic retrieval can remain empty or incomplete until an operator runs another full reindex. Concurrent mutations can also leave stale vectors that consume finite candidate slots and amplify M6, even though canonical filtering prevents deleted rows from being returned directly.

**Repair direction:** Model rebuild as a durable epoch/state machine: record rebuild intent before reset, retain recoverable work for every canonical row, re-read each row immediately before indexing, coordinate mutations by generation/lock, and preferably build a new index before atomically switching it live.

**Evidence:** Deterministic operation-order and snapshot-race proof. Current tests cover successful rebuild and per-item upsert failure, but not crash points or concurrent delete/update.

**Resolution:** Fixed. Reindex now loads active canonical rows, transactionally seeds a durable upsert intent for every row before reset, retains the queue throughout rebuild, and re-reads canonical state before and after external upsert. Rows deleted/expired during rebuild are deleted from the vector index, while repeatedly changing rows retain retryable intent. Regression tests: `reindex_seeds_durable_intents_before_reset` and `reindex_deletes_item_that_becomes_inactive_during_upsert`.

### M8 — Medium: Substring tag matching can batch-delete memories with different tags

**Locations:**

- `crates/storage/src/memory/query.rs:56-68`
- `crates/storage/src/memory/query.rs:176-189`
- `crates/storage/src/memory/query.rs:286-295`
- `crates/storage/src/memory/maintenance.rs:77-86`

**Trigger:** A caller queries or batch-deletes tag `go` while a memory is tagged `mongodb`, or uses another tag that is a substring of a stored tag or the serialized JSON text.

**Control flow:** Tags are stored as serialized text. SQL filtering uses `tags LIKE '%' || ? || '%'`, and post-vector filtering uses `tag.contains(wanted)`, so neither path compares complete tag values. The same substring predicate selects rows for destructive batch deletion.

**Impact:** Queries return incorrectly tagged rows, and `memory_forget` batch deletion can soft-delete memories that do not carry any requested tag. The destructive effect makes this more than a ranking-quality issue.

**Repair direction:** Normalize tags into a relation or use a queryable JSON array with exact element comparison. Keep query and mutation semantics identical and add overlapping-tag regression cases such as `go` versus `mongodb` and `prod` versus `production`.

**Evidence:** Direct predicate proof. The only batch-tag test verifies that either of two distinct exact tags is selected; it does not test overlapping values.

**Resolution:** Fixed. SQLite query and batch-delete predicates now compare comma-delimited complete tokens with `instr`, and canonical/vector post-filters use string equality. Regression tests: `tag_query_requires_exact_membership` and `batch_delete_tag_requires_exact_membership`.

### M9 — High in multi-conversation deployments: Ghost learning promotes private episodes into Agent-global file memory

**Locations:**

- `crates/agent/src/runtime/learning.rs:639-694`
- `crates/agent/src/ghost_background_review.rs:24-26`
- `crates/agent/src/ghost_background_review.rs:424-495`
- `crates/agent/src/ghost_background_review.rs:580-618`
- `crates/agent/src/memory_file_store.rs:65-76`
- `crates/agent/src/ghost_recall.rs:20-35`
- `crates/agent/src/ghost_recall.rs:79-110`

**Trigger:** One Agent serves multiple conversations or users. A Ghost episode from session A contains a private preference, correction, project fact, or other reusable-looking detail and is selected for background review.

**Control flow:** The episode ledger retains `session_key` and `subject_key`, but the restricted reviewer can write only through `memory_manage` into the single Agent-wide `USER.md` or `MEMORY.md`. The tool context replaces the original identity with the constant `ghost_background_review`, and `MemoryFileStore` has no session/subject namespace. Later Ghost recall reads the same two files without filtering by the current session and injects matching content into any eligible conversation for that Agent. There is no explicit promotion decision, redaction step, or global-memory authorization boundary between a private episode and the shared files.

**Impact:** Information learned from one conversation can be injected into another conversation's model context and potentially disclosed. This recreates the cross-session confidentiality problem fixed for structured memory in M1, but through the Ghost file-memory path.

**Repair direction:** Define private versus Agent-global ownership for file memory. Preserve the episode subject/session through review tool context, default learned entries to session-private storage, and require an explicit policy-approved promotion for global `USER.md`/`MEMORY.md`. Apply current-session filtering during recall and add a two-session regression test proving that a lesson from A is absent from B unless promoted.

**Evidence:** Direct end-to-end control-flow proof plus the passing `ghost_learning_closes_loop_from_experience_to_file_memory_only` test, which confirms that episode review writes the shared file-memory layer. Existing tests use one conversation and do not exercise cross-session recall.

**Resolution:** Fixed. Automatic Ghost reviews now open a stable-hash session-scoped `MemoryFileStore` and preserve the episode session in tool context. Recall merges only the current session's files with explicit global `USER.md`/`MEMORY.md`; another session's automatic memory is not searched. Regression tests: `ghost_session_file_memory_is_isolated_by_session_key`, `ghost_session_recall_merges_global_but_not_other_sessions`, and the updated `ghost_learning_closes_loop_from_experience_to_file_memory_only`.

### M10 — Medium: Per-message coordinator lifetime disables cross-turn nudges and global review throttling

**Locations:**

- `crates/agent/src/runtime/message_task.rs:107-155`
- `crates/agent/src/runtime.rs:2293-2310`
- `crates/agent/src/runtime/process_message_inner.rs:33-37`
- `crates/agent/src/runtime/process_message_inner.rs:644-657`
- `crates/agent/src/learning_dedup.rs:16-26`
- `crates/agent/src/learning_throttle.rs:12-26`

**Trigger:** A normal gateway/daemon conversation sends the default three or more user turns, or several conversations finish nudge-eligible work concurrently.

**Control flow:** `run_message_task` constructs a fresh `AgentRuntime` for every inbound message. The `SkillNudgeEngine`, `LearningDedup`, and `LearningThrottle` are constructed inside that Runtime and store all state only in process memory. Each message therefore starts with zero user turns, records exactly one turn, and is dropped before the next message; the default Memory Nudge soft threshold of three can never be reached on this path. The ten-minute dedup window, two-review concurrency limit, and five-minute completion cooldown are likewise isolated per message task, so they do not deduplicate or throttle reviews created by other messages.

**Impact:** Turn-based self-improvement review is silently unavailable on the primary per-message runtime path, while iteration-based reviews can bypass the intended Agent-wide storm controls when multiple messages run concurrently. Unit tests pass because they reuse one coordinator across several synthetic turns, unlike production.

**Repair direction:** Move nudge counters, dedup state, and throttle state to an Agent-scoped shared service keyed by the intended conversation/Agent ownership contract, or persist the counters where restart continuity is required. Pass that shared handle into each per-message Runtime. Add a production-shaped test that invokes separate message tasks for successive turns and a concurrency test spanning separate Runtime instances.

**Evidence:** Direct lifecycle proof. `test_memory_nudge_after_turns` and throttle/dedup unit tests pass within one long-lived coordinator but do not construct a new Runtime per turn.

**Resolution:** Fixed. The long-lived run-loop Runtime now passes its shared `Arc<LearningCoordinator>` into every per-message Runtime, so turn counters, deduplication, concurrency slots, and cooldown state survive message-task replacement. Regression test: `shared_learning_coordinator_accumulates_turns_across_message_runtimes`.

### M11 — Medium: Losing a Ghost review lease does not stop already-running side effects

**Locations:**

- `crates/agent/src/ghost_background_review.rs:285-353`
- `crates/agent/src/ghost_background_review.rs:424-495`
- `crates/storage/src/ghost_ledger.rs:382-507`
- `crates/storage/src/ghost_ledger.rs:524-541`
- `crates/storage/src/ghost_ledger.rs:571-660`

**Trigger:** A background review runs longer than the 600-second lease while its heartbeat loses ownership or exits after three storage failures. Another worker later cleans up and reclaims the same episode.

**Control flow:** The heartbeat task only logs and exits on lost ownership or repeated failure; it does not cancel or notify `run_background_review_for_episode`. The restricted tool loop continues provider calls and executes `memory_manage` side effects without checking lease ownership before or after each action. Owner-aware ledger finalization prevents the stale worker from inserting the final review run, but it happens after the file-memory mutations. A replacement worker can therefore review and write for the same episode again.

**Impact:** One episode can produce duplicate or conflicting add/replace/remove operations, and the durable audit can show only the winning worker even though the stale worker already changed memory. Exact duplicate adds are partially idempotent, but replacements, removals, and different model outputs are not.

**Repair direction:** Couple lease ownership to execution cancellation. Expose heartbeat loss through a cancellation token, check ownership immediately before every side-effecting tool call and after slow provider calls, and stop without further writes when ownership is uncertain. For stronger recovery, persist idempotency keys or stage mutations and commit them with the review run.

**Evidence:** Direct control-flow proof. Ledger tests cover claim, expiry cleanup, and owner-aware finalization, but no test loses a lease while a tool loop is active.

**Resolution:** Fixed. Each claimed review owns an `AbortToken`; heartbeat ownership loss or repeated heartbeat failure cancels it. Provider waits select against cancellation, and the tool loop checks the token again before every tool action. The initial heartbeat also now requires `Ok(true)` rather than treating `Ok(false)` as success. Regression test: `ghost_review_stops_after_lease_loss_before_memory_write`.

### M12 — High: SkillFileStore follows symlinked skill paths outside the skills root

**Locations:**

- `crates/agent/src/skill_file_store.rs:82-123`
- `crates/agent/src/skill_file_store.rs:198-221`
- `crates/agent/src/skill_file_store.rs:284-313`
- `crates/agent/src/skill_file_store.rs:595-620`
- `crates/agent/src/skill_file_store.rs:649-710`
- `crates/agent/src/skill_file_store.rs:860-872`
- `crates/agent/src/forked/agent/tool_exec.rs:809-850`

**Trigger:** A symlinked category, skill directory, `SKILL.md`, or auxiliary subdirectory exists below the configured skills directory and points elsewhere. The main Agent or a review Agent then views or mutates that skill.

**Control flow:** Target validation is lexical. Existing-target resolution uses `is_dir`, `exists`, and recursive directory traversal, all of which follow symlinks, then accepts the lexical path with `strip_prefix` instead of comparing canonical paths. Reads follow linked files and directories; writes through a linked parent directory are created outside the skills root. Atomic replacement of a linked leaf replaces that leaf link, but only after the existing external target may already have been read and snapshotted. `copy_dir_recursive` skips symlinks only while snapshotting/restoring; normal resolution, `collect_files`, and mutations do not reject them. Forked reviews prefer this `SkillFileStore` path whenever the handle is present.

**Impact:** Skill operations can read files outside the skills root, recurse through external directories or cycles, and overwrite external files through a linked parent directory. This bypasses the intended skill path boundary and can turn a learned-skill write into an arbitrary filesystem write within the process's OS permissions.

**Repair direction:** Canonicalize the skills root and every existing target, reject any symlink in the target chain, and verify the canonical target remains beneath the canonical root before reads, recursion, snapshots, deletion, or writes. For new files, validate the nearest existing canonical ancestor and use no-follow/open-at-style primitives where available.

**Evidence:** Direct filesystem-semantics proof. Current traversal tests cover `..` and absolute-style names only; the 23 passing SkillFileStore tests contain no symlink case.

**Resolution:** Fixed. `SkillFileStore` canonicalizes its skills root, rejects symbolic links in every existing target component, verifies canonical targets remain beneath the root, skips symlinks during discovery/listing, and validates auxiliary destinations before mutation. Regression tests: `skill_file_store_rejects_symlinked_skill_directory` and `skill_file_store_rejects_symlinked_auxiliary_parent`.

### M13 — High: Skill patch safety scanning checks only the replacement fragment

**Locations:**

- `crates/agent/src/skill_file_store.rs:198-221`
- `crates/agent/src/skill_file_store/patch.rs:8-59`
- `crates/agent/src/unified_security_scanner.rs:22-46`
- `crates/agent/src/forked/agent/tool_exec.rs:809-841`
- `crates/tools/src/security_scan.rs:409-415`

**Trigger:** Existing skill text contains a benign prefix and a patch supplies a separately benign fragment that becomes unsafe only after composition, for example changing `Ignore previous PLACEHOLDER` so the final file contains `Ignore previous instructions`.

**Control flow:** `SkillFileStore::patch` normalizes and scans only the replacement `content`, then composes `next` with the existing file and writes it without scanning the result or the full skill directory. The scanner's injection rules operate on complete phrases, so split payloads can evade fragment scanning. Forked review execution routes `skill_manage patch` through this store when available, bypassing its older fallback path that correctly scans the composed `new_content`.

**Impact:** An Agent-created learned skill can persist prompt injection, dangerous commands, credential-access instructions, or other content that the AgentCreated trust policy is intended to block. The unsafe skill can later be injected into prompts or executed as procedure content.

**Repair direction:** Scan the fully composed `next` content before snapshot/write, and run directory-level scanning when an auxiliary file or multi-file relationship can create the unsafe condition. Keep one shared mutation implementation so the tool and store paths cannot diverge.

**Evidence:** Deterministic composition proof. Existing tests verify unsafe full create/edit content and patch matching, but do not test a payload formed across the old/new boundary.

**Resolution:** Fixed. Patch now composes the final candidate first and runs the AgentCreated safety scan against that complete content before snapshot or write. Regression test: `skill_file_store_patch_scans_composed_content`.

### M14 — Medium: Skill restore overlays snapshots and leaves post-snapshot files active

**Locations:**

- `crates/agent/src/skill_file_store.rs:323-366`
- `crates/agent/src/skill_file_store.rs:430-474`
- `crates/agent/src/skill_file_store.rs:875-900`

**Trigger:** A skill gains a new auxiliary file after a snapshot, such as `scripts/new.py`, and the operator or Agent invokes `restore_latest` to roll back to the older snapshot that does not contain that file.

**Control flow:** Restore creates the destination directory and recursively copies snapshot entries over it, but never removes or atomically replaces the current skill directory. Files present only in the current version survive the restore. The operation also does not security-scan the final restored directory.

**Impact:** Rollback reports success while newly added behavior remains active. A faulty or unsafe script can survive an attempted recovery, and subsequent loading sees a hybrid state that never existed in any snapshot.

**Repair direction:** Restore into a fresh sibling directory, security-scan the complete candidate, then atomically swap it into place while retaining the current snapshot for undo. Add a regression test proving destination-only files disappear.

**Evidence:** Direct copy semantics proof. Existing restore tests verify that snapshot files are copied back, but do not assert removal of files absent from the snapshot.

**Resolution:** Fixed. Every mutation snapshot now captures the complete skill directory. Restore builds and safety-scans a fresh sibling candidate, renames the live directory aside, swaps the candidate into place, and removes the replaced directory only after commit. Destination-only files therefore disappear. Regression tests: `skill_file_store_restore_is_exact_and_removes_new_files` and `skill_file_store_restore_is_exact_and_scans_snapshot`.

### M15 — Medium: Skill mutations can report failure after partially committing filesystem state

**Locations:**

- `crates/agent/src/skill_file_store.rs:154-195`
- `crates/agent/src/skill_file_store.rs:198-257`
- `crates/agent/src/skill_file_store.rs:284-320`
- `crates/agent/src/skill_file_store.rs:477-502`

**Trigger:** A secondary step fails after the primary content write, such as `meta.yaml` creation, re-enabling the toggle file, or deleting `.skills_prompt_snapshot.json`.

**Control flow:** Create first makes the live directory and writes `SKILL.md`, then writes metadata and cache/toggle state. Edit, patch, and auxiliary writes similarly commit the primary file before updating toggle/cache state. There is no staging directory, rollback guard, or committed-state marker. The caller receives `Err` even though the skill may already be created or changed; a create retry is then rejected because the partial directory exists, while a patch retry may apply against already-mutated content.

**Impact:** Failures are ambiguous and retries are not safe. Reviews can be recorded as failed even though they changed active skill state, and partially created skills can require manual repair.

**Repair direction:** Stage complete create operations and atomically rename them into place. For mutations, define a transaction/commit order in which post-write cache invalidation is infallible or recoverable, and roll back from the snapshot when a required secondary update fails. Return an explicit committed-with-warning result for non-critical cache cleanup failures rather than a generic failure.

**Evidence:** Direct ordered-write proof. Existing mutation tests cover successful invalidation and normal snapshots, not injected failures between steps.

**Resolution:** Fixed. Create writes and scans a complete staging directory before one live-directory rename. After committed create/edit/patch/write/restore/delete operations, toggle and prompt-snapshot maintenance failures are logged without turning a visible commit into `Err`; parent-sync and replaced-directory cleanup failures follow the same committed-with-warning rule. Regression test: `skill_file_store_post_commit_cache_failure_is_non_fatal`.

## Module 5 Task 5 architecture risks and missing coverage

- `MemoryFileStore::restore_latest` restores snapshot text without re-running the learned-memory security scan or the current character budget. Snapshots normally originate from previously accepted content, but external modification or legacy snapshots can reintroduce unsafe/oversized memory.
- `LearningCoordinator::evaluate_nudge` calls `check_skill_nudge` twice in one branch. No production caller currently uses this method, so it is recorded as dormant correctness debt rather than a live defect.
- Ghost review side effects and ledger audit are not one transaction. A process crash after a file write but before review-run insertion leaves an unaudited mutation and makes retry semantics dependent on the action's accidental idempotency.
- Coverage now closes cross-session Ghost recall, per-message Runtime nudge accumulation, active lease-loss cancellation, symlinked skills, split-payload patch scanning, destination-only rollback files, unsafe snapshot restore, and post-commit cache failure. A real process crash between external file mutation and ledger audit remains untested.

## Module 5 Task 5 reviewed areas with no confirmed defect

- Ghost policy is refreshed from the live Runtime configuration before turn/delegation/evolution decisions; no stale-policy path was confirmed.
- Pending episode claim and completed/failed review-run insertion use transactional owner-aware ledger updates, preventing a stale worker from overwriting the winning ledger state at finalization.
- Provider lifecycle uses shutdown guards, and review failure paths record failed runs when ownership remains valid.
- Restricted Ghost background review exposes only `memory_manage`, `session_search`, and `skill_view`; it cannot modify skills directly.
- Learning reservation/completion RAII balances throttle slots on early return, task cancellation, and panic unwind within one coordinator instance.
- File-memory add/replace/remove normal paths scan newly supplied content, require unique replacement/removal matches, serialize read-modify-write with process and owner-aware filesystem locks, snapshot before mutation, and use durable atomic replacement.
- Skill target names and auxiliary relative paths reject lexical traversal components and constrain ordinary auxiliary files to approved subdirectories; the remaining escape is the unresolved symlink boundary in M12.
- Skill snapshots skip symlink entries, and successful writes use durable temporary-file replacement.

### M16 — High: Core evolution executes unaudited LLM-generated shell directly on the host

**Locations:**

- `crates/tools/src/system_info.rs:551-568`
- `crates/skills/src/core_evolution/generation.rs:54-124`
- `crates/skills/src/core_evolution.rs:900-1032`
- `crates/skills/src/core_evolution.rs:1035-1192`

**Trigger:** The model requests a Process/BuiltIn capability and the generation provider returns syntactically valid shell containing destructive filesystem access, credential reads, network exfiltration, process spawning, or another host-side effect.

**Control flow:** `system_info(action="request")` lets the model enqueue a capability description. Core evolution sends that description to an LLM, extracts the returned code, and performs only `bash -n` before writing an executable artifact. `validate_artifact` then launches the generated script with ordinary `bash` on the daemon host and sends `{}` to stdin. It does not call the skill static audit, the unified security scanner, a permission policy, a restricted tool layer, or a sandbox. The ten-second timeout wraps `wait_with_output`; dropping that future does not configure `kill_on_drop`, so a timed-out child can continue running after validation returns.

**Impact:** Prompt injection or a compromised generation model can obtain arbitrary command execution with the Blockcell process's OS permissions during validation, before any user invokes the capability. A forked or daemonized child can survive the validation timeout.

**Repair direction:** Treat generated code as untrusted. Apply a deterministic policy before artifact creation, execute validation in an OS sandbox with an empty/minimal environment, read-only workspace, no network, bounded resources, and an explicit allowlist, and kill the entire process group on timeout/cancellation. Require explicit user approval for privilege-bearing capability classes.

**Evidence:** Direct control-flow proof. The Skills suite tests extraction, syntax, provider timeouts, and registry behavior, but has no test asserting that generated shell cannot access host files/network or survive validation timeout.

**Resolution:** Fixed. Core evolution now applies deterministic static policy before writing generated artifacts, and shell validation is syntax-only rather than executing generated code on the host. Regression test: `evolution_rejects_dangerous_generated_process_code`.

### M17 — High: Prompt-only skill evolution bypasses the learned-skill prompt-injection scanner

**Locations:**

- `crates/skills/src/audit.rs:80-123`
- `crates/skills/src/audit.rs:241-288`
- `crates/skills/src/service.rs:730-910`
- `crates/skills/src/evolution/versioning.rs:23-78`

**Trigger:** An evolution prompt or adjacent/historical skill content causes the LLM to generate a `SKILL.md` containing prompt injection, deceptive instructions, context-exfiltration directions, or another payload that is long enough and contains Markdown headings.

**Control flow:** The evolution pipeline's deterministic audit for `PromptTool` calls only `check_prompt_only` plus size/empty checks. Unlike `SkillFileStore`, it does not use the unified AgentCreated skill scanner. The independent LLM audit reviews content but is not a deterministic trust boundary. After compile/contract checks, `create_new_version` writes `patch.diff` directly to live `SKILL.md`.

**Impact:** Self-evolution can persist instructions that later enter the Agent prompt and influence tool use or disclose context, bypassing the scanner enforced for ordinary learned-skill writes.

**Repair direction:** Route every evolution candidate through the same AgentCreated directory-level security policy used by guarded skill writes, scan the complete final skill tree immediately before commit, and reject rather than retry indefinitely on policy violations that cannot be safely transformed.

**Evidence:** Direct policy comparison. Existing prompt-only audit tests cover only minimum length and headings; no evolution test covers prompt injection or context exfiltration.

**Resolution:** Fixed. PromptTool static audit now blocks deterministic prompt-injection and system-prompt exfiltration instructions before deployment. Regression test: `evolution_rejects_prompt_injection_in_prompt_only_skill`.

### M18 — High: Skill names are joined into live and version paths without validation

**Locations:**

- `crates/skills/src/service.rs:1670-1725`
- `crates/skills/src/evolution/lifecycle.rs:105-137`
- `crates/skills/src/evolution/versioning.rs:17-43`
- `crates/skills/src/versioning.rs:301-312`

**Trigger:** A manual or externally constructed evolution context supplies a special skill name such as `..`, an absolute path, or a multi-component relative path.

**Control flow:** `trigger_manual_evolution`, `trigger_external_evolution`, and `trigger_evolution` do not apply the single-component skill-name validation used by version import and guarded skill mutation. Deployment joins `record.skill_name` directly below the selected skill root, and VersionManager repeats the same unchecked joins for live files, history, versions, cleanup, and rollback. For example, `skills_dir.join("..")` resolves to the workspace parent while the generated evolution record ID remains a valid single filename.

**Impact:** A crafted evolution can overwrite, snapshot, clear, restore, or delete files outside the skills directory under the process's permissions. Rollback and staged cleanup enlarge the possible destructive scope.

**Repair direction:** Validate the skill identifier once at every public trigger boundary, require exactly one normal path component, canonicalize the skills root, reject symlinked target chains, and revalidate persisted records before every filesystem mutation.

**Evidence:** Deterministic path semantics. Import has explicit component validation, demonstrating the intended rule, but the evolution triggers and VersionManager do not share it.

**Resolution:** Fixed. Evolution triggers and all VersionManager path boundaries share a one-normal-component skill-name validator before joining filesystem paths. Regression tests: `evolution_rejects_parent_directory_skill_name` and `version_manager_rejects_parent_directory_skill_name`.

### M19 — Medium: Evolution workers continue side effects after losing their workflow lease

**Locations:**

- `crates/scheduler/src/evolution_worker.rs:182-206`
- `crates/scheduler/src/evolution_worker.rs:439-483`
- `crates/scheduler/src/skill_evolution_worker.rs:178-199`
- `crates/scheduler/src/skill_evolution_worker.rs:455-496`

**Trigger:** A generation, compile, validation, load, or full skill pipeline step outlives its lease while heartbeat renewal returns false or fails three consecutive times; another worker recovers and reclaims the workflow.

**Control flow:** Both heartbeat tasks only log and exit. They do not signal the active engine/pipeline future. The stale worker continues writing records/artifacts, executing validation, registering capabilities, deploying skills, or creating versions. The post-step ownership check discards only the workflow-store result after those side effects have happened.

**Impact:** Two workers can execute the same non-idempotent evolution side effects, producing conflicting versions, duplicate validation execution, stale registry activation, or a live deployment with no matching winning workflow audit.

**Repair direction:** Couple heartbeat ownership to an abort token, select every long-running provider/process/engine operation against it, check immediately before filesystem/registry mutations, and make step commits idempotent under a durable step key.

**Evidence:** Direct control-flow proof; the workers have no focused tests, including no lease-loss-during-step case.

**Resolution:** Fixed. Both evolution workers expose heartbeat lease loss through a cancellation receiver and select active pipeline work against it, stopping the stale future before result commit. Regression test: `evolution_lease_loss_cancels_pending_step`.

### M20 — Medium: Restart promotes every persisted evolved capability past canary

**Locations:**

- `crates/skills/src/capability_provider.rs:401-441`
- `crates/skills/src/capability_provider.rs:490-536`
- `crates/skills/src/capability_provider.rs:717-749`
- `crates/skills/src/capability_provider.rs:752-804`

**Trigger:** The process restarts while a newly evolved capability is still `Available`/Observing and has fewer than five canary calls, or has accumulated errors below the evaluation point.

**Control flow:** Registration stores canary counters only in the in-memory `canary_trackers` map. `save` persists descriptors but not trackers/lifecycle. On restart, `load` restores descriptors and `rehydrate_executors` rebuilds every persisted executor, sets its lifecycle to Active, and changes its descriptor status to Active without replaying or restarting canary.

**Impact:** Restart is a promotion bypass. An unproven or already erroring capability becomes fully Active without meeting `CANARY_MIN_CALLS`, and the lost observations cannot trigger automatic rejection.

**Repair direction:** Persist lifecycle and canary totals atomically with the descriptor, restore Observing capabilities as Observing, and continue or conservatively restart the canary after executor rehydration.

**Evidence:** Direct restart-state proof. Registry tests cover in-process canary behavior and executor replacement, not save/load during canary.

**Resolution:** Fixed. Rehydration keeps persisted Available evolved capabilities in Observing and starts a fresh conservative canary tracker instead of promoting them to Active. Regression test: `canary_restart_keeps_evolved_capability_observing`.

### M21 — Medium: Skill durable workflows never leave Observing after success or rollback

**Locations:**

- `crates/scheduler/src/skill_evolution_worker.rs:202-215`
- `crates/scheduler/src/skill_evolution_worker.rs:281-313`
- `crates/skills/src/service.rs:993-1018`
- `crates/skills/src/service.rs:1175-1228`

**Trigger:** A deployed skill reaches the end of its observation window, either passing and becoming Completed or exceeding the error threshold and becoming RolledBack.

**Control flow:** The worker changes the SQLite workflow to `Observing` when deployment starts. Later observation ticks run inside `EvolutionService` and update only the JSON evolution record plus in-memory maps. They have no workflow-store handle and never change the durable workflow to Promoted, Failed, or RolledBack. `workflow_blocks_enqueue` treats Observing as permanently active.

**Impact:** Durable workflow status and audit remain stale forever, operational listings report work still observing, and the row permanently blocks re-enqueue decisions keyed to that evolution ID.

**Repair direction:** Return explicit observation outcomes to the scheduler or inject a workflow-status callback, then atomically finalize the durable workflow on Completed/RolledBack and record the terminal observation metrics.

**Evidence:** Repository-wide search finds the sole skill-workflow `Observing` write and no subsequent terminal update.

**Resolution:** Fixed. Observation ticks now reconcile terminal JSON records back into the durable workflow store: Completed becomes Promoted, while RolledBack/Failed becomes Failed with terminal context. Regression test: `observation_workflow_maps_record_terminal_status`.

### M22 — Medium: Concurrent observation reports lose persisted calls and can misclassify rollout health

**Locations:**

- `crates/skills/src/service.rs:1265-1300`
- `crates/skills/src/service.rs:1165-1175`
- `crates/skills/src/evolution/versioning.rs:413-453`

**Trigger:** Two or more calls to the same observing skill finish concurrently, especially across Runtime instances or processes.

**Control flow:** Each reporter loads the same JSON record, increments counters in its private copy, and replaces the record file. There is no per-record lock, compare-and-swap, or append-only event. One write can overwrite another. Once the persisted total is nonzero, observation evaluation prefers the persisted counters over the process-local tracker, so the in-memory count cannot repair the loss.

**Impact:** Total/error counts and error rate are inaccurate. A bad rollout can be promoted because error calls were overwritten, or a good rollout can be rolled back from a distorted small sample.

**Repair direction:** Store observation increments in a transactional database or owner-locked append log, aggregate atomically, and make evaluation consume one canonical counter source across processes.

**Evidence:** Direct lost-update interleaving. Existing tests exercise `ObservationStats` sequentially only.

**Resolution:** Fixed. Observation record load/increment/save is serialized by a stale-aware cross-process owner lock, preserving one canonical persisted count. Regression test: `observation_concurrent_reports_preserve_all_counts`.

### M23 — Medium: Imported and restored skill versions have no authenticity or safety verification

**Locations:**

- `crates/skills/src/versioning.rs:324-349`
- `crates/skills/src/versioning.rs:628-658`
- `crates/skills/src/versioning.rs:726-863`

**Trigger:** A user imports a tampered archive, or a local/legacy version snapshot is modified after creation, then switches or rolls back to it.

**Control flow:** Import validates archive shape, link type, and size, but trusts `version.json`, does not recompute/compare its hash, has no signature/trust policy, and does not run the learned-skill security scan. Restore copies the selected snapshot into the live skill tree without verifying hash or scanning the complete candidate. The stored MD5 covers only the primary file at creation and is not enforced during restore.

**Impact:** A modified snapshot can activate prompt injection or executable code while retaining apparently valid version metadata. Version history provides chronology but not artifact authenticity or policy compliance.

**Repair direction:** Define trust levels for Manual/Evolution/Import, hash the complete canonical tree with a modern digest, verify it before restore, require signatures for external packages where applicable, and run the full directory security policy before atomic activation.

**Evidence:** Direct import/restore control-flow proof. Tests cover traversal, symlinks, size, and nested asset preservation, not signature/hash mismatch or unsafe restored content.

**Resolution:** Fixed. New snapshots store a sorted complete-tree SHA-256 digest; restore requires and verifies it, while import verifies supplied digests when present and security-audits the complete extracted candidate before storing it. Regression tests: `version_integrity_rejects_tampered_snapshot` and `version_integrity_rejects_unsafe_import`.

### M24 — Medium: Capability activation reports success after registry persistence or rollback snapshot failure

**Locations:**

- `crates/skills/src/core_evolution.rs:1195-1262`
- `crates/skills/src/core_evolution.rs:459-485`

**Trigger:** `evolved_tools.json` cannot be written or capability version snapshot creation fails after the executor has been registered in memory.

**Control flow:** `load_capability` registers the descriptor/executor first, logs and ignores `registry.save()` failure, then logs and ignores `create_version_if_new_artifact()` failure. It returns success, and `run_step` marks the CoreEvolutionRecord Active; the following Promote step is a no-op.

**Impact:** The workflow can be marked Promoted while restart loses the capability, rollback has no snapshot, or audit claims activation that is only process-local. Retrying can encounter an already-mutated registry with incomplete durable state.

**Repair direction:** Treat registry persistence and initial version snapshot as required transaction steps. Stage/verify the snapshot first, persist registry atomically, roll back the in-memory registration on failure, and only then mark Active/Promoted.

**Evidence:** Direct ignored-error paths; no failure-injection test covers either persistence boundary.

**Resolution:** Fixed. Capability activation now requires a rollback snapshot before registry mutation, persists registration as a transaction, restores any previous in-memory state on save failure, and propagates the error. Regression test: `capability_activation_failure_does_not_leave_registered_executor`.

### M25 — Medium: Skill version operations are not serialized and history writes are not atomic

**Locations:**

- `crates/skills/src/versioning.rs:47-122`
- `crates/skills/src/versioning.rs:124-278`
- `crates/skills/src/versioning.rs:609-658`

**Trigger:** Two processes or services create, switch, clean up, or roll back versions of the same skill concurrently; this can occur after lease loss/reclaim or through concurrent administrative operations.

**Control flow:** Unlike `CapabilityVersionManager`, `VersionManager` has no per-skill process/file lock. Each operation independently reads `version_history.json`, chooses a version number, mutates snapshots/live files, and writes history with plain `std::fs::write`. Concurrent creates can both choose the same `vN`; cleanup can delete a snapshot being restored; a crash during history write can truncate the only history file.

**Impact:** Version entries and snapshots can overwrite each other, current_version can point to the wrong or missing tree, rollback can restore a hybrid/incorrect state, and the recovery path itself can become unavailable.

**Repair direction:** Add a per-skill cross-process owner lock around the full mutation, use unique staging directories and atomic/fsynced history replacement, and recover incomplete swap journals before each operation.

**Evidence:** Direct read-modify-write interleaving. Capability versioning already implements the required locking/journaling pattern; skill versioning tests are single-threaded.

**Resolution:** Fixed. Skill version create/switch/rollback/cleanup/import operations hold a per-skill owner lock across the complete read-modify-write interval, and history uses unique fsynced temporary files plus atomic replacement. Regression test: `version_concurrent_creates_preserve_history`.

## Module 5 Task 6 architecture risks and missing coverage

- Skill observation uses a fixed 60-minute window and can complete with zero calls; no minimum sample threshold is required before promotion.
- Scheduled Ghost's `max_syncs_per_day` counter is process-local, so restart resets the daily quota; dispatch failures also consume quota before the message is accepted.
- Core evolution supports DynamicLibrary as an advertised provider even though loading falls back to launching the `.dylib` path as a process; this is incomplete behavior rather than a newly proven privilege escalation.
- Focused coverage now exercises lease-loss cancellation, observation terminal workflow synchronization, canary restart behavior, concurrent observation/version mutations, snapshot integrity, unsafe import, and partial activation rollback. A real process crash during a live skill-tree restore remains untested.

## Module 5 Task 6 reviewed areas with no confirmed defect

- Skill pipeline restart normalization maps interrupted Generating/Auditing/CompileFailed records back to a replayable checkpoint while preserving generated patch/feedback where available.
- Static skill audit blocks several direct deletion, shell, eval, oversized-content, and wrong-language patterns before the independent LLM audit.
- Skill snapshot copy and version import reject symlinks and special archive entries; import also constrains entries to one skill root and enforces entry/file/total size limits.
- CapabilityVersionManager uses per-capability file locks, atomic history replacement, rollback journals, and executor rebinding after rollback.
- Core workflow step selection distinguishes database query failure from legitimate completion, and workflow-store finalization checks lease ownership.
- Scheduled Ghost hot reload retries failed reads, updates schedule state after configuration changes, and its prompt explicitly forbids skill creation.

## Module 5 Task 4 architecture risks and missing coverage

- `restore` immediately enqueues and performs a vector upsert without checking whether `expires_at` is already in the past. Canonical retrieval filters the row, but the stale vector can occupy finite candidate windows until maintenance.
- `SessionStore::append` appends a message without refreshing the outer metadata `updated_at`. Its production caller persists background subagent results this way, while Layer 2 inactivity compaction relies on that timestamp. Add a contract test before deciding whether background delivery should count as session activity.
- Vector coverage still lacks restore-of-expired-row behavior and a real process-termination test in the middle of reindex; durable reset-failure and mutation-during-upsert cases now cover the corresponding state transitions deterministically.

## Module 5 Task 4 reviewed areas with no confirmed defect

- SQLite `memory_items` remains canonical; FTS triggers update inside the canonical transaction.
- Upsert uses a write transaction, and canonical row changes plus vector-sync intent are committed together before external vector mutation.
- Soft delete, batch delete, maintenance, and normal restore persist retryable vector intent before best-effort external synchronization.
- Failed vector upserts/deletes remain in `memory_vector_queue`, and retry re-reads canonical state before choosing upsert versus delete.
- Active deduplication is isolated by `dedup_key + COALESCE(session_key, '')`, including concurrent store handles.
- RabitQ SQLite rows are the vector source of truth; reopening marks non-empty indexes dirty and lazily rebuilds the binary cache.
- Session filenames use reversible safe encoding, and session save uses file locking, unique temporary files, fsync, and atomic replacement.
- Auto Memory cursor writes use process-local serialization, a cross-process owner lock, and atomic replacement; detached extraction markers now clean up during panic unwinding.

## Closed regression gaps

- Same raw `chat_id` across two channels.
- Same raw `chat_id` across two account IDs.
- Message-task panic and aborted JoinHandle supervision.
- Deferred review reservation followed by empty response or early `?` return.
- Session-scoped structured-memory recall and deduplication.
- FTS candidate windows dominated by deleted/expired rows.
- Recovery from stale file-memory and skill lock directories.
- Background task lifecycle notification and summary routing after main-session rotation.
- Immediate-event and summary transport failure retry state.
- Idempotent notification of tasks interrupted by restart.
- Startup cleanup of terminal task records without consuming the unfinished-task recovery limit.

## Open architecture risks and missing coverage

- `DeliveryPolicy.persist` and `max_delay_seconds` remain unenforced by the in-memory event store. Pending events and summary items can still disappear on process restart even though the default policy requests persistence.
- `CapabilityRegistryHandle` is an outer `Arc<tokio::Mutex<dyn CapabilityRegistryOps>>`. Callers hold that mutex while awaiting `execute_capability`, and the adapter then awaits an external capability executor after releasing only its inner concrete-registry lock. This serializes all capability-registry operations behind the full duration of one capability execution. No lock cycle was found, so this is recorded as a head-of-line blocking risk rather than a confirmed deadlock.

## Review progress

- Module 2 Task 1 lifecycle and early-return/error audit: complete.
- Runtime lifecycle findings R1-R3: fixed and regression-tested.
- Runtime lifecycle findings R4-R5: fixed and regression-tested after the completed early-return/error audit.
- Module 2 Task 2 authorization, path, confirmation, inheritance, subagent restriction, and policy-reload review: complete; R6-R7 fixed and regression-tested.
- Module 2 Task 2 cancellation, steering, forked-agent work, and cleanup review: complete; R8-R9 fixed and regression-tested.
- Module 2 Task 3 context ordering, truncation, compact recovery budgets, summaries, and file/skill tracking review: complete; R10-R12 fixed and regression-tested.
- Module 2 Task 3 task state, restart recovery, event emission, notification routing, shared state, spawned-task ownership, shutdown, and cache-key review: complete. R13-R19 are fixed and regression-tested.
- Module 5 structured-memory and learning-lock findings M1-M3: reviewed, fixed, and regression-tested.
- Module 5 Task 4 persistence, retrieval, session isolation, vector synchronization, crash consistency, and mutation-contract review: complete. M4-M8 are fixed and regression-tested.
- Module 5 Task 5 Ghost decisions, background review, ledger leases, dedup/throttle lifetime, guarded file writes, snapshots, rollback, traversal, and security scanning: complete. M9-M15 are fixed and regression-tested.
- Module 5 Task 6 skill/core evolution generation, audit, compilation, versioning, deployment, canary, restart recovery, rollback, worker leases, and scheduled Ghost review: complete. M16-M25 are fixed and regression-tested.
- The implementation and verification record is in `docs/superpowers/plans/2026-07-25-skill-evolution-lifecycle-fixes.md`.
