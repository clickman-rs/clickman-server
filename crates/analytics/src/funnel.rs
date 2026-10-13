use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Funnel {
    pub key: String,
    pub name: String,
    pub steps: Vec<Step>,
    #[serde(default = "seven_days")]
    pub window: String,
    #[serde(default = "ninety_days")]
    pub range: String,
    #[serde(default)]
    pub breakdown: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub event: Value,
    #[serde(default, rename = "where")]
    pub filters: BTreeMap<String, Value>,
}

fn seven_days() -> String {
    "7d".into()
}
fn ninety_days() -> String {
    "90d".into()
}

#[derive(Debug, Clone)]
pub struct Occurrence {
    pub at: i64,
    pub actor: i64,
    pub event: String,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid funnel: {0}")]
pub struct FunnelError(String);

pub(crate) fn duration(value: &str) -> Result<Duration, FunnelError> {
    let (digits, unit) = if let Some(digits) = value.strip_suffix('h') {
        (digits, 'h')
    } else if let Some(digits) = value.strip_suffix('d') {
        (digits, 'd')
    } else {
        return Err(FunnelError("duration must look like 12h or 7d".into()));
    };
    let number: i64 = digits
        .parse()
        .map_err(|_| FunnelError("duration must look like 12h or 7d".into()))?;
    let hours = match unit {
        'h' => Some(number),
        'd' => number.checked_mul(24),
        _ => None,
    }
    .filter(|h| (1..=24 * 365).contains(h))
    .ok_or_else(|| FunnelError("duration must be between 1h and 365d".into()))?;
    Ok(Duration::hours(hours))
}

impl Step {
    pub fn events(&self) -> Vec<&str> {
        match &self.event {
            Value::String(s) => vec![s],
            Value::Array(v) => v.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        }
    }
    pub fn matches(&self, occurrence: &Occurrence) -> bool {
        let event_matches = match &self.event {
            Value::String(event) => event == &occurrence.event,
            Value::Array(events) => events
                .iter()
                .any(|event| event.as_str() == Some(&occurrence.event)),
            _ => false,
        };
        event_matches
            && self.filters.iter().all(|(key, value)| {
                let actual = occurrence.fields.get(key).unwrap_or(&Value::Null);
                match value {
                    Value::Array(values) => values.contains(actual),
                    _ => value == actual,
                }
            })
    }
}

impl Funnel {
    pub fn parse(value: Value) -> Result<Self, FunnelError> {
        let funnel: Self = serde_json::from_value(value).map_err(|e| FunnelError(e.to_string()))?;
        funnel.validate()?;
        Ok(funnel)
    }
    pub fn validate(&self) -> Result<(), FunnelError> {
        if self.key.is_empty()
            || self.key.len() > 100
            || !self
                .key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(FunnelError(
                "key must be lowercase letters, digits and underscores".into(),
            ));
        }
        if self.name.trim().is_empty() || self.name.len() > 200 {
            return Err(FunnelError("name is required (up to 200 bytes)".into()));
        }
        if !(2..=10).contains(&self.steps.len()) {
            return Err(FunnelError("a funnel needs 2 to 10 steps".into()));
        }
        for step in &self.steps {
            let events = step.events();
            if events.is_empty()
                || events.iter().any(|e| {
                    e.is_empty() || e.chars().count() > 200 || e.chars().any(char::is_control)
                })
                || (step.event.is_array() && events.len() != step.event.as_array().unwrap().len())
            {
                return Err(FunnelError(
                    "each step needs an event or list of event names".into(),
                ));
            }
            let scalar = |v: &Value| !v.is_array() && !v.is_object();
            if step.filters.iter().any(|(k, v)| {
                k.is_empty()
                    || k.len() > 128
                    || match v {
                        Value::Array(a) => a.is_empty() || a.iter().any(|v| !scalar(v)),
                        _ => !scalar(v),
                    }
            }) {
                return Err(FunnelError(
                    "where maps flattened keys to scalars or lists".into(),
                ));
            }
        }
        if self
            .breakdown
            .as_ref()
            .is_some_and(|v| v.trim().is_empty() || v.len() > 128)
        {
            return Err(FunnelError("breakdown must be a flattened key".into()));
        }
        duration(&self.window)?;
        duration(&self.range)?;
        Ok(())
    }
    pub fn from(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        now - duration(&self.range).expect("validated funnel")
    }
    pub fn event_names(&self) -> Vec<String> {
        self.steps
            .iter()
            .flat_map(|s| s.events().into_iter().map(str::to_owned))
            .collect()
    }
    pub fn compute(&self, occurrences: Vec<Occurrence>, now: DateTime<Utc>) -> Value {
        let size = self.steps.len();
        let mut reached = vec![0u64; size];
        let mut gaps: Vec<Vec<f64>> = vec![Vec::new(); size];
        let mut days: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        let mut values: HashMap<String, (Value, Vec<u64>)> = HashMap::new();
        for path in self.paths(occurrences, now) {
            let entry = &path[0];
            let day = DateTime::from_timestamp_millis(entry.at)
                .expect("valid occurrence")
                .date_naive()
                .to_string();
            let daily = days.entry(day).or_insert_with(|| vec![0; size]);
            let value = self
                .breakdown
                .as_ref()
                .and_then(|k| entry.fields.get(k))
                .cloned()
                .unwrap_or(Value::Null);
            let (_, split) = values
                .entry(value.to_string())
                .or_insert_with(|| (value, vec![0; size]));
            for (index, occurrence) in path.iter().enumerate() {
                reached[index] += 1;
                daily[index] += 1;
                split[index] += 1;
                if index > 0 {
                    gaps[index].push((occurrence.at - path[index - 1].at) as f64 / 1000.0);
                }
            }
        }
        let steps: Vec<_> = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                gaps[i].sort_by(f64::total_cmp);
                let v = &gaps[i];
                let median = if v.is_empty() {
                    None
                } else if v.len() % 2 == 1 {
                    Some(v[v.len() / 2].round() as i64)
                } else {
                    Some(((v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0).round() as i64)
                };
                json!({"events": step.events(), "actors": reached[i], "median_seconds": median})
            })
            .collect();
        let mut breakdown: Vec<_> = if self.breakdown.is_some() {
            values.into_values().collect()
        } else {
            vec![]
        };
        breakdown.sort_by(|a, b| {
            b.1[0]
                .cmp(&a.1[0])
                .then(a.0.to_string().cmp(&b.0.to_string()))
        });
        json!({"key": self.key, "name": self.name, "steps": steps, "days": days.into_iter().map(|(day, actors)| json!({"day":day, "actors":actors})).collect::<Vec<_>>(), "breakdown_key":self.breakdown, "breakdown":breakdown.into_iter().take(10).map(|(value, actors)|json!({"value":value,"actors":actors})).collect::<Vec<_>>()})
    }

    /// One first-entry journey per actor, shared by reports and saved segments.
    pub fn paths(&self, occurrences: Vec<Occurrence>, now: DateTime<Utc>) -> Vec<Vec<Occurrence>> {
        let mut journeys: HashMap<i64, Vec<Occurrence>> = HashMap::new();
        for occurrence in occurrences {
            journeys
                .entry(occurrence.actor)
                .or_default()
                .push(occurrence);
        }
        let first = self.from(now).timestamp_millis();
        let last = now.timestamp_millis();
        let predicate_keys: BTreeSet<_> = self
            .steps
            .iter()
            .flat_map(|step| step.filters.keys().map(String::as_str))
            .collect();
        let mut result = Vec::new();
        for mut journey in journeys.into_values() {
            let rank = |o: &Occurrence| {
                self.steps
                    .iter()
                    .position(|s| s.matches(o))
                    .unwrap_or(self.steps.len())
            };
            journey.sort_by_key(|occurrence| occurrence.at);
            for tied in journey.chunk_by_mut(|a, b| a.at == b.at) {
                if tied.len() < 2 {
                    continue;
                }
                // Reports retain full fields; segment reads project predicates.
                // Predicate order must agree in both representations. Missing
                // and explicit null match identically, so normalize them here.
                // Cache only keys for actual timestamp ties, not every event.
                tied.sort_by_cached_key(|occurrence| {
                    let predicates: BTreeMap<_, _> = predicate_keys
                        .iter()
                        .map(|key| (*key, occurrence.fields.get(*key).unwrap_or(&Value::Null)))
                        .collect();
                    (
                        rank(occurrence),
                        occurrence.event.clone(),
                        serde_json::to_string(&predicates).expect("JSON predicates"),
                        serde_json::to_string(&occurrence.fields).expect("JSON fields"),
                    )
                });
            }
            let Some(mut position) = journey
                .iter()
                .position(|o| (first..=last).contains(&o.at) && self.steps[0].matches(o))
            else {
                continue;
            };
            let deadline = journey[position].at
                + duration(&self.window)
                    .expect("validated funnel")
                    .num_milliseconds();
            let mut path = vec![journey[position].clone()];
            for step in self.steps.iter().skip(1) {
                let Some(next) = (position + 1..journey.len()).find(|i| step.matches(&journey[*i]))
                else {
                    break;
                };
                if journey[next].at > deadline || journey[next].at > last {
                    break;
                }
                position = next;
                path.push(journey[position].clone());
            }
            result.push(path);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_names_follow_the_ingestion_contract_for_control_characters_and_unicode() {
        for event in [json!("view\npage"), json!(["view", "buy\u{0000}"])] {
            assert!(Funnel::parse(json!({"key":"invalid","name":"Invalid","steps":[{"event":event},{"event":"buy"}]})).is_err());
        }
        assert!(Funnel::parse(json!({"key":"unicode","name":"Unicode","steps":[{"event":"просмотр"},{"event":["покупка","💚"]}]})).is_ok());
    }

    #[test]
    fn unrelated_fields_do_not_change_completion_when_segment_observations_are_projected() {
        let funnel = Funnel::parse(json!({"key":"repeat","name":"Repeat","steps":[{"event":"view"},{"event":"view","where":{"format":"z"}}]})).unwrap();
        let now = Utc::now();
        let at = (now - Duration::hours(1)).timestamp_millis();
        let rows = vec![
            Occurrence {
                actor: 1,
                at,
                event: "view".into(),
                fields: BTreeMap::from([
                    ("a_unused".into(), json!("a")),
                    ("format".into(), json!("z")),
                ]),
            },
            Occurrence {
                actor: 1,
                at,
                event: "view".into(),
                fields: BTreeMap::from([
                    ("a_unused".into(), json!("z")),
                    ("format".into(), json!("a")),
                ]),
            },
        ];
        let projected: Vec<_> = rows
            .iter()
            .cloned()
            .map(|mut row| {
                row.fields.retain(|key, _| key == "format");
                row
            })
            .collect();
        let complete = funnel.compute(rows, now);
        let projected = funnel.compute(projected, now);
        assert_eq!(complete["steps"], projected["steps"]);
        assert_eq!(complete["steps"][1]["actors"], 1);
    }
    #[test]
    fn unicode_duration_is_rejected_without_panicking() {
        for value in ["д", "12д", "💥", "12💥", ""] {
            let parsed = Funnel::parse(
                json!({"key":"invalid","name":"Invalid","steps":[{"event":"view"},{"event":"buy"}],"window":value}),
            );
            assert!(parsed.is_err());
        }
    }
    #[test]
    fn first_entry_only_and_filter_order_and_window() {
        let f = Funnel::parse(json!({"key":"purchase","name":"Purchase","steps":[{"event":["view","upsell"]},{"event":"start"},{"event":"buy","where":{"store":"ios"}}],"window":"1d","breakdown":"os"})).unwrap();
        let now = DateTime::parse_from_rfc3339("2026-09-23T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let occurrence = |actor, event: &str, offset| Occurrence {
            actor,
            event: event.into(),
            at: now.timestamp_millis() + offset,
            fields: BTreeMap::from([("store".into(), json!("ios")), ("os".into(), json!("iOS"))]),
        };
        let result = f.compute(
            vec![
                occurrence(1, "buy", -1000),
                occurrence(1, "start", -91000),
                occurrence(1, "view", -151000),
                occurrence(2, "view", -3 * 86400000),
                occurrence(2, "view", -200000),
                occurrence(2, "start", -150000),
            ],
            now,
        );
        assert_eq!(result["steps"][0]["actors"], 2);
        assert_eq!(result["steps"][1]["actors"], 1);
        assert_eq!(result["steps"][2]["median_seconds"], 90);
        assert_eq!(result["breakdown"][0]["value"], "iOS");
        assert!(Funnel::parse(json!({"key":"bad","name":"Bad","steps":[{"event":"a"},{"event":"b"}],"window":"0d"})).is_err());
    }
}
