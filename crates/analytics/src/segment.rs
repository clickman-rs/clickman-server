use crate::funnel::{Step, duration};
use crate::{Funnel, Occurrence};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub key: String,
    pub name: String,
    pub rule: Rule,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
    Event {
        filter: Step,
        range: String,
    },
    Funnel {
        funnel_key: String,
        at_step: usize,
        #[serde(default)]
        exclude_later: bool,
        #[serde(default = "zero")]
        wait: String,
    },
    InactiveAfter {
        entry: Step,
        absent: Step,
        after: String,
        range: String,
    },
}
fn zero() -> String {
    "0h".into()
}
fn wait(value: &str) -> Result<Duration, String> {
    if value == "0h" {
        Ok(Duration::zero())
    } else {
        duration(value).map_err(|e| e.to_string())
    }
}
impl Segment {
    pub fn parse(value: Value, funnels: &[Funnel]) -> Result<Self, String> {
        let segment: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        segment.validate(funnels)?;
        Ok(segment)
    }
    pub fn validate(&self, funnels: &[Funnel]) -> Result<(), String> {
        // Reuse the exact event/filter/key validation accepted by funnels.
        let validate = |steps: Value| {
            Funnel::parse(json!({"key":self.key,"name":self.name,"steps":steps}))
                .map(|_| ())
                .map_err(|e| e.to_string())
        };
        match &self.rule {
            Rule::Event { filter, range } => {
                validate(json!([filter, filter]))?;
                duration(range).map_err(|e| e.to_string())?;
            }
            Rule::Funnel {
                funnel_key,
                at_step,
                wait: delay,
                ..
            } => {
                validate(json!([{"event":"segment"},{"event":"segment"}]))?;
                let funnel = funnels
                    .iter()
                    .find(|f| f.key == *funnel_key)
                    .ok_or("Referenced funnel does not exist.")?;
                if *at_step == 0 || *at_step > funnel.steps.len() {
                    return Err("Choose an existing funnel step (1-based).".into());
                }
                wait(delay)?;
            }
            Rule::InactiveAfter {
                entry,
                absent,
                after,
                range,
            } => {
                validate(json!([entry, absent]))?;
                duration(after).map_err(|e| e.to_string())?;
                duration(range).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    pub fn from(&self, funnels: &[Funnel], now: DateTime<Utc>) -> DateTime<Utc> {
        match &self.rule {
            Rule::Event { range, .. } | Rule::InactiveAfter { range, .. } => {
                now - duration(range).expect("validated range")
            }
            Rule::Funnel { funnel_key, .. } => funnels
                .iter()
                .find(|f| f.key == *funnel_key)
                .expect("validated funnel")
                .from(now),
        }
    }
    pub fn event_names(&self, funnels: &[Funnel]) -> Vec<String> {
        match &self.rule {
            Rule::Event { filter, .. } => filter.events().into_iter().map(str::to_owned).collect(),
            Rule::InactiveAfter { entry, absent, .. } => entry
                .events()
                .into_iter()
                .chain(absent.events())
                .map(str::to_owned)
                .collect(),
            Rule::Funnel { funnel_key, .. } => funnels
                .iter()
                .find(|f| f.key == *funnel_key)
                .expect("validated funnel")
                .event_names(),
        }
    }
    /// Actor -> time at which the condition became true. Timers are evaluated
    /// on every worker tick, even when no new event arrives.
    pub fn members(
        &self,
        occurrences: impl AsRef<[Occurrence]>,
        funnels: &[Funnel],
        now: DateTime<Utc>,
    ) -> BTreeMap<i64, i64> {
        let first = self.from(funnels, now).timestamp_millis();
        let last = now.timestamp_millis();
        let occurrences = occurrences.as_ref();
        match &self.rule {
            Rule::Event { filter, .. } => {
                let mut members = BTreeMap::new();
                for o in occurrences {
                    if (first..=last).contains(&o.at) && filter.matches(o) {
                        members
                            .entry(o.actor)
                            .and_modify(|t: &mut i64| *t = (*t).min(o.at))
                            .or_insert(o.at);
                    }
                }
                members
            }
            Rule::Funnel {
                funnel_key,
                at_step,
                exclude_later,
                wait: delay,
            } => {
                let funnel = funnels
                    .iter()
                    .find(|f| f.key == *funnel_key)
                    .expect("validated funnel");
                let delay = wait(delay).expect("validated delay").num_milliseconds();
                funnel
                    .paths(
                        occurrences
                            .iter()
                            .filter(|o| {
                                (first..=last).contains(&o.at)
                                    && funnel.steps.iter().any(|step| step.matches(o))
                            })
                            .cloned()
                            .collect(),
                        now,
                    )
                    .into_iter()
                    .filter_map(|path| {
                        if path.len() < *at_step || (*exclude_later && path.len() > *at_step) {
                            return None;
                        }
                        let entered = path[at_step - 1].at + delay;
                        (entered <= last).then_some((path[0].actor, entered))
                    })
                    .collect()
            }
            Rule::InactiveAfter {
                entry,
                absent,
                after,
                ..
            } => {
                let delay = duration(after).expect("validated delay").num_milliseconds();
                let mut actors: BTreeMap<i64, (Option<i64>, Option<i64>)> = BTreeMap::new();
                for o in occurrences {
                    if (first..=last).contains(&o.at) {
                        let state = actors.entry(o.actor).or_default();
                        if entry.matches(o) {
                            state.0 = Some(state.0.map_or(o.at, |at| at.min(o.at)));
                        }
                        if absent.matches(o) {
                            state.1 = Some(state.1.map_or(o.at, |at| at.max(o.at)));
                        }
                    }
                }
                actors
                    .into_iter()
                    .filter_map(|(actor, (entry, absent))| {
                        let at = entry?;
                        let entered = at + delay;
                        if entered > last || absent.is_some_and(|absent| absent >= at) {
                            None
                        } else {
                            Some((actor, entered))
                        }
                    })
                    .collect()
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inactivity_timers_and_funnel_segments_share_report_semantics() {
        let now = Utc::now();
        let f=Funnel::parse(json!({"key":"buy","name":"Buy","steps":[{"event":"view"},{"event":"checkout"},{"event":"buy"}]})).unwrap();
        let row = |actor, name: &str, hours| Occurrence {
            actor,
            event: name.into(),
            at: (now - Duration::hours(hours)).timestamp_millis(),
            fields: BTreeMap::new(),
        };
        let rows = vec![
            row(1, "view", 25),
            row(1, "checkout", 25),
            row(2, "view", 25),
            row(2, "checkout", 25),
            row(2, "buy", 1),
            row(3, "view", 1),
            row(3, "checkout", 1),
        ];
        let segment=Segment::parse(json!({"key":"abandoned","name":"Abandoned","rule":{"kind":"funnel","funnel_key":"buy","at_step":2,"exclude_later":true,"wait":"24h"}}),std::slice::from_ref(&f)).unwrap();
        assert_eq!(
            segment
                .members(rows.clone(), std::slice::from_ref(&f), now)
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![1]
        );
        let inactive=Segment::parse(json!({"key":"inactive","name":"Inactive","rule":{"kind":"inactive_after","entry":{"event":"checkout"},"absent":{"event":"buy"},"after":"24h","range":"7d"}}),&[]).unwrap();
        assert_eq!(
            inactive
                .members(rows, &[], now)
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![1]
        );
    }
}
