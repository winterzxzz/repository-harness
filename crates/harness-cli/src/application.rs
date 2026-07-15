use std::path::PathBuf;

use crate::domain::{
    AuditResult, BacklogFilter, BacklogRecord, BoolFlag, ContextScoreResult, CsvList,
    DecisionRecord, FrictionRecord, HarnessStats, ImprovementProposal, InputType, IntakeRecord,
    InterventionRecord, RiskLane, StoryMatrixRecord, StoryVerifyAllResult, StoryVerifyStatus,
    ToolArgSpec, ToolEntry, TraceRecord, TraceScoreResult,
};

#[derive(Debug)]
pub struct HarnessContext {
    pub repo_root: PathBuf,
    pub db_path: PathBuf,
    pub schema_dir: PathBuf,
}

#[derive(Debug)]
pub struct IntakeInput {
    pub input_type: InputType,
    pub summary: String,
    pub risk_lane: RiskLane,
    pub risk_flags: CsvList,
    pub affected_docs: CsvList,
    pub story_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug)]
pub struct StoryAddInput {
    pub id: String,
    pub title: String,
    pub risk_lane: RiskLane,
    pub contract_doc: Option<String>,
    pub verify_command: Option<String>,
    pub e2e_command: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug)]
pub struct StoryUpdateInput {
    pub id: String,
    pub status: Option<String>,
    pub evidence: Option<String>,
    pub unit: Option<BoolFlag>,
    pub integration: Option<BoolFlag>,
    pub e2e: Option<BoolFlag>,
    pub platform: Option<BoolFlag>,
    pub verify_command: Option<String>,
    pub e2e_command: Option<String>,
}

#[derive(Debug)]
pub struct DecisionAddInput {
    pub id: String,
    pub title: String,
    pub status: String,
    pub doc_path: Option<String>,
    pub verify_command: Option<String>,
    pub predicted_impact: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug)]
pub struct BacklogAddInput {
    pub title: String,
    pub discovered_while: Option<String>,
    pub current_pain: Option<String>,
    pub suggestion: Option<String>,
    pub risk: Option<RiskLane>,
    pub predicted_impact: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug)]
pub struct ToolRegisterInput {
    pub name: String,
    pub command: String,
    pub description: String,
    pub responsibility: String,
    pub args: Vec<ToolArgSpec>,
    pub force: bool,
    pub kind: String,
    pub capability: Option<String>,
    pub scan_target: Option<String>,
}

#[derive(Debug)]
pub struct InterventionAddInput {
    pub trace_id: Option<i64>,
    pub story_id: Option<String>,
    pub intervention_type: String,
    pub description: String,
    pub source: String,
    pub impact: Option<String>,
}

#[derive(Debug, Default)]
pub struct InterventionFilter {
    pub trace_id: Option<i64>,
    pub story_id: Option<String>,
    pub intervention_type: Option<String>,
}

#[derive(Debug)]
pub struct BacklogCloseInput {
    pub id: i64,
    pub status: String,
    pub actual_outcome: Option<String>,
}

#[derive(Debug)]
pub struct TraceInput {
    pub task_summary: String,
    pub intake_id: Option<i64>,
    pub story_id: Option<String>,
    pub agent: Option<String>,
    pub outcome: Option<String>,
    pub duration_seconds: Option<i64>,
    pub token_estimate: Option<i64>,
    pub friction: Option<String>,
    pub notes: Option<String>,
    pub actions: CsvList,
    pub files_read: CsvList,
    pub files_changed: CsvList,
    pub decisions: CsvList,
    pub errors: CsvList,
}

#[derive(Debug)]
pub struct ContextPackInput {
    pub story_id: Option<String>,
    pub lane: Option<RiskLane>,
}

#[derive(Debug)]
pub struct ChangesetApplyResult {
    pub id: String,
    pub applied: bool,
    pub operations: usize,
}

#[derive(Debug)]
pub struct DbRebuildResult {
    pub db_path: PathBuf,
    pub changesets: usize,
    pub operations: usize,
}

/// Outcome of one `tool check` scan. The CLI reports these facts; the agent
/// applies policy (skip / degrade / use) based on `status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCheckResult {
    pub name: String,
    pub kind: String,
    pub capability: Option<String>,
    pub status: String,
    pub detail: String,
}

pub trait HarnessRepository {
    type Error;

