use std::collections::BTreeSet;
#[cfg(test)]
use std::{fs, path::PathBuf};

use anyhow::{Result, ensure};
use duck_core::Context;
use serde::{Deserialize, Serialize};

use crate::store::{self, Error, MAX_BYTES, RuleConfig, Spec, valid_package};
#[cfg(test)]
use crate::store::{Revision, Save, active, code_of};

const SPEC: Spec = Spec {
    subdir: "misc/language",
    domain: b"duck-misc/language/v1\0",
    marker: "play-integrity/language-spoof.version",
};

pub type Status = store::Status<Config>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub package_name: String,
    pub user_id: u32,
    pub locale: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub rules: Vec<Rule>,
}

// Accept the language[-Script][-REGION] forms used by Android; reject extensions,
// private-use tags and ambiguous input rather than silently discarding subtags.
fn normalize_locale(tag: &str) -> Option<String> {
    let parts: Vec<_> = tag.split('-').collect();
    let language = *parts.first()?;
    if language.eq_ignore_ascii_case("und")
        || !(2..=3).contains(&language.len())
        || !language.bytes().all(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    let mut result = vec![language.to_ascii_lowercase()];
    let mut index = 1;
    if parts
        .get(index)
        .is_some_and(|v| v.len() == 4 && v.bytes().all(|c| c.is_ascii_alphabetic()))
    {
        let script = parts[index].to_ascii_lowercase();
        result.push(format!(
            "{}{}",
            script[..1].to_ascii_uppercase(),
            &script[1..]
        ));
        index += 1;
    }
    if let Some(region) = parts.get(index) {
        if !((region.len() == 2 && region.bytes().all(|c| c.is_ascii_alphabetic()))
            || (region.len() == 3 && region.bytes().all(|c| c.is_ascii_digit())))
        {
            return None;
        }
        result.push(region.to_ascii_uppercase());
        index += 1;
    }
    (index == parts.len()).then(|| result.join("-"))
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
        rule.locale = normalize_locale(&rule.locale).ok_or(Error::InvalidConfig)?;
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
fn directory(ctx: &Context) -> PathBuf {
    store::directory(SPEC, ctx)
}
#[cfg(test)]
fn raw(ctx: &Context) -> Result<Vec<u8>> {
    store::raw(SPEC, ctx)
}
#[cfg(test)]
fn lock(ctx: &Context) -> Result<fs::File> {
    store::lock(SPEC, ctx)
}
#[cfg(test)]
fn write(ctx: &Context, name: &str, bytes: &[u8]) -> Result<()> {
    store::write(SPEC, ctx, name, bytes)
}
#[cfg(test)]
fn save_request(ctx: &Context, request: Save<Config>) -> Result<Status> {
    store::save_request(SPEC, ctx, request)
}
#[cfg(test)]
fn enable_request(ctx: &Context, request: Revision) -> Result<Status> {
    store::enable_request::<Config>(SPEC, ctx, request)
}

#[cfg(test)]
mod tests;
