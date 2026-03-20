use crate::modules::quota::{
    resolve_project_with_contract, ProjectResolutionOutcome, ProjectResolutionStage,
};

fn describe_resolution_outcome(outcome: ProjectResolutionOutcome) -> String {
    match outcome {
        ProjectResolutionOutcome::Resolved(project) => project.project_id,
        ProjectResolutionOutcome::InProgressExhausted { .. } => {
            "Project resolution still in progress after 5 onboardUser attempts".to_string()
        }
        ProjectResolutionOutcome::TransportFailure { stage, error, .. } => {
            format!("{} transport failure: {}", stage.as_str(), error)
        }
        ProjectResolutionOutcome::LoadHttpFailure {
            status,
            body_preview,
            ..
        } => format!("loadCodeAssist returned HTTP {}: {}", status, body_preview),
        ProjectResolutionOutcome::OnboardHttpFailure {
            status,
            body_preview,
            ..
        } => format!("onboardUser returned HTTP {}: {}", status, body_preview),
        ProjectResolutionOutcome::TerminalMissingProject { stage, .. } => match stage {
            ProjectResolutionStage::LoadCodeAssist => {
                "loadCodeAssist completed without a real project_id".to_string()
            }
            ProjectResolutionStage::OnboardUser => {
                "onboardUser returned done=true without a real project_id".to_string()
            }
        },
        ProjectResolutionOutcome::ParseFailure { stage, error, .. } => {
            format!("{} parse failure: {}", stage.as_str(), error)
        }
    }
}

pub async fn resolve_project(access_token: &str) -> ProjectResolutionOutcome {
    resolve_project_with_contract(access_token, None, None).await
}

pub async fn fetch_project_id(access_token: &str) -> Result<String, String> {
    match resolve_project(access_token).await {
        ProjectResolutionOutcome::Resolved(project) => Ok(project.project_id),
        outcome => Err(describe_resolution_outcome(outcome)),
    }
}