    fn init(&self) -> std::result::Result<InitResult, Self::Error>;
    fn migrate(&self) -> std::result::Result<MigrateResult, Self::Error>;
    fn import_brownfield(&self) -> std::result::Result<BrownfieldImportResult, Self::Error>;
    fn record_intake(&self, input: IntakeInput) -> std::result::Result<i64, Self::Error>;
    fn add_story(&self, input: StoryAddInput) -> std::result::Result<(), Self::Error>;
    fn update_story(&self, input: StoryUpdateInput) -> std::result::Result<(), Self::Error>;
    fn verify_story(&self, id: &str) -> std::result::Result<StoryVerifyResult, Self::Error>;
    fn verify_all_stories(&self) -> std::result::Result<StoryVerifyAllResult, Self::Error>;
    fn add_decision(&self, input: DecisionAddInput) -> std::result::Result<(), Self::Error>;
    fn verify_decision(&self, id: &str) -> std::result::Result<DecisionVerifyResult, Self::Error>;
    fn add_backlog(&self, input: BacklogAddInput) -> std::result::Result<i64, Self::Error>;
    fn close_backlog(&self, input: BacklogCloseInput) -> std::result::Result<(), Self::Error>;
    fn register_tool(&self, input: ToolRegisterInput) -> std::result::Result<(), Self::Error>;
    fn remove_tool(&self, name: &str) -> std::result::Result<(), Self::Error>;
    fn check_tools(
        &self,
        name: Option<String>,
    ) -> std::result::Result<Vec<ToolCheckResult>, Self::Error>;
    fn add_intervention(
        &self,
        input: InterventionAddInput,
    ) -> std::result::Result<i64, Self::Error>;
    fn record_trace(&self, input: TraceInput) -> std::result::Result<i64, Self::Error>;
    fn score_trace(&self, id: Option<i64>) -> std::result::Result<TraceScoreResult, Self::Error>;
    fn score_context(&self, id: i64) -> std::result::Result<ContextScoreResult, Self::Error>;
    fn context_pack(&self, input: ContextPackInput) -> std::result::Result<String, Self::Error>;
    fn story_verify_status(&self, id: &str) -> std::result::Result<StoryVerifyStatus, Self::Error>;
    fn query_matrix(&self) -> std::result::Result<Vec<StoryMatrixRecord>, Self::Error>;
    fn query_backlog(
        &self,
        filter: BacklogFilter,
    ) -> std::result::Result<Vec<BacklogRecord>, Self::Error>;
    fn query_decisions(&self) -> std::result::Result<Vec<DecisionRecord>, Self::Error>;
    fn query_intakes(&self) -> std::result::Result<Vec<IntakeRecord>, Self::Error>;
    fn query_traces(&self) -> std::result::Result<Vec<TraceRecord>, Self::Error>;
    fn query_friction(&self) -> std::result::Result<Vec<FrictionRecord>, Self::Error>;
    fn query_tools(
        &self,
        responsibility: Option<String>,
        capability: Option<String>,
    ) -> std::result::Result<Vec<ToolEntry>, Self::Error>;
    fn query_interventions(
        &self,
        filter: InterventionFilter,
    ) -> std::result::Result<Vec<InterventionRecord>, Self::Error>;
    fn query_stats(&self) -> std::result::Result<HarnessStats, Self::Error>;
    fn audit(&self) -> std::result::Result<AuditResult, Self::Error>;
    fn propose(&self, commit: bool) -> std::result::Result<Vec<ImprovementProposal>, Self::Error>;
    fn query_sql(&self, sql: &str) -> std::result::Result<QueryTable, Self::Error>;
    fn apply_changeset(
        &self,
        path: &std::path::Path,
    ) -> std::result::Result<ChangesetApplyResult, Self::Error>;
    fn rebuild_db(
        &self,
        changeset_dir: &std::path::Path,
    ) -> std::result::Result<DbRebuildResult, Self::Error>;
}

pub struct HarnessService<R: HarnessRepository> {
    repository: R,
}

