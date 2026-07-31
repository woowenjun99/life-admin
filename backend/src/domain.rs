use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSourceType {
    Text,
    Image,
    Pdf,
}

impl CaptureSourceType {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "text" => Some(Self::Text),
            "image" => Some(Self::Image),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxStatus {
    Captured,
    Reviewing,
    Planned,
    Archived,
}

#[allow(dead_code)] // Enforced by the later review/plan update routes.
impl InboxStatus {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "captured" => Some(Self::Captured),
            "reviewing" => Some(Self::Reviewing),
            "planned" => Some(Self::Planned),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Captured, Self::Reviewing)
                | (Self::Reviewing, Self::Planned)
                | (Self::Planned, Self::Archived)
                | (Self::Archived, Self::Planned)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Ready,
    Waiting,
    Complete,
}

impl PlanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Waiting => "waiting",
            Self::Complete => "complete",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ready" => Some(Self::Ready),
            "waiting" => Some(Self::Waiting),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Ready, Self::Waiting)
                | (Self::Waiting, Self::Ready)
                | (Self::Ready | Self::Waiting, Self::Complete)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanStepState {
    pub position: u32,
    pub status: PlanStatus,
}

pub fn highlighted_next_action(steps: &[PlanStepState]) -> Option<u32> {
    steps
        .iter()
        .filter(|step| step.status == PlanStatus::Ready)
        .map(|step| step.position)
        .min()
}

pub fn derived_plan_status(steps: &[PlanStepState]) -> Option<PlanStatus> {
    if steps.is_empty() {
        return None;
    }

    if steps.iter().all(|step| step.status == PlanStatus::Complete) {
        return Some(PlanStatus::Complete);
    }

    if steps.iter().all(|step| step.status != PlanStatus::Ready) {
        return Some(PlanStatus::Waiting);
    }

    Some(PlanStatus::Ready)
}

#[cfg(test)]
mod tests {
    use super::{
        InboxStatus, PlanStatus, PlanStepState, derived_plan_status, highlighted_next_action,
    };

    #[test]
    fn inbox_statuses_only_follow_the_review_flow() {
        assert!(InboxStatus::Captured.can_transition_to(InboxStatus::Reviewing));
        assert!(InboxStatus::Reviewing.can_transition_to(InboxStatus::Planned));
        assert!(InboxStatus::Planned.can_transition_to(InboxStatus::Archived));
        assert!(InboxStatus::Archived.can_transition_to(InboxStatus::Planned));
        assert!(!InboxStatus::Captured.can_transition_to(InboxStatus::Planned));
        assert!(!InboxStatus::Captured.can_transition_to(InboxStatus::Archived));
        assert!(!InboxStatus::Reviewing.can_transition_to(InboxStatus::Archived));
        assert!(!InboxStatus::Archived.can_transition_to(InboxStatus::Captured));
    }

    #[test]
    fn plan_statuses_allow_waiting_but_not_reopening_complete_work() {
        assert!(PlanStatus::Ready.can_transition_to(PlanStatus::Waiting));
        assert!(PlanStatus::Waiting.can_transition_to(PlanStatus::Ready));
        assert!(PlanStatus::Waiting.can_transition_to(PlanStatus::Complete));
        assert!(!PlanStatus::Complete.can_transition_to(PlanStatus::Ready));
    }

    #[test]
    fn highlights_the_lowest_ordered_ready_step() {
        let steps = [
            PlanStepState {
                position: 2,
                status: PlanStatus::Ready,
            },
            PlanStepState {
                position: 0,
                status: PlanStatus::Waiting,
            },
            PlanStepState {
                position: 1,
                status: PlanStatus::Ready,
            },
        ];

        assert_eq!(highlighted_next_action(&steps), Some(1));
        assert_eq!(derived_plan_status(&steps), Some(PlanStatus::Ready));
    }

    #[test]
    fn a_waiting_plan_has_no_actionable_next_step() {
        let steps = [PlanStepState {
            position: 0,
            status: PlanStatus::Waiting,
        }];

        assert_eq!(highlighted_next_action(&steps), None);
        assert_eq!(derived_plan_status(&steps), Some(PlanStatus::Waiting));
    }

    #[test]
    fn a_plan_with_only_completed_steps_is_complete() {
        let steps = [
            PlanStepState {
                position: 0,
                status: PlanStatus::Complete,
            },
            PlanStepState {
                position: 1,
                status: PlanStatus::Complete,
            },
        ];

        assert_eq!(highlighted_next_action(&steps), None);
        assert_eq!(derived_plan_status(&steps), Some(PlanStatus::Complete));
    }
}
