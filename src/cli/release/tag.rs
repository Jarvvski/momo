//! Tagging the minted Version and deriving the remote to link to.

use std::cmp::Ordering;
use std::path::Path;

use anyhow::Result;

use crate::cli::error::CliError;
use crate::cli::version::FileVersionSource;
use crate::domain::config::Config;
use crate::domain::package::Package;
use crate::domain::remote::Remote;
use crate::domain::tag;
use crate::domain::vcs::Vcs;
use crate::domain::version::VersionSource;

use super::Tagged;

/// Create the VCS tag for the Version the ledger records.
///
/// The tag names the Version the last `release`/`candidate` minted — the one no
/// tag records yet — so `tag` brings the repository and momo's own record into
/// sync. A ledger that is *older* than the newest tag is stale: a later Version
/// is already tagged, so tagging this one would move the release backwards. It
/// is refused rather than created. `rev` overrides the adapter's default anchor
/// (jj: `@-`, git: `HEAD`). Returns the tag, or `None` when the Package has
/// never minted a Version.
pub fn tag(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: &dyn Vcs,
    rev: Option<&str>,
    dry_run: bool,
) -> Result<Option<Tagged>> {
    let Some(version) = FileVersionSource::new(&package.version_file(repo_root)).current()? else {
        return Ok(None);
    };
    let prefix = tag::prefix(package, &config.tag_prefix);
    if let Some(module) = tag::go_module_path(package)
        && let Some(warning) = tag::go_major_warning(module, &version)
    {
        eprintln!("warning: {warning}");
    }

    // The ledger must name the pending Version: one no tag records yet. A
    // Version below the newest tag is stale — a later one is already tagged, so
    // tagging this one would publish a release behind it.
    if let Some(latest) = vcs.latest_tag(&prefix)?
        && version.cmp_precedence(&latest) == Ordering::Less
    {
        return Err(CliError::StaleLedger { version, latest }.into());
    }

    let message = format!("Release {version}");
    if !dry_run {
        vcs.create_tag(rev, &prefix, &version, &message)?;
    }
    Ok(Some(Tagged {
        name: format!("{prefix}{version}"),
        version,
    }))
}

/// The Remote derived from the VCS, when it has one momo recognises.
///
/// A VCS remote momo cannot understand is ignored with a warning: it cannot be
/// linked to, and there is nothing for the user to fix to keep releasing. A
/// configured `remote` never reaches here — config resolution has already
/// rejected an unusable one.
pub(super) fn derive_remote(vcs: Option<&dyn Vcs>) -> Result<Option<Remote>> {
    let Some(vcs) = vcs else {
        return Ok(None);
    };
    let Some(url) = vcs.remote()? else {
        return Ok(None);
    };
    match url.parse() {
        Ok(remote) => Ok(Some(remote)),
        Err(_) => {
            eprintln!("warning: remote `{url}` is not recognised; no reference links added");
            Ok(None)
        }
    }
}
