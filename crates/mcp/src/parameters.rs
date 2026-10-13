use rmcp::schemars;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectInput {
    /// UUID of a project returned by list_projects. Host authorization is checked on every call.
    pub project_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportsInput {
    /// UUID of a project returned by list_projects.
    pub project_id: String,
    /// Optional report keys (up to 32): actives, retention, events, or funnel:<key>.
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DefinitionInput {
    /// UUID of a project returned by list_projects.
    pub project_id: String,
    /// Clickman definition object. Funnel: key, name, steps, optional window/range/breakdown.
    /// A step is {"event":"name" or ["names"],"where":{"flattened_key":scalar or [scalars]}}.
    /// Funnels need 2..10 steps; window/range are 1h..365d (defaults 7d/90d).
    /// Keys contain lowercase letters, digits and underscores; breakdown is an optional flattened field key.
    /// Segment: key, name, rule (event, funnel, inactive_after).
    /// Event rule: {"kind":"event","filter":step,"range":"30d"}.
    /// Funnel rule: {"kind":"funnel","funnel_key":"key","at_step":1,"exclude_later":true,"wait":"24h"}; step is 1-based, wait defaults to 0h.
    /// Inactivity rule: {"kind":"inactive_after","entry":step,"absent":step,"after":"24h","range":"30d"}.
    pub definition: Value,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveSegmentInput {
    /// UUID of a project returned by list_projects.
    pub project_id: String,
    /// Segment definition: key, name, rule (event, funnel, inactive_after).
    /// Event rule: {"kind":"event","filter":{"event":"buy","where":{"plan":"paid"}},"range":"30d"}.
    /// Funnel rule: {"kind":"funnel","funnel_key":"checkout","at_step":2,"exclude_later":true,"wait":"24h"}.
    /// Inactivity rule: {"kind":"inactive_after","entry":{"event":"view"},"absent":{"event":"buy"},"after":"24h","range":"30d"}.
    pub definition: Value,
    /// Explicitly choose whether membership evaluation should run. Existing members are never backfilled by MCP.
    pub enabled: bool,
}
