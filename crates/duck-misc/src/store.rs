use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Result, ensure};
use duck_core::Context;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

pub(crate) const MAX_BYTES: u64 = 65536;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Invalid spoofing configuration")]
    InvalidConfig,
    #[error("Rules exceed the 64 KiB configuration limit")]
    ConfigTooLarge,
    #[error("The saved configuration changed; refresh before saving")]
    StaleRevision,
    #[error("Another configuration operation is running")]
    Busy,
    #[error("Add at least one application rule before enabling")]
    EmptyRules,
    #[error("Enable the module and reboot to load the spoofing runtime")]
    RuntimeUnavailable,
}

pub(crate) fn code_of(error: &anyhow::Error) -> &'static str {
    match error.downcast_ref::<Error>() {
        Some(Error::InvalidConfig) => "invalid_config",
        Some(Error::ConfigTooLarge) => "config_too_large",
        Some(Error::StaleRevision) => "stale_revision",
        Some(Error::Busy) => "operation_busy",
        Some(Error::EmptyRules) => "empty_rules",
        Some(Error::RuntimeUnavailable) => "runtime_unavailable",
        None => "state_unavailable",
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Spec {
    pub subdir: &'static str,
    pub domain: &'static [u8],
    pub marker: &'static str,
}

pub(crate) trait RuleConfig: Default + Serialize + DeserializeOwned {
    fn is_empty(&self) -> bool;
    fn validate(&mut self) -> Result<()>;
}

#[derive(Debug, Serialize)]
pub struct Status<C> {
    pub config: Option<C>,
    pub config_error: Option<String>,
    pub revision: String,
    pub enabled: bool,
    pub payload_installed: bool,
    pub parent_active: bool,
    pub restart_required: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Revision {
    pub revision: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Save<C> {
    pub revision: String,
    pub config: C,
}

pub(crate) fn directory(spec: Spec, ctx: &Context) -> PathBuf {
    ctx.paths.var_dir.join(spec.subdir)
}

pub(crate) fn valid_package(name: &str) -> bool {
    name.len() <= 255
        && name.contains('.')
        && name.split('.').all(|part| {
            !part.is_empty()
                && part.as_bytes()[0].is_ascii_alphabetic()
                && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        })
}

fn open_read(path: &Path) -> Result<Option<fs::File>> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    match options.open(path) {
        Ok(file) => {
            ensure!(file.metadata()?.is_file(), Error::InvalidConfig);
            Ok(Some(file))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn raw(spec: Spec, ctx: &Context) -> Result<Vec<u8>> {
    let Some(file) = open_read(&directory(spec, ctx).join("rules.json"))? else {
        return Ok(vec![]);
    };
    let mut bytes = vec![];
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= MAX_BYTES, Error::InvalidConfig);
    Ok(bytes)
}

fn activation(spec: Spec, ctx: &Context) -> Result<Vec<u8>> {
    let Some(file) = open_read(&directory(spec, ctx).join("enabled"))? else {
        return Ok(vec![]);
    };
    let mut bytes = vec![];
    file.take(65).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 64, Error::InvalidConfig);
    Ok(bytes)
}

fn revision(spec: Spec, bytes: &[u8], activation: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(spec.domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.update(activation);
    hex::encode(hash.finalize())
}

pub(crate) fn parse<C: RuleConfig>(bytes: &[u8]) -> Result<C> {
    if bytes.is_empty() {
        return Ok(C::default());
    }
    let mut config: C = serde_json::from_slice(bytes).map_err(|_| Error::InvalidConfig)?;
    config.validate()?;
    Ok(config)
}

pub(crate) fn active(bytes: &[u8]) -> bool {
    bytes.starts_with(b"1:")
        && bytes.len() > 2
        && bytes[2..]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

fn absent(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

fn payload_installed(spec: Spec, ctx: &Context) -> bool {
    let marker = ctx.paths.root.join(spec.marker);
    let Ok(Some(file)) = open_read(&marker) else {
        return false;
    };
    let mut version = vec![];
    file.take(16).read_to_end(&mut version).is_ok()
        && version == b"1\n"
        && ctx.paths.root.join("play-integrity/classes.dex").is_file()
        && ctx.paths.root.join("zygisk/arm64-v8a.so").is_file()
}

pub(crate) fn lock(spec: Spec, ctx: &Context) -> Result<fs::File> {
    let directory = directory(spec, ctx);
    fs::create_dir_all(&directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let private = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(&directory)?;
        private.set_permissions(fs::Permissions::from_mode(0o700))?;
    }
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let lock = options.open(directory.join("write.lock"))?;
    ensure!(lock.metadata()?.is_file(), Error::InvalidConfig);
    match lock.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => return Err(Error::Busy.into()),
        Err(fs::TryLockError::Error(error)) => return Err(error.into()),
    }
    Ok(lock)
}

pub(crate) fn write(spec: Spec, ctx: &Context, name: &str, bytes: &[u8]) -> Result<()> {
    let directory = directory(spec, ctx);
    let mut file = tempfile::NamedTempFile::new_in(&directory)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(directory.join(name))?;
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

pub(crate) fn input<T: DeserializeOwned>() -> Result<T> {
    let mut bytes = vec![];
    std::io::stdin()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= MAX_BYTES, Error::InvalidConfig);
    serde_json::from_slice(&bytes).map_err(|_| Error::InvalidConfig.into())
}

fn check(spec: Spec, ctx: &Context, expected: &str) -> Result<()> {
    ensure!(
        revision(spec, &raw(spec, ctx)?, &activation(spec, ctx)?) == expected,
        Error::StaleRevision
    );
    Ok(())
}

pub(crate) fn status<C: RuleConfig>(spec: Spec, ctx: &Context) -> Result<Status<C>> {
    let bytes = raw(spec, ctx)?;
    let activation = activation(spec, ctx)?;
    let enabled = active(&activation);
    let config = parse::<C>(&bytes);
    Ok(Status {
        config_error: config.as_ref().err().map(|_| "invalid_config".into()),
        config: config.ok(),
        revision: revision(spec, &bytes, &activation),
        enabled,
        payload_installed: payload_installed(spec, ctx),
        parent_active: absent(&ctx.paths.root.join("disable"))
            && absent(&ctx.paths.root.join("remove")),
        restart_required: true,
    })
}

pub(crate) fn save_request<C: RuleConfig>(
    spec: Spec,
    ctx: &Context,
    mut request: Save<C>,
) -> Result<Status<C>> {
    request.config.validate()?;
    let _lock = lock(spec, ctx)?;
    check(spec, ctx, &request.revision)?;
    write(
        spec,
        ctx,
        "rules.json",
        &serde_json::to_vec(&request.config)?,
    )?;
    status(spec, ctx)
}

pub(crate) fn enable_request<C: RuleConfig>(
    spec: Spec,
    ctx: &Context,
    request: Revision,
) -> Result<Status<C>> {
    let _lock = lock(spec, ctx)?;
    check(spec, ctx, &request.revision)?;
    ensure!(!parse::<C>(&raw(spec, ctx)?)?.is_empty(), Error::EmptyRules);
    let state = status::<C>(spec, ctx)?;
    ensure!(
        state.payload_installed && state.parent_active,
        Error::RuntimeUnavailable
    );
    set_activation(spec, ctx, true)?;
    status(spec, ctx)
}

fn set_activation(spec: Spec, ctx: &Context, enabled: bool) -> Result<()> {
    let nonce = tempfile::NamedTempFile::new_in(directory(spec, ctx))?;
    let token = nonce.path().file_name().unwrap().to_string_lossy();
    write(
        spec,
        ctx,
        "enabled",
        format!("{}:{token}", u8::from(enabled)).as_bytes(),
    )
}

pub(crate) fn disable(spec: Spec, ctx: &Context) -> Result<serde_json::Value> {
    let _lock = lock(spec, ctx)?;
    set_activation(spec, ctx, false)?;
    Ok(serde_json::json!({"enabled": false, "restart_required": true}))
}
