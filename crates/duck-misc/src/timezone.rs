mod iana;

use std::collections::BTreeSet;

use anyhow::{Result, ensure};
use duck_core::Context;
use serde::{Deserialize, Serialize};

use crate::store::{self, Error, MAX_BYTES, RuleConfig, Spec, valid_package};

const SPEC: Spec = Spec {
    subdir: "misc/timezone",
    domain: b"duck-misc/timezone/v1\0",
    marker: "play-integrity/timezone-spoof.version",
};

pub type Status = store::Status<Config>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub package_name: String,
    pub user_id: u32,
    pub timezone: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub rules: Vec<Rule>,
}

fn valid_timezone_syntax(id: &str) -> bool {
    if !(3..=64).contains(&id.len()) {
        return false;
    }
    if id.eq_ignore_ascii_case("UTC") {
        return true;
    }
    let parts: Vec<_> = id.split('/').collect();
    (2..=3).contains(&parts.len())
        && parts.iter().all(|part| {
            (1..=32).contains(&part.len())
                && part.as_bytes()[0].is_ascii_alphabetic()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'+'))
        })
}

// Accept only canonical regional IANA timezone identifiers (e.g. America/New_York) and UTC;
// reject POSIX TZ expressions, file paths and unknown zones before runtime injection.
fn normalize_timezone(id: &str) -> Option<String> {
    if !valid_timezone_syntax(id) {
        return None;
    }
    iana::IANA_TIMEZONES
        .iter()
        .find(|zone| zone.eq_ignore_ascii_case(id))
        .map(|zone| (*zone).to_owned())
}

fn validate(config: &mut Config) -> Result<()> {
    ensure!(config.rules.len() <= 256, Error::InvalidConfig);
    let mut seen = BTreeSet::new();
    for rule in &mut config.rules {
        ensure!(
            valid_package(&rule.package_name) && rule.user_id <= 21473,
            Error::InvalidConfig
        );
        ensure!(
            seen.insert((rule.user_id, rule.package_name.clone())),
            Error::InvalidConfig
        );
        rule.timezone = normalize_timezone(&rule.timezone).ok_or(Error::InvalidConfig)?;
    }
    config
        .rules
        .sort_by(|a, b| (a.user_id, &a.package_name).cmp(&(b.user_id, &b.package_name)));
    ensure!(
        serde_json::to_vec(config)?.len() as u64 <= MAX_BYTES,
        Error::ConfigTooLarge
    );
    Ok(())
}

impl RuleConfig for Config {
    fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    fn validate(&mut self) -> Result<()> {
        validate(self)
    }
}

pub fn status(ctx: &Context) -> Result<Status> {
    store::status(SPEC, ctx)
}

pub fn save(ctx: &Context) -> Result<Status> {
    store::save_request(SPEC, ctx, store::input()?)
}

pub fn enable(ctx: &Context) -> Result<Status> {
    store::enable_request::<Config>(SPEC, ctx, store::input()?)
}

pub fn disable(ctx: &Context) -> Result<serde_json::Value> {
    store::disable(SPEC, ctx)
}

#[cfg(test)]
mod tests;
