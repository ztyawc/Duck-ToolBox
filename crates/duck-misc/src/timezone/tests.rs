use std::fs;

use super::*;
use crate::{
    language,
    store::{Revision, Save, code_of},
};

fn context(root: &std::path::Path) -> Context {
    fs::create_dir_all(root.join("play-integrity")).unwrap();
    fs::write(root.join("play-integrity/language-spoof.version"), "1\n").unwrap();
    fs::write(root.join("play-integrity/timezone-spoof.version"), "1\n").unwrap();
    fs::write(root.join("play-integrity/classes.dex"), "dex fixture").unwrap();
    fs::create_dir_all(root.join("zygisk")).unwrap();
    fs::write(root.join("zygisk/arm64-v8a.so"), "ELF fixture").unwrap();
    Context {
        paths: duck_core::AppPaths::for_root(root, root),
        sysroot: duck_core::Sysroot::new(root),
    }
}

fn config() -> Config {
    Config {
        rules: vec![Rule {
            package_name: "com.example.app".into(),
            user_id: 0,
            timezone: "america/new_york".into(),
        }],
    }
}

fn save_current(ctx: &Context, config: Config) -> Status {
    store::save_request(
        SPEC,
        ctx,
        Save {
            revision: status(ctx).unwrap().revision,
            config,
        },
    )
    .unwrap()
}

#[test]
fn iana_timezone_ids_are_canonical_and_posix_or_unknown_forms_fail() {
    for (input, expected) in [
        ("america/new_york", "America/New_York"),
        ("Asia/Shanghai", "Asia/Shanghai"),
        (
            "america/argentina/buenos_aires",
            "America/Argentina/Buenos_Aires",
        ),
        ("america/port-au-prince", "America/Port-au-Prince"),
        ("utc", "UTC"),
    ] {
        assert_eq!(normalize_timezone(input).as_deref(), Some(expected));
    }
    for input in [
        "",
        "EST5EDT",
        "GMT+8",
        "Etc/GMT-8",
        ":/etc/localtime",
        "../America/New_York",
        "America//New_York",
        "America/New York",
        "America/Unknown_City",
        "Mars/Olympus",
        "America/New_York\n",
    ] {
        assert!(normalize_timezone(input).is_none(), "{input}");
    }
    for zone in iana::IANA_TIMEZONES {
        assert_eq!(normalize_timezone(zone).as_deref(), Some(*zone), "{zone}");
    }
}

#[test]
fn user_scopes_are_independent_but_duplicate_targets_are_rejected() {
    let mut value = config();
    let mut work = value.rules[0].clone();
    work.user_id = 10;
    work.timezone = "Asia/Tokyo".into();
    value.rules.push(work.clone());
    validate(&mut value).unwrap();
    assert_eq!(value.rules[0].timezone, "America/New_York");
    assert_eq!(value.rules[1].timezone, "Asia/Tokyo");
    value.rules.push(work);
    assert_eq!(
        code_of(&validate(&mut value).unwrap_err()),
        "invalid_config"
    );
}

#[test]
fn save_enable_and_stop_are_isolated_from_language_spoofing() {
    let temp = tempfile::tempdir().unwrap();
    let ctx = context(temp.path());
    let initial = status(&ctx).unwrap();
    assert!(!initial.enabled);

    let saved = save_current(&ctx, config());
    assert!(!saved.enabled);
    assert_eq!(saved.config.unwrap().rules[0].timezone, "America/New_York");

    let enabled = store::enable_request::<Config>(
        SPEC,
        &ctx,
        Revision {
            revision: saved.revision,
        },
    )
    .unwrap();
    assert!(enabled.enabled);
    assert!(!language::status(&ctx).unwrap().enabled);

    disable(&ctx).unwrap();
    assert!(!status(&ctx).unwrap().enabled);
    assert_eq!(status(&ctx).unwrap().config.unwrap().rules.len(), 1);
}

#[test]
fn stop_invalidates_stale_enable_and_handles_corrupt_rules() {
    let temp = tempfile::tempdir().unwrap();
    let ctx = context(temp.path());
    let saved = save_current(&ctx, config());
    disable(&ctx).unwrap();
    let stale = store::enable_request::<Config>(
        SPEC,
        &ctx,
        Revision {
            revision: saved.revision,
        },
    )
    .unwrap_err();
    assert_eq!(code_of(&stale), "stale_revision");

    store::write(SPEC, &ctx, "rules.json", b"{corrupt").unwrap();
    store::write(SPEC, &ctx, "enabled", b"1:token").unwrap();
    assert_eq!(
        status(&ctx).unwrap().config_error.as_deref(),
        Some("invalid_config")
    );
    disable(&ctx).unwrap();
    assert!(!status(&ctx).unwrap().enabled);
    assert_eq!(store::raw(SPEC, &ctx).unwrap(), b"{corrupt");
}

#[test]
fn enable_requires_timezone_payload_marker() {
    let temp = tempfile::tempdir().unwrap();
    let ctx = context(temp.path());
    save_current(&ctx, config());
    fs::remove_file(ctx.paths.root.join("play-integrity/timezone-spoof.version")).unwrap();
    assert!(!status(&ctx).unwrap().payload_installed);
    assert!(language::status(&ctx).unwrap().payload_installed);
    let error = store::enable_request::<Config>(
        SPEC,
        &ctx,
        Revision {
            revision: status(&ctx).unwrap().revision,
        },
    )
    .unwrap_err();
    assert_eq!(code_of(&error), "runtime_unavailable");
}
