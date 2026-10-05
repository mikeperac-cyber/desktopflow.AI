use crate::{
    ai::RecoveryFailureKind,
    executor::{ExecutionReport, ExecutionStatus},
};

pub const MAX_REPLAN_ATTEMPTS: u8 = 2;

pub fn should_replan(report: &ExecutionReport) -> bool {
    report.status != ExecutionStatus::Completed && report.replan_attempts < MAX_REPLAN_ATTEMPTS
}

pub fn recovery_failure_kind(report: &ExecutionReport) -> RecoveryFailureKind {
    if report.status == ExecutionStatus::VerificationFailed {
        RecoveryFailureKind::VerificationFailed
    } else {
        RecoveryFailureKind::ActionFailed
    }
}

pub fn merge_attempt(
    aggregate: Option<ExecutionReport>,
    mut attempt: ExecutionReport,
) -> ExecutionReport {
    let plan_attempt = aggregate
        .as_ref()
        .map_or(1, |report| report.plan_attempts.saturating_add(1));
    for step in &mut attempt.step_results {
        step.plan_attempt = plan_attempt;
    }
    attempt.plan_attempts = plan_attempt;
    attempt.replan_attempts = plan_attempt.saturating_sub(1);
    attempt.recovered = attempt.status == ExecutionStatus::Completed && plan_attempt > 1;

    let Some(mut report) = aggregate else {
        return attempt;
    };
    report.finished_at_unix_ms = attempt.finished_at_unix_ms;
    report.status = attempt.status;
    report.total_steps = report.total_steps.saturating_add(attempt.total_steps);
    report.completed_steps = report
        .completed_steps
        .saturating_add(attempt.completed_steps);
    report.plan_attempts = attempt.plan_attempts;
    report.replan_attempts = attempt.replan_attempts;
    report.recovered = attempt.recovered;
    report.step_results.extend(attempt.step_results);
    report.failure_message = attempt.failure_message;
    report.recovery_failure_message = attempt.recovery_failure_message;
    report
}

pub fn mark_recovery_failure(report: &mut ExecutionReport, message: String) {
    report.status = ExecutionStatus::Failed;
    report.failure_message = Some("DeskFlow stopped during bounded recovery.".to_string());
    report.recovery_failure_message = Some(message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::ExecutionStepResult;

    fn report(status: ExecutionStatus) -> ExecutionReport {
        ExecutionReport {
            started_at_unix_ms: 1,
            finished_at_unix_ms: 2,
            status,
            total_steps: 1,
            completed_steps: usize::from(status == ExecutionStatus::Completed),
            plan_attempts: 1,
            replan_attempts: 0,
            recovered: false,
            step_results: Vec::<ExecutionStepResult>::new(),
            failure_message: (status != ExecutionStatus::Completed).then(|| "changed".to_string()),
            recovery_failure_message: None,
        }
    }

    #[test]
    fn successful_second_plan_is_marked_as_recovered() {
        let first = merge_attempt(None, report(ExecutionStatus::VerificationFailed));
        let merged = merge_attempt(Some(first), report(ExecutionStatus::Completed));
        assert_eq!(merged.status, ExecutionStatus::Completed);
        assert_eq!(merged.plan_attempts, 2);
        assert_eq!(merged.replan_attempts, 1);
        assert!(merged.recovered);
        assert_eq!(merged.total_steps, 2);
    }

    #[test]
    fn retry_limit_is_strictly_bounded() {
        let mut aggregate = None;
        for _ in 0..=MAX_REPLAN_ATTEMPTS {
            aggregate = Some(merge_attempt(
                aggregate,
                report(ExecutionStatus::VerificationFailed),
            ));
        }
        let aggregate = aggregate.expect("aggregate report");
        assert_eq!(aggregate.replan_attempts, MAX_REPLAN_ATTEMPTS);
        assert!(!should_replan(&aggregate));
    }
}
