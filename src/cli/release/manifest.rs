//! Manifest rendering and the shared-manifest conflict check.

use std::path::Path;

use anyhow::{Result, bail};
use semver::Version;

use crate::domain::manifest::ManifestUpdate;
use crate::domain::package::Package;

use super::Prepared;

/// Render `version` into the Package's ecosystem manifest, when it has one.
///
/// A read-only step: it parses the manifest, so a malformed one fails before
/// anything is written, and returns the text to stage. A Package whose
/// ecosystem has no adapter, or that has no manifest on disk, is skipped — the
/// Release still succeeds. An inherited version momo cannot resolve is an error,
/// never a silent skip.
pub(super) fn render_manifest(
    package_dir: &Path,
    package: &Package,
    version: &Version,
) -> Result<Option<(String, ManifestUpdate)>> {
    let Some(sync) = crate::adapters::manifest::for_package(package) else {
        return Ok(None);
    };
    let Some(manifest) = sync.locate(package_dir, package.manifest_override()) else {
        return Ok(None);
    };
    Ok(sync.render(&manifest, version)?)
}

/// Refuse a run whose Packages would write one manifest to different versions.
///
/// A Cargo workspace member inherits its version from the root's
/// `[workspace.package].version`, a single scalar shared by every member, so a
/// release may only write it when every Package resolving to that manifest
/// agrees on the target. Otherwise the last writer would win and the earlier
/// ones would be silently wrong, so the whole run fails before anything is
/// written.
pub fn reject_shared_manifest_conflicts(plans: &[(&Package, Prepared)]) -> Result<()> {
    let mut written: Vec<(&Path, &Version, &str, &Package)> = Vec::new();
    for (package, prepared) in plans {
        let Some((path, text)) = prepared.manifest_write() else {
            continue;
        };
        let Some((_, version, seen, other)) = written.iter().find(|(seen, ..)| *seen == path)
        else {
            written.push((path, prepared.version(), text, package));
            continue;
        };
        if **version == *prepared.version() && *seen == text {
            continue;
        }
        let detail = if **version == *prepared.version() {
            format!("the same version {version} but different contents")
        } else {
            format!("different versions ({version} and {})", prepared.version())
        };
        bail!(
            "`{}` and `{}` both sync `{}` with {detail}; a workspace version is shared, \
             so release them in separate runs",
            other.identity(),
            package.identity(),
            path.display(),
        );
    }
    Ok(())
}
