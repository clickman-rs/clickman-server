pub const PROJECTS: &str = r#"query ClickmanMcpProjects {
  viewer { canCreateProjects }
  projects { edges { node { id slug name canAdmin createdAt } } }
}"#;

pub const PROJECT: &str = r#"query ClickmanMcpProject($projectId: UUID!) {
  project(id: $projectId) { id slug name canAdmin createdAt }
}"#;

pub const REPORTS: &str = r#"query ClickmanMcpReports($projectId: UUID!, $keys: [String!]) {
  project(id: $projectId) { id reports(keys: $keys) { key result computedAt } }
}"#;

pub const FUNNELS: &str = r#"query ClickmanMcpFunnels($projectId: UUID!) {
  project(id: $projectId) { id funnels { key name definition } }
}"#;

pub const SEGMENTS: &str = r#"query ClickmanMcpSegments($projectId: UUID!) {
  project(id: $projectId) { id segments { id key name definition enabled fireExisting revision members evaluatedAt } }
}"#;

pub const SAVE_FUNNEL: &str = r#"mutation ClickmanMcpSaveFunnel($projectId: UUID!, $definition: JSON!) {
  saveFunnel(projectId: $projectId, definition: $definition) { key name definition }
}"#;

pub const SAVE_SEGMENT: &str = r#"mutation ClickmanMcpSaveSegment($projectId: UUID!, $definition: JSON!, $enabled: Boolean!) {
  saveSegment(projectId: $projectId, definition: $definition, enabled: $enabled, fireExisting: false) {
    id key name definition enabled fireExisting revision members evaluatedAt
  }
}"#;
