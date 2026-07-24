//! Learning Coordinator — unified decision point for all learning operations
//!
//! Replaces scattered `skill_nudge_engine` + `ghost_memory_lifecycle` calls
//! in runtime.rs with a single coordinated decision flow.
//!
//! Key guarantees:
//! - Same turn never triggers two independent reviews
//! - Throttle prevents review storms (max concurrent + cooldown)
//! - Dedup prevents duplicate learning within a time window
//! - Combined review when both memory and skill nudges fire

use std::sync::{Arc, Mutex};

use crate::ghost_learning::{GhostLearningPolicy, LearningDecision};
use crate::learning_dedup::LearningDedup;
use crate::learning_throttle::LearningThrottle;
use crate::skill_nudge::{NudgeResult, SkillNudgeEngine};

/// What kind of learning review to trigger
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearningAction {
    /// No learning needed this turn
    Skip,
    /// Review user memory (declarative knowledge)
    MemoryReview { trigger: MemoryTrigger },
    /// Review skill library (procedural knowledge)
    SkillReview { trigger: SkillTrigger },
    /// Review both memory and skills in a single pass
    CombinedReview {
        memory_trigger: MemoryTrigger,
        skill_trigger: SkillTrigger,
    },
}

/// Why a memory review was triggered
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryTrigger {
    /// Nudge threshold reached
    NudgeThreshold { count: u32 },
    /// Ghost learning boundary decision
    GhostBoundary,
    /// Pre-compress flush
    PreCompress,
    /// Session end
    SessionEnd,
    /// Session rotate
    SessionRotate,
    /// Delegation end
    DelegationEnd { success: bool },
}

/// Why a skill review was triggered
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillTrigger {
    /// Nudge threshold reached
    NudgeThreshold { count: u32 },
    /// Evolution trigger (skill error pattern)
    EvolutionTrigger {
        skill_name: String,
        error_count: u32,
    },
}

/// Unified learning coordinator
///
/// Wraps SkillNudgeEngine + GhostLearningPolicy + throttle + dedup
/// into a single decision point.
pub struct LearningCoordinator {
    nudge_engine: Mutex<SkillNudgeEngine>,
    ghost_policy: Mutex<GhostLearningPolicy>,
    throttle: LearningThrottle,
    dedup: LearningDedup,
    ghost_learning_enabled: bool,
    self_improve_review_enabled: bool,
}

pub struct LearningReviewReservationGuard {
    coordinator: Arc<LearningCoordinator>,
    reserved: bool,
}

impl LearningReviewReservationGuard {
    pub fn new(coordinator: Arc<LearningCoordinator>) -> Self {
        Self {
            coordinator,
            reserved: true,
        }
    }

    pub fn into_completion_guard(mut self) -> LearningReviewCompletionGuard {
        self.reserved = false;
        LearningReviewCompletionGuard {
            coordinator: Arc::clone(&self.coordinator),
        }
    }
}

impl Drop for LearningReviewReservationGuard {
    fn drop(&mut self) {
        if self.reserved {
            self.coordinator.cancel_review();
        }
    }
}

pub struct LearningReviewCompletionGuard {
    coordinator: Arc<LearningCoordinator>,
}

impl Drop for LearningReviewCompletionGuard {
    fn drop(&mut self) {
        self.coordinator.review_completed();
    }
}

impl std::fmt::Debug for LearningCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LearningCoordinator")
            .field("ghost_learning_enabled", &self.ghost_learning_enabled)
            .field(
                "self_improve_review_enabled",
                &self.self_improve_review_enabled,
            )
            .field(
                "ghost_policy",
                &self.ghost_policy.lock().unwrap_or_else(recover_mutex),
            )
            .finish()
    }
}

fn recover_mutex<T>(e: std::sync::PoisonError<T>) -> T {
    tracing::warn!("LearningCoordinator Mutex poisoned, recovering");
    e.into_inner()
}

impl LearningCoordinator {
    pub fn new(
        nudge_engine: SkillNudgeEngine,
        ghost_policy: GhostLearningPolicy,
        throttle: LearningThrottle,
        dedup: LearningDedup,
        ghost_learning_enabled: bool,
        self_improve_review_enabled: bool,
    ) -> Self {
        Self {
            nudge_engine: Mutex::new(nudge_engine),
            ghost_policy: Mutex::new(ghost_policy),
            throttle,
            dedup,
            ghost_learning_enabled,
            self_improve_review_enabled,
        }
    }