impl<R: HarnessRepository> HarnessService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn init(&self) -> std::result::Result<InitResult, R::Error> {
        self.repository.init()
    }

    pub fn migrate(&self) -> std::result::Result<MigrateResult, R::Error> {
        self.repository.migrate()
    }

    pub fn import_brownfield(&self) -> std::result::Result<BrownfieldImportResult, R::Error> {
        self.repository.import_brownfield()
    }

    pub fn record_intake(&self, input: IntakeInput) -> std::result::Result<i64, R::Error> {
        self.repository.record_intake(input)
    }

    pub fn add_story(&self, input: StoryAddInput) -> std::result::Result<(), R::Error> {
        self.repository.add_story(input)
    }

    pub fn update_story(&self, input: StoryUpdateInput) -> std::result::Result<(), R::Error> {
        self.repository.update_story(input)
    }

    pub fn verify_story(&self, id: &str) -> std::result::Result<StoryVerifyResult, R::Error> {
        self.repository.verify_story(id)
    }

    pub fn verify_all_stories(&self) -> std::result::Result<StoryVerifyAllResult, R::Error> {
        self.repository.verify_all_stories()
    }

    pub fn add_decision(&self, input: DecisionAddInput) -> std::result::Result<(), R::Error> {
        self.repository.add_decision(input)
    }

    pub fn verify_decision(&self, id: &str) -> std::result::Result<DecisionVerifyResult, R::Error> {
        self.repository.verify_decision(id)
    }

    pub fn add_backlog(&self, input: BacklogAddInput) -> std::result::Result<i64, R::Error> {
        self.repository.add_backlog(input)
    }

    pub fn close_backlog(&self, input: BacklogCloseInput) -> std::result::Result<(), R::Error> {
        self.repository.close_backlog(input)
    }

    pub fn register_tool(&self, input: ToolRegisterInput) -> std::result::Result<(), R::Error> {
        self.repository.register_tool(input)
    }

    pub fn remove_tool(&self, name: &str) -> std::result::Result<(), R::Error> {
        self.repository.remove_tool(name)
    }

    pub fn check_tools(
        &self,
        name: Option<String>,
    ) -> std::result::Result<Vec<ToolCheckResult>, R::Error> {
        self.repository.check_tools(name)
    }

    pub fn add_intervention(
        &self,
        input: InterventionAddInput,
    ) -> std::result::Result<i64, R::Error> {
        self.repository.add_intervention(input)
    }

    pub fn record_trace(&self, input: TraceInput) -> std::result::Result<i64, R::Error> {
        self.repository.record_trace(input)
    }

    pub fn score_trace(&self, id: Option<i64>) -> std::result::Result<TraceScoreResult, R::Error> {
        self.repository.score_trace(id)
    }

    pub fn score_context(&self, id: i64) -> std::result::Result<ContextScoreResult, R::Error> {
        self.repository.score_context(id)
    }

    pub fn context_pack(&self, input: ContextPackInput) -> std::result::Result<String, R::Error> {
        self.repository.context_pack(input)
    }

    pub fn story_verify_status(
        &self,
        id: &str,
    ) -> std::result::Result<StoryVerifyStatus, R::Error> {
        self.repository.story_verify_status(id)
    }

    pub fn query_matrix(&self) -> std::result::Result<Vec<StoryMatrixRecord>, R::Error> {
        self.repository.query_matrix()
    }

    pub fn query_backlog(
        &self,
        filter: BacklogFilter,
    ) -> std::result::Result<Vec<BacklogRecord>, R::Error> {
        self.repository.query_backlog(filter)
    }

    pub fn query_decisions(&self) -> std::result::Result<Vec<DecisionRecord>, R::Error> {
        self.repository.query_decisions()
    }

    pub fn query_intakes(&self) -> std::result::Result<Vec<IntakeRecord>, R::Error> {
        self.repository.query_intakes()
    }

    pub fn query_traces(&self) -> std::result::Result<Vec<TraceRecord>, R::Error> {
        self.repository.query_traces()
    }

    pub fn query_friction(&self) -> std::result::Result<Vec<FrictionRecord>, R::Error> {
        self.repository.query_friction()
    }

    pub fn query_tools(
        &self,
        responsibility: Option<String>,
        capability: Option<String>,
    ) -> std::result::Result<Vec<ToolEntry>, R::Error> {
        self.repository.query_tools(responsibility, capability)
    }

    pub fn query_interventions(
        &self,
        filter: InterventionFilter,
    ) -> std::result::Result<Vec<InterventionRecord>, R::Error> {
        self.repository.query_interventions(filter)
    }

    pub fn query_stats(&self) -> std::result::Result<HarnessStats, R::Error> {
        self.repository.query_stats()
    }

    pub fn audit(&self) -> std::result::Result<AuditResult, R::Error> {
        self.repository.audit()
    }

    pub fn propose(&self, commit: bool) -> std::result::Result<Vec<ImprovementProposal>, R::Error> {
        self.repository.propose(commit)
    }

    pub fn query_sql(&self, sql: &str) -> std::result::Result<QueryTable, R::Error> {
        self.repository.query_sql(sql)
    }

    pub fn apply_changeset(
        &self,
        path: &std::path::Path,
    ) -> std::result::Result<ChangesetApplyResult, R::Error> {
        self.repository.apply_changeset(path)
    }

    pub fn rebuild_db(
        &self,
        changeset_dir: &std::path::Path,
    ) -> std::result::Result<DbRebuildResult, R::Error> {
        self.repository.rebuild_db(changeset_dir)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InitResult {
    Created { db_path: PathBuf },
    Existing { db_path: PathBuf, version: i64 },
    MigratedExisting { db_path: PathBuf },
}

#[derive(Debug, PartialEq, Eq)]
pub struct MigrateResult {
    pub current_version: i64,
    pub applied: Vec<i64>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BrownfieldImportResult {
    pub stories: usize,
    pub decisions: usize,
    pub backlog_items: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DecisionVerifyResult {
    pub command: String,
    pub result: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StoryVerifyResult {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub result: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct QueryTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}
