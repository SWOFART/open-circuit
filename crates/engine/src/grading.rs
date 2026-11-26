//! Schema-2 build assignment grading. The original check result remains intact.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::assignment::{AssignmentError, AssignmentErrorCode};
use crate::checks::evaluate_checks_with_status;
use crate::{CheckRequest, CheckResult, CheckVerdict, MAX_CHECK_CRITERIA, StableId};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "AssignmentGrading.ts")]
pub struct AssignmentGrading {
    pub criteria: Vec<CriterionGrading>,
    pub hints: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "CriterionGrading.ts")]
pub struct CriterionGrading {
    pub criterion_id: StableId,
    pub points: u16,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_hint_key"
    )]
    #[ts(optional)]
    pub hint_key: Option<String>,
}

fn present_hint_key<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

fn valid_hint_key(key: &str) -> bool {
    (1..=64).contains(&key.len())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

impl AssignmentGrading {
    pub(crate) fn validate<'a>(
        &self,
        authored: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), AssignmentError> {
        use AssignmentErrorCode::{DuplicateId, InvalidField, TooLarge, UnknownReference};
        let error = AssignmentError::new;
        if self.criteria.len() > MAX_CHECK_CRITERIA {
            return Err(error(TooLarge, "/grading/criteria".into()));
        }
        if self.hints.len() > 64 {
            return Err(error(TooLarge, "/grading/hints".into()));
        }
        for (key, text) in &self.hints {
            if !valid_hint_key(key) {
                return Err(error(InvalidField, "/grading/hints".into()));
            }
            if text.trim().is_empty() || text.chars().take(501).count() > 500 {
                return Err(error(InvalidField, format!("/grading/hints/{key}")));
            }
        }
        let mut missing = authored.into_iter().collect::<HashSet<_>>();
        let known = missing.clone();
        let mut possible = 0_u32;
        for (index, row) in self.criteria.iter().enumerate() {
            let id = row.criterion_id.as_str();
            let base = format!("/grading/criteria/{index}");
            if !known.contains(id) {
                return Err(error(UnknownReference, format!("{base}/criterionId")));
            }
            if !missing.remove(id) {
                return Err(error(DuplicateId, format!("{base}/criterionId")));
            }
            if row.points > 1000 {
                return Err(error(InvalidField, format!("{base}/points")));
            }
            if let Some(key) = &row.hint_key {
                if !valid_hint_key(key) {
                    return Err(error(InvalidField, format!("{base}/hintKey")));
                }
                if !self.hints.contains_key(key) {
                    return Err(error(UnknownReference, format!("{base}/hintKey")));
                }
            }
            possible += u32::from(row.points);
        }
        if !missing.is_empty() || possible == 0 {
            return Err(error(InvalidField, "/grading/criteria".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "GradedCheckRequest.ts")]
pub struct GradedCheckRequest {
    pub request: CheckRequest,
    pub grading: AssignmentGrading,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "GradedCheckResult.ts")]
pub struct GradedCheckResult {
    #[ts(type = "2")]
    pub schema_version: u32,
    pub result: CheckResult,
    pub score: GradingScore,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "GradingScore.ts")]
pub struct GradingScore {
    pub earned: u32,
    pub possible: u32,
    pub criteria: Vec<CriterionScore>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "CriterionScore.ts")]
pub struct CriterionScore {
    pub criterion_id: StableId,
    pub earned: u16,
    pub possible: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hint: Option<ScoreHint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ScoreHint.ts")]
pub struct ScoreHint {
    pub key: String,
    pub text: String,
}

pub fn evaluate_graded_checks(
    graded: GradedCheckRequest,
) -> Result<GradedCheckResult, AssignmentError> {
    let GradedCheckRequest { request, grading } = graded;
    if request.checks.len() > MAX_CHECK_CRITERIA {
        return Err(AssignmentError::new(
            AssignmentErrorCode::TooLarge,
            "/checks",
        ));
    }
    let ids = request
        .checks
        .iter()
        .map(|criterion| criterion.id().as_str());
    if ids.clone().collect::<HashSet<_>>().len() != request.checks.len() {
        return Err(AssignmentError::new(
            AssignmentErrorCode::DuplicateId,
            "/checks",
        ));
    }
    grading.validate(ids)?;
    let ordered_ids: Vec<_> = request
        .checks
        .iter()
        .map(|criterion| criterion.id().clone())
        .collect();
    let (result, compiled) = evaluate_checks_with_status(request);
    Ok(score_result(result, compiled, ordered_ids, grading))
}

pub(crate) fn score_result(
    result: CheckResult,
    compiled: bool,
    ordered_ids: Vec<StableId>,
    grading: AssignmentGrading,
) -> GradedCheckResult {
    let available = compiled && result.locked.verdict == CheckVerdict::Pass;
    let by_id: HashMap<_, _> = grading
        .criteria
        .into_iter()
        .map(|row| (row.criterion_id.clone(), row))
        .collect();
    let criteria: Vec<_> = ordered_ids
        .into_iter()
        .zip(&result.criteria)
        .map(|(id, criterion)| {
            let row = &by_id[&id];
            let verdict = criterion.verdict();
            let earned = if available && verdict == CheckVerdict::Pass {
                row.points
            } else {
                0
            };
            let hint = if available && verdict == CheckVerdict::Fail {
                row.hint_key.as_ref().map(|key| ScoreHint {
                    key: key.clone(),
                    text: grading.hints[key].clone(),
                })
            } else {
                None
            };
            CriterionScore {
                criterion_id: id,
                earned,
                possible: row.points,
                hint,
            }
        })
        .collect();
    GradedCheckResult {
        schema_version: 2,
        score: GradingScore {
            earned: criteria.iter().map(|row| u32::from(row.earned)).sum(),
            possible: criteria.iter().map(|row| u32::from(row.possible)).sum(),
            criteria,
        },
        result,
    }
}