    /// Called at the start of each user turn
    ///
    /// Replaces:
    /// - `ghost_memory_lifecycle.on_turn_start()`
    /// - `skill_nudge_engine.record_user_turn()`
    pub fn on_turn_start(&self, is_real_user: bool) {
        if is_real_user {
            self.nudge_engine
                .lock()
                .unwrap_or_else(recover_mutex)
                .record_user_turn();
        }
    }

    /// Record a tool iteration (each LLM call + tool execution)
    ///
    /// Replaces: `skill_nudge_engine.record_iteration()`
    pub fn record_iteration(&self) {
        self.nudge_engine
            .lock()
            .unwrap_or_else(recover_mutex)
            .record_iteration();
    }

    /// Evaluate what learning action to take at turn end
    ///
    /// Called before the LLM loop to check memory nudge,
    /// and after each iteration to check skill nudge.
    /// Returns the combined action.
    ///
    /// Replaces:
    /// - `skill_nudge_engine.check_memory_nudge()` + `check_skill_nudge()`
    /// - `deferred_review_mode` logic
    pub fn evaluate_nudge(
        &self,
        has_memory_store: bool,
        has_skill_tool: bool,
        existing_action: Option<&LearningAction>,
    ) -> LearningAction {
        if !self.self_improve_review_enabled {
            return LearningAction::Skip;
        }

        if !self.throttle.try_start_review() {
            return LearningAction::Skip;
        }

        // First, check thresholds WITHOUT resetting counters (read-only check)
        // This allows dedup to block the review without losing counter values
        let mut engine = self.nudge_engine.lock().unwrap_or_else(recover_mutex);
        let turns_before = engine.turns_since_memory();
        let iterations_before = engine.iterations_since_skill();

        let memory_due = engine.would_memory_nudge() && has_memory_store;
        let skill_due = engine.would_skill_nudge() && has_skill_tool;

        // Dedup check — BEFORE resetting counters
        let dedup_key = if memory_due && skill_due {
            "combined_nudge".to_string()
        } else if memory_due {
            "memory_nudge".to_string()
        } else if skill_due {
            "skill_nudge".to_string()
        } else {
            self.throttle.cancel_review(); // rollback — no nudge due
            return existing_action.cloned().unwrap_or(LearningAction::Skip);
        };

        if self.dedup.is_duplicate(&dedup_key) {
            // Counters NOT reset — next turn will still see the same counts
            self.throttle.cancel_review(); // rollback — dedup blocked
            return existing_action.cloned().unwrap_or(LearningAction::Skip);
        }

        // Now that dedup passed, actually trigger the nudge (which resets counters).
        // Only call check_*_nudge() for the nudges that are actually due,
        // to avoid resetting counters for nudges that aren't due yet.
        // Previously, both check_memory_nudge() and check_skill_nudge() were
        // called unconditionally, which could reset the skill counter even
        // when only a memory nudge was due (and vice versa).
        if memory_due {
            let _memory_nudge = engine.check_memory_nudge();
        }
        if skill_due {
            let _skill_nudge = engine.check_skill_nudge();
        }

        // Determine action — use pre-captured counts
        let new_action = if memory_due && skill_due {
            LearningAction::CombinedReview {
                memory_trigger: MemoryTrigger::NudgeThreshold {
                    count: turns_before,
                },
                skill_trigger: SkillTrigger::NudgeThreshold {
                    count: iterations_before,
                },
            }
        } else if memory_due {
            LearningAction::MemoryReview {
                trigger: MemoryTrigger::NudgeThreshold {
                    count: turns_before,
                },
            }
        } else if skill_due {
            LearningAction::SkillReview {
                trigger: SkillTrigger::NudgeThreshold {
                    count: iterations_before,
                },
            }
        } else {
            self.throttle.cancel_review(); // rollback — no nudge due after actual check
            return existing_action.cloned().unwrap_or(LearningAction::Skip);
        };

        // Note: counters are already reset by check_memory_nudge()/check_skill_nudge()
        // when they trigger, so no additional reset needed here.

        // If there's an existing action, merge/upgrade — use actual counts from new_action
        match (&new_action, existing_action) {
            (
                LearningAction::SkillReview {
                    trigger: new_skill_trigger,
                },
                Some(LearningAction::MemoryReview {
                    trigger: mem_trigger,
                }),
            ) => LearningAction::CombinedReview {
                memory_trigger: mem_trigger.clone(),
                skill_trigger: new_skill_trigger.clone(),
            },
            (
                LearningAction::MemoryReview {
                    trigger: new_mem_trigger,
                },
                Some(LearningAction::SkillReview {
                    trigger: skill_trigger,
                }),
            ) => LearningAction::CombinedReview {
                memory_trigger: new_mem_trigger.clone(),
                skill_trigger: skill_trigger.clone(),
            },
            _ => new_action,
        }
    }

