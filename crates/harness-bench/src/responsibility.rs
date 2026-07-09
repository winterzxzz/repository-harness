use std::collections::BTreeMap;

use serde::Serialize;

use crate::score::CheckResult;

/// Passed/total tally for one harness responsibility.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct ResponsibilityScore {
    pub passed: u32,
    pub total: u32,
}

/// Group check results by their responsibility tag, tallying pass/total.
/// `BTreeMap` keeps responsibilities in stable alphabetical order for output.
pub fn rollup(checks: &[CheckResult]) -> BTreeMap<String, ResponsibilityScore> {
    let mut map: BTreeMap<String, ResponsibilityScore> = BTreeMap::new();
    for check in checks {
        let entry = map.entry(check.responsibility.clone()).or_default();
        entry.total += 1;
        if check.passed {
            entry.passed += 1;
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(responsibility: &str, passed: bool) -> CheckResult {
        CheckResult {
            id: "c".into(),
            responsibility: responsibility.into(),
            passed,
            detail: String::new(),
        }
    }

    #[test]
    fn tallies_pass_and_total_per_responsibility() {
        let checks = vec![
            result("Observability", true),
            result("Observability", false),
            result("Task specification", true),
        ];
        let map = rollup(&checks);
        assert_eq!(
            map["Observability"],
            ResponsibilityScore {
                passed: 1,
                total: 2
            }
        );
        assert_eq!(
            map["Task specification"],
            ResponsibilityScore {
                passed: 1,
                total: 1
            }
        );
    }

    #[test]
    fn empty_input_is_empty_map() {
        assert!(rollup(&[]).is_empty());
    }
}
