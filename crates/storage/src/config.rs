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
    pub allowed_origins: Vec<String>,
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
            allowed_origins: Vec::new(),
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
    pub fn accepts_origin(&self, origin: Option<&str>) -> bool {
        let Some(origin) = origin else {
            return true;
        };
        if self.allowed_origins.is_empty() {
            return true;
        }
        let Ok(origin) = url::Url::parse(origin) else {
            return false;
        };
        self.allowed_origins.iter().any(|candidate| {
            url::Url::parse(candidate).is_ok_and(|c| c.origin() == origin.origin())
        })
    }
    pub fn validate(&self) -> Result<()> {
        if self.allowed_origins.len() > 50
            || self.allowed_origins.iter().any(|o| {
                o.len() > 2048
                    || url::Url::parse(o).map_or(true, |u| {
                        !matches!(u.scheme(), "http" | "https")
                            || u.host_str().is_none()
                            || u.path() != "/"
                            || u.query().is_some()
                            || u.fragment().is_some()
                            || !u.username().is_empty()
                            || u.password().is_some()
                    })
            })
        {
            return Err(Error::Invalid(
                "allowed_origins must contain up to 50 HTTP(S) origins".into(),
            ));
        }
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