    /// Check memory nudge only (called before LLM loop)
    ///
    /// Returns the memory action if a memory nudge is due.
    /// This is separate from evaluate_nudge because memory nudge
    /// is checked once before the loop, while skill nudge is
    /// checked each iteration.
    pub fn check_memory_nudge(&self, has_memory_store: bool) -> Option<MemoryTrigger> {
        if !self.self_improve_review_enabled {
            return None;
        }

        // 先用只读判断检查阈值和资源可用性，避免阈值已到但资源不可用时消耗计数器
        {
            let engine = self.nudge_engine.lock().unwrap_or_else(recover_mutex);
            if !engine.would_memory_nudge() || !has_memory_store {
                return None;
            }
        }

        // throttle 检查必须在 dedup 写入之前完成，
        // 否则 throttle 失败时 dedup 已写入，导致后续同类 nudge 被误判重复
        if !self.throttle.try_start_review() {
            return None;
        }

        // dedup 只读检查：如果 key 已存在，rollback throttle 并返回
        if self.dedup.contains_recent("memory_nudge") {
            self.throttle.cancel_review();
            return None;
        }

        // throttle 和 dedup 都通过，确认 review 会启动，现在写入 dedup 记录
        self.dedup.record("memory_nudge");

        let mut engine = self.nudge_engine.lock().unwrap_or_else(recover_mutex);
        // Capture count before check resets it
        let turns_before = engine.turns_since_memory();
        let memory_nudge = engine.check_memory_nudge();

        if memory_nudge == NudgeResult::NoNudge {
            self.throttle.cancel_review(); // rollback the try_start_review increment
            return None;
        }

        // Use pre-captured count (check_memory_nudge already reset the counter)
        Some(MemoryTrigger::NudgeThreshold {
            count: turns_before,
        })
    }

    /// Check skill nudge only (called each iteration)
    ///
    /// Returns the skill action if a skill nudge is due,
    /// potentially upgrading an existing memory action to combined.
    ///
    /// If `existing_memory` is true, the caller already reserved a throttle
    /// slot via `check_memory_nudge`, so we skip `try_start_review()` to
    /// avoid double-counting (which would leak the throttle counter).
    pub fn check_skill_nudge(
        &self,
        has_skill_tool: bool,
        existing_memory: bool,
    ) -> Option<SkillTrigger> {
        if !self.self_improve_review_enabled {
            return None;
        }

        // 先用只读判断检查阈值和资源可用性，避免阈值已到但资源不可用时消耗计数器
        {
            let engine = self.nudge_engine.lock().unwrap_or_else(recover_mutex);
            if !engine.would_skill_nudge() || !has_skill_tool {
                return None;
            }
        }

        // throttle 检查必须在 dedup 写入之前完成，
        // 否则 throttle 失败时 dedup 已写入，导致后续同类 nudge 被误判重复
        // Only reserve a new throttle slot if no memory review is already pending.
        // If existing_memory is true, the caller's memory slot covers this review too.
        if !existing_memory && !self.throttle.try_start_review() {
            return None;
        }

        // dedup 只读检查：如果 key 已存在，rollback throttle 并返回
        let dedup_key = if existing_memory {
            "combined_nudge"
        } else {
            "skill_nudge"
        };
        if self.dedup.contains_recent(dedup_key) {
            if !existing_memory {
                self.throttle.cancel_review();
            }
            return None;
        }

        // throttle 和 dedup 都通过，确认 review 会启动，现在写入 dedup 记录
        self.dedup.record(dedup_key);

        let mut engine = self.nudge_engine.lock().unwrap_or_else(recover_mutex);
        // Capture count before check resets it
        let iterations_before = engine.iterations_since_skill();
        let skill_nudge = engine.check_skill_nudge();

        if skill_nudge == NudgeResult::NoNudge {
            if !existing_memory {
                self.throttle.cancel_review(); // rollback only if we reserved a slot
            }
            return None;
        }

        // Use pre-captured count (check_skill_nudge already reset the counter)
        Some(SkillTrigger::NudgeThreshold {
            count: iterations_before,
        })
    }

