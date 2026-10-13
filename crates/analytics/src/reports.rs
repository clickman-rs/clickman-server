use chrono::{Duration, NaiveDate};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct Activity {
    pub actor: i64,
    pub first_seen: NaiveDate,
    pub days: BTreeSet<NaiveDate>,
}

pub fn actives(activities: &[Activity], today: NaiveDate) -> Value {
    let days: Vec<_> = (0..90)
        .rev()
        .map(|offset| {
            let day = today - Duration::days(offset);
            let count = |window: i64| {
                activities
                    .iter()
                    .filter(|a| {
                        a.days
                            .range((day - Duration::days(window - 1))..=day)
                            .next()
                            .is_some()
                    })
                    .count()
            };
            json!({"day": day, "dau": count(1), "wau": count(7), "mau": count(30)})
        })
        .collect();
    json!({"days": days})
}

pub fn retention(activities: &[Activity], today: NaiveDate) -> Value {
    let first = today - Duration::days(89);
    let cohorts: BTreeSet<_> = activities
        .iter()
        .filter(|a| (first..=today).contains(&a.first_seen))
        .map(|a| a.first_seen)
        .collect();
    let cohorts: Vec<_> = cohorts.into_iter().map(|day| {
        let members: Vec<_> = activities.iter().filter(|a| a.first_seen == day).collect();
        let count = |offset| { let returned = day + Duration::days(offset); if returned > today { Value::Null } else { json!(members.iter().filter(|a| a.days.contains(&returned)).count()) } };
        json!({"day": day, "size": members.len(), "d1": count(1), "d7": count(7), "d30": count(30)})
    }).collect();
    json!({"cohorts": cohorts})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_cross_year_and_future_retention_is_null() {
        let date = |s| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let activities = vec![Activity {
            actor: 1,
            first_seen: date("2025-12-30"),
            days: BTreeSet::from([date("2025-12-30"), date("2025-12-31")]),
        }];
        let report = actives(&activities, date("2026-01-02"));
        assert_eq!(
            report["days"][89],
            json!({"day":"2026-01-02", "dau":0, "wau":1, "mau":1})
        );
        let cohort = &retention(&activities, date("2026-01-02"))["cohorts"][0];
        assert_eq!(cohort["d1"], 1);
        assert!(cohort["d7"].is_null());
    }
}
