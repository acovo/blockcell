# Agent Runtime and Learning Systems Review

**Date:** 2026-07-24
**Status:** Runtime findings remediated; review in progress
**Plan:** `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`

## Verification baseline

- `cargo test -p blockcell-storage -- --nocapture`: 59 passed, 0 failed.
- `cargo test -p blockcell-agent runtime -- --nocapture`: 121 passed, 0 failed; 491 filtered out.

Passing tests establish the current baseline. The remediated cross-channel/account routing,
message-task panic, and deferred-review cleanup cases now have regression coverage. The
non-WebSocket error-delivery and streamed-provider success-accounting cases now also have
regression coverage.

Final remediation verification:

- `cargo test -p blockcell-storage -p blockcell-tools -p blockcell-agent --all-targets`: all passed (Agent 609, Storage 62, Tools 408, plus integration targets).
- `cargo test -p blockcell --bin blockcell websocket -- --nocapture`: 14 passed, 0 failed.
- `cargo fmt --all -- --check`: passed.

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

## Closed regression gaps

- Same raw `chat_id` across two channels.
- Same raw `chat_id` across two account IDs.
- Message-task panic and aborted JoinHandle supervision.
- Deferred review reservation followed by empty response or early `?` return.
- Session-scoped structured-memory recall and deduplication.
- FTS candidate windows dominated by deleted/expired rows.
- Recovery from stale file-memory and skill lock directories.

## Review progress

- Module 2 Task 1 lifecycle and early-return/error audit: complete.
- Runtime lifecycle findings R1-R3: fixed and regression-tested.
- Runtime lifecycle findings R4-R5: fixed and regression-tested after the completed early-return/error audit.
- Module 5 structured-memory and learning-lock findings M1-M3: reviewed, fixed, and regression-tested.
- The implementation and verification record is in `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review-fixes.md`.