    /// Reset skill counter after a skill write tool is used
    ///
    /// Replaces: `skill_nudge_engine.reset_skill()`
    pub fn reset_skill(&self) {
        self.nudge_engine
            .lock()
            .unwrap_or_else(recover_mutex)
            .reset_skill();
    }

    /// Reset memory counter after a memory write tool is used
    ///
    /// Replaces: `skill_nudge_engine.reset_memory()`
    pub fn reset_memory(&self) {
        self.nudge_engine
            .lock()
            .unwrap_or_else(recover_mutex)
            .reset_memory();
    }

    /// Record that a review has started (for throttle tracking)
    pub fn review_started(&self) {
        self.throttle.review_started();
    }

    /// Record that a review has completed (for throttle tracking)
    pub fn review_completed(&self) {
        self.throttle.review_completed();
    }

    pub fn cancel_review(&self) {
        self.throttle.cancel_review();
    }

    /// Get the ghost learning policy decision for a boundary
    pub fn ghost_decide(
        &self,
        boundary: &crate::ghost_learning::GhostLearningBoundary,
    ) -> LearningDecision {
        if !self.ghost_learning_enabled {
            return LearningDecision::Ignore;
        }
        self.ghost_policy
            .lock()
            .unwrap_or_else(recover_mutex)
            .decide(boundary)
    }

    /// Get the ghost learning policy decision with turn count
    pub fn ghost_decide_with_turn_count(
        &self,
        boundary: &crate::ghost_learning::GhostLearningBoundary,
        turn_count: Option<u32>,
    ) -> LearningDecision {
        if !self.ghost_learning_enabled {
            return LearningDecision::Ignore;
        }
        self.ghost_policy
            .lock()
            .unwrap_or_else(recover_mutex)
            .decide_with_turn_count(boundary, turn_count)
    }

    /// 从配置更新 ghost 学习策略（支持热重载）
    pub fn update_ghost_policy(&self, config: &blockcell_core::config::GhostLearningConfig) {
        let new_policy = GhostLearningPolicy::from_config(config);
        *self.ghost_policy.lock().unwrap_or_else(recover_mutex) = new_policy;
    }

    /// Check if self-improve review is enabled
    pub fn is_self_improve_enabled(&self) -> bool {
        self.self_improve_review_enabled
    }

    /// Get debug status
    pub fn status(&self) -> String {
        format!(
            "LearningCoordinator: nudge={}, throttle_active={}, dedup_entries={}",
            self.nudge_engine
                .lock()
                .unwrap_or_else(recover_mutex)
                .status(),
            self.throttle.active_count(),
            self.dedup.len(),
        )
    }
}

/// Convert LearningAction to the ReviewMode used by spawn_review
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewMode {
    Skill,
    Memory,
    Combined,
}

