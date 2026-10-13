use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RateLimit {
    pub per_second: f64,
    pub burst: f64,
}
impl Default for RateLimit {
    fn default() -> Self {
        Self {
            per_second: 20.0,
            burst: 100.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectSettings {
    pub filter_fragments: Vec<String>,
    pub max_event_age_days: u32,
    pub dedup_window_days: u32,
    pub retention_days: Option<u32>,
    pub rotation_lag_seconds: u32,
    pub rate_limit: RateLimit,
}
impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            filter_fragments: clickman_protocol::DEFAULT_FRAGMENTS
                .iter()
                .map(|v| (*v).to_string())
                .collect(),
            max_event_age_days: 90,
            dedup_window_days: 7,
            retention_days: None,
            rotation_lag_seconds: 120,
            rate_limit: RateLimit::default(),
        }
    }
}
impl ProjectSettings {
    pub fn validate(&self) -> Result<()> {
        if !(1..=3650).contains(&self.max_event_age_days)
            || !(1..=365).contains(&self.dedup_window_days)
            || self.retention_days.is_some_and(|v| v == 0 || v > 36500)
            || self.rotation_lag_seconds > 86400
        {
            return Err(Error::Invalid(
                "invalid age, deduplication, retention or rotation limits".into(),
            ));
        }
        if !self.rate_limit.per_second.is_finite()
            || self.rate_limit.per_second <= 0.0
            || !self.rate_limit.burst.is_finite()
            || self.rate_limit.burst < 1.0
            || self.rate_limit.burst > 1_000_000.0
        {
            return Err(Error::Invalid(
                "rate limit needs a positive rate and a burst of 1..1000000".into(),
            ));
        }
        if self.filter_fragments.len() > 1000
            || self
                .filter_fragments
                .iter()
                .any(|s| s.trim().is_empty() || s.len() > 200)
        {
            return Err(Error::Invalid("invalid sanitizer fragments".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Reader,
    Admin,
}
impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reader => "reader",
            Self::Admin => "admin",
        }
    }
}

/// Host-authenticated request permissions. The host owns users and sessions.
#[derive(Debug, Clone, Default)]
pub struct Access {
    root: bool,
    projects: BTreeMap<Uuid, Role>,
}
impl Access {
    pub fn root() -> Self {
        Self {
            root: true,
            projects: BTreeMap::new(),
        }
    }
    pub fn project(project: Uuid, role: Role) -> Self {
        Self {
            root: false,
            projects: BTreeMap::from([(project, role)]),
        }
    }
    pub fn projects(projects: BTreeMap<Uuid, Role>) -> Self {
        Self {
            root: false,
            projects,
        }
    }
    pub fn is_root(&self) -> bool {
        self.root
    }
    pub fn can_read(&self, project: Uuid) -> bool {
        self.root || self.projects.contains_key(&project)
    }
    pub fn can_admin(&self, project: Uuid) -> bool {
        self.root || self.projects.get(&project) == Some(&Role::Admin)
    }
}
