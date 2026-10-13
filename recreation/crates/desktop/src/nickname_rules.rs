//! Pattern nicknames (Chatterino's nickname rules): a login or a regular
//! expression, and the name to show instead. A chatter's own nickname from
//! "Personalize chatter" wins; rules apply to new messages.
use std::sync::RwLock;

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub pattern: String,
    /// The nickname; with `regex`, `$1`… insert captured groups.
    pub replace: String,
    pub regex: bool,
    pub case_sensitive: bool,
}
pub const MAX: usize = 50;
static RULES: RwLock<Vec<(Rule, Option<regex::Regex>)>> = RwLock::new(Vec::new());

/// Check a rule before saving it.
pub fn validate(rule: Rule) -> Result<Rule, String> {
    let pattern = rule.pattern.trim().to_owned();
    if pattern.is_empty() || pattern.chars().count() > 200 {
        return Err("Enter a login or pattern (up to 200 characters).".into());
    }
    let replace = crate::chatter_appearance::validate_alias(&rule.replace)?;
    if replace.is_empty() {
        return Err("Enter the nickname to show.".into());
    }
    if rule.regex {
        compile(&pattern, rule.case_sensitive)?;
    }
    Ok(Rule {
        pattern,
        replace,
        ..rule
    })
}
fn compile(pattern: &str, case_sensitive: bool) -> Result<regex::Regex, String> {
    regex::RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .size_limit(256 * 1024)
        .dfa_size_limit(256 * 1024)
        .build()
        .map_err(|_| "That isn't a valid regular expression.".to_owned())
}

pub fn set(rules: Vec<Rule>) {
    let compiled = rules
        .into_iter()
        .take(MAX)
        .filter_map(|rule| {
            let regex = if rule.regex {
                Some(compile(&rule.pattern, rule.case_sensitive).ok()?)
            } else {
                None
            };
            Some((rule, regex))
        })
        .collect();
    if let Ok(mut current) = RULES.write() {
        *current = compiled;
    }
}
pub fn get() -> Vec<Rule> {
    RULES
        .read()
        .map(|rules| rules.iter().map(|(rule, _)| rule.clone()).collect())
        .unwrap_or_default()
}

/// The nickname for a chatter, from the first rule that matches their login.
pub fn nickname(login: &str) -> Option<String> {
    let rules = RULES.read().ok()?;
    for (rule, regex) in rules.iter() {
        let found = match regex {
            Some(regex) => regex
                .is_match(login)
                .then(|| regex.replace(login, rule.replace.as_str()).into_owned()),
            None if rule.case_sensitive => (rule.pattern == login).then(|| rule.replace.clone()),
            None => rule
                .pattern
                .eq_ignore_ascii_case(login)
                .then(|| rule.replace.clone()),
        };
        if let Some(name) = found.filter(|n| !n.trim().is_empty()) {
            return Some(name.chars().filter(|c| !c.is_control()).take(40).collect());
        }
    }
    None
}

pub fn load(value: &serde_json::Value) {
    let rules = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            validate(Rule {
                pattern: item["pattern"].as_str().unwrap_or("").into(),
                replace: item["replace"].as_str().unwrap_or("").into(),
                regex: item["regex"].as_bool().unwrap_or(false),
                case_sensitive: item["case_sensitive"].as_bool().unwrap_or(false),
            })
            .ok()
        })
        .collect();
    set(rules);
}
pub fn json() -> serde_json::Value {
    serde_json::Value::Array(
        get()
            .into_iter()
            .map(|r| {
                serde_json::json!({"pattern": r.pattern, "replace": r.replace, "regex": r.regex, "case_sensitive": r.case_sensitive})
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{Rule, nickname, set, validate};
    #[test]
    fn logins_and_patterns_become_nicknames() {
        set(vec![
            validate(Rule {
                pattern: "Fossabot".into(),
                replace: "Fossa".into(),
                regex: false,
                case_sensitive: false,
            })
            .unwrap(),
            validate(Rule {
                pattern: "^(.+)_bot$".into(),
                replace: "bot: $1".into(),
                regex: true,
                case_sensitive: false,
            })
            .unwrap(),
        ]);
        assert_eq!(nickname("fossabot").as_deref(), Some("Fossa"));
        assert_eq!(nickname("sery_bot").as_deref(), Some("bot: sery"));
        assert_eq!(nickname("someone"), None);
        assert!(
            validate(Rule {
                pattern: "(".into(),
                replace: "x".into(),
                regex: true,
                case_sensitive: false
            })
            .is_err()
        );
        set(Vec::new());
    }
}