impl From<&LearningAction> for Option<ReviewMode> {
    fn from(action: &LearningAction) -> Self {
        match action {
            LearningAction::Skip => None,
            LearningAction::MemoryReview { .. } => Some(ReviewMode::Memory),
            LearningAction::SkillReview { .. } => Some(ReviewMode::Skill),
            LearningAction::CombinedReview { .. } => Some(ReviewMode::Combined),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_nudge::NudgeConfig;

    fn test_coordinator() -> LearningCoordinator {
        let nudge_engine = SkillNudgeEngine::new(NudgeConfig {
            min_nudge_interval_secs: 0,
            ..Default::default()
        });
        let ghost_policy = GhostLearningPolicy::default();
        let throttle = LearningThrottle::new(2, 0);
        let dedup = LearningDedup::new(0); // 0 window = no dedup for tests
        LearningCoordinator::new(nudge_engine, ghost_policy, throttle, dedup, true, true)
    }

    #[test]
    fn test_skip_when_no_nudge() {
        let coord = test_coordinator();
        let action = coord.evaluate_nudge(true, true, None);
        assert_eq!(action, LearningAction::Skip);
    }

    #[test]
    fn test_memory_nudge_after_turns() {
        let coord = test_coordinator();
        // 3 user turns = soft threshold for memory
        for _ in 0..3 {
            coord.on_turn_start(true);
        }
        let trigger = coord.check_memory_nudge(true);
        assert!(trigger.is_some());
    }

    #[test]
    fn pending_review_reservation_releases_slot_when_dropped_before_spawn() {
        let coord = std::sync::Arc::new(LearningCoordinator::new(
            SkillNudgeEngine::new(NudgeConfig {
                min_nudge_interval_secs: 0,
                ..Default::default()
            }),
            GhostLearningPolicy::default(),
            LearningThrottle::new(1, 300),
            LearningDedup::new(0),
            true,
            true,
        ));
        for _ in 0..3 {
            coord.on_turn_start(true);
        }
        assert!(coord.check_memory_nudge(true).is_some());

        let reservation = LearningReviewReservationGuard::new(std::sync::Arc::clone(&coord));
        drop(reservation);

        for _ in 0..3 {
            coord.on_turn_start(true);
        }
        assert!(coord.check_memory_nudge(true).is_some());
    }

    #[test]
    fn test_skill_nudge_after_iterations() {
        let coord = test_coordinator();
        // 5 iterations = soft threshold for skill
        for _ in 0..5 {
            coord.record_iteration();
        }
        let trigger = coord.check_skill_nudge(true, false);
        assert!(trigger.is_some());
    }

    #[test]
    fn test_combined_when_both_due() {
        let coord = test_coordinator();
        // 3 user turns for memory
        for _ in 0..3 {
            coord.on_turn_start(true);
        }
        // 5 iterations for skill
        for _ in 0..5 {
            coord.record_iteration();
        }
        let action = coord.evaluate_nudge(true, true, None);
        assert!(matches!(action, LearningAction::CombinedReview { .. }));
    }

    #[test]
    fn test_skill_upgrades_memory_to_combined() {
        let coord = test_coordinator();
        // Memory nudge fires first
        for _ in 0..3 {
            coord.on_turn_start(true);
        }
        let memory_trigger = coord.check_memory_nudge(true);
        assert!(memory_trigger.is_some());

        // Then skill nudge fires
        for _ in 0..5 {
            coord.record_iteration();
        }
        let skill_trigger = coord.check_skill_nudge(true, true);
        assert!(skill_trigger.is_some());
    }

    #[test]
    fn test_reset_skill_after_write() {
        let coord = test_coordinator();
        for _ in 0..5 {
            coord.record_iteration();
        }
        coord.reset_skill();
        let trigger = coord.check_skill_nudge(true, false);
        assert!(trigger.is_none());
    }

    #[test]
    fn test_reset_memory_after_write() {
        let coord = test_coordinator();
        for _ in 0..3 {
            coord.on_turn_start(true);
        }
        coord.reset_memory();
        let trigger = coord.check_memory_nudge(true);
        assert!(trigger.is_none());
    }

    #[test]
    fn test_disabled_self_improve() {
        let nudge_engine = SkillNudgeEngine::new(NudgeConfig {
            min_nudge_interval_secs: 0,
            ..Default::default()
        });
        let ghost_policy = GhostLearningPolicy::default();
        let throttle = LearningThrottle::new(2, 0);
        let dedup = LearningDedup::new(0);
        let coord =
            LearningCoordinator::new(nudge_engine, ghost_policy, throttle, dedup, true, false);
        for _ in 0..3 {
            coord.on_turn_start(true);
        }
        let trigger = coord.check_memory_nudge(true);
        assert!(trigger.is_none());
    }

    #[test]
    fn test_review_mode_conversion() {
        assert_eq!(Option::<ReviewMode>::from(&LearningAction::Skip), None);
        assert_eq!(
            Option::<ReviewMode>::from(&LearningAction::MemoryReview {
                trigger: MemoryTrigger::NudgeThreshold { count: 3 }
            }),
            Some(ReviewMode::Memory)
        );
        assert_eq!(
            Option::<ReviewMode>::from(&LearningAction::SkillReview {
                trigger: SkillTrigger::NudgeThreshold { count: 5 }
            }),
            Some(ReviewMode::Skill)
        );
        assert_eq!(
            Option::<ReviewMode>::from(&LearningAction::CombinedReview {
                memory_trigger: MemoryTrigger::NudgeThreshold { count: 3 },
                skill_trigger: SkillTrigger::NudgeThreshold { count: 5 },
            }),
            Some(ReviewMode::Combined)
        );
    }
}
