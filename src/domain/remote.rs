//! The code-host remote a Changelog's reference links point at.
//!
//! A remote URL is normalised to a browsable web base, then turned into the
//! `compare` and tag URLs the Changelog footer uses. Forge detection is by host
//! name: GitHub, GitLab, and a generic fallback that uses GitHub's URL shapes
//! (Gitea and most self-hosted forges share them).

use std::fmt;
use std::str::FromStr;

use semver::Version;

/// The code host a remote belongs to, which decides its URL shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    /// GitHub.
    GitHub,
    /// GitLab.
    GitLab,
    /// Any other host; momo uses GitHub-compatible URL shapes.
    Generic,
}

/// A code-host remote, normalised to the web base URL momo links to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    base: String,
    forge: Forge,
}

impl Remote {
    /// The web base URL, without a trailing slash or `.git` suffix.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The code host this remote belongs to.
    pub fn forge(&self) -> Forge {
        self.forge
    }

    /// The URL comparing `from` to `to`.
    pub fn compare(&self, from: &str, to: &str) -> String {
        match self.forge {
            Forge::GitLab => format!("{}/-/compare/{from}...{to}", self.base),
            Forge::GitHub | Forge::Generic => format!("{}/compare/{from}...{to}", self.base),
        }
    }

    /// The URL of a tag's release page.
    pub fn tag(&self, tag: &str) -> String {
        match self.forge {
            Forge::GitLab => format!("{}/-/tags/{tag}", self.base),
            Forge::GitHub | Forge::Generic => format!("{}/releases/tag/{tag}", self.base),
        }
    }

    /// The reference-link footer for `versions`, newest first.
    ///
    /// `prefix` is the tag prefix, so a Version `1.2.0` links to `v1.2.0`. When
    /// `unreleased`, an Unreleased link compares the newest Release to `HEAD`;
    /// without one the footer defines no `[Unreleased]` link, so a document with
    /// no Unreleased section carries no dangling definition. Each Release links
    /// to the comparison with the one below it, and the oldest links to its tag.
    pub fn footer(&self, versions: &[Version], prefix: &str, unreleased: bool) -> Vec<String> {
        let tag = |version: &Version| format!("{prefix}{version}");
        let mut lines = Vec::new();
        if unreleased && let Some(newest) = versions.first() {
            lines.push(format!(
                "[Unreleased]: {}",
                self.compare(&tag(newest), "HEAD")
            ));
        }
        for (index, version) in versions.iter().enumerate() {
            let link = match versions.get(index + 1) {
                Some(previous) => self.compare(&tag(previous), &tag(version)),
                None => self.tag(&tag(version)),
            };
            lines.push(format!("[{version}]: {link}"));
        }
        lines
    }
}

impl FromStr for Remote {
    type Err = RemoteError;

    /// Parse a remote URL — `https://host/owner/repo(.git)`,
    /// `git@host:owner/repo(.git)`, or `ssh://git@host/owner/repo(.git)`.
    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let base = normalise(url).ok_or_else(|| RemoteError {
            url: url.to_owned(),
        })?;
        let forge = Forge::of(&base);
        Ok(Self { base, forge })
    }
}

impl fmt::Display for Remote {
    /// The normalised web base URL.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.base)
    }
}

impl Forge {
    /// The Forge a normalised base URL belongs to, from its host name.
    fn of(base: &str) -> Self {
        let host = base
            .split_once("://")
            .map_or(base, |(_, rest)| rest)
            .split('/')
            .next()
            .unwrap_or_default();
        if host.contains("github") {
            Self::GitHub
        } else if host.contains("gitlab") {
            Self::GitLab
        } else {
            Self::Generic
        }
    }
}

/// The error produced when a remote URL cannot be understood.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{url}` is not a remote URL momo recognises")]
pub struct RemoteError {
    /// The URL that could not be parsed.
    pub url: String,
}

/// Normalise a remote URL to `https://host/owner/repo`, or `None` when it is
/// not a recognisable remote.
///
/// A non-`http(s)` scheme (`ssh`, `git`, `git+ssh`) carries a transport port,
/// not a web one, so a trailing `:<digits>` is dropped before the web base is
/// built. An `http(s)` authority keeps its port, which may be a real web port.
fn normalise(url: &str) -> Option<String> {
    let url = url.trim();
    let url = url.strip_suffix(".git").unwrap_or(url);
    let (authority, path) = match url.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_once('/')?;
            let authority = if matches!(scheme, "http" | "https") {
                authority
            } else {
                strip_port(authority)
            };
            (authority, path)
        }
        None => url.split_once(':')?,
    };
    let host = authority.rsplit('@').next()?;
    let path = path.trim_matches('/');
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("https://{host}/{path}"))
}

/// Drop a trailing `:<digits>` from `authority`, the transport port of a
/// non-`http(s)` scheme. Anything else is returned unchanged.
fn strip_port(authority: &str) -> &str {
    match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => authority,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
    }

    fn remote(url: &str) -> Remote {
        url.parse().expect("a remote URL")
    }

    #[test]
    fn scp_style_and_https_urls_normalise_to_the_same_base() {
        let expected = "https://github.com/Jarvvski/momo";
        assert_eq!(remote("git@github.com:Jarvvski/momo.git").base(), expected);
        assert_eq!(remote("git@github.com:Jarvvski/momo").base(), expected);
        assert_eq!(
            remote("https://github.com/Jarvvski/momo.git").base(),
            expected
        );
        assert_eq!(
            remote("ssh://git@github.com/Jarvvski/momo.git").base(),
            expected
        );
    }

    #[test]
    fn an_ssh_transport_port_is_dropped_from_the_web_base() {
        let expected = "https://github.com/o/r";
        assert_eq!(remote("ssh://git@github.com:2222/o/r.git").base(), expected);
        assert_eq!(remote("git://github.com:9418/o/r.git").base(), expected);
        assert_eq!(remote("git+ssh://git@github.com:2222/o/r").base(), expected);
    }

    #[test]
    fn an_http_port_is_kept_in_the_web_base() {
        // A self-hosted forge may serve its web UI on a non-standard port.
        assert_eq!(
            remote("https://gitlab.example.com:8443/group/repo").base(),
            "https://gitlab.example.com:8443/group/repo"
        );
        assert_eq!(
            remote("http://gitlab.example.com:8443/group/repo.git").base(),
            "https://gitlab.example.com:8443/group/repo"
        );
    }

    #[test]
    fn the_forge_is_detected_from_the_host() {
        assert_eq!(remote("git@github.com:a/b.git").forge(), Forge::GitHub);
        assert_eq!(remote("https://gitlab.com/a/b.git").forge(), Forge::GitLab);
        assert_eq!(remote("https://codeberg.org/a/b").forge(), Forge::Generic);
    }

    #[test]
    fn gitlab_uses_its_own_url_shapes() {
        let gitlab = remote("https://gitlab.com/a/b.git");
        assert_eq!(
            gitlab.compare("v1.0.0", "v1.1.0"),
            "https://gitlab.com/a/b/-/compare/v1.0.0...v1.1.0"
        );
        assert_eq!(gitlab.tag("v1.0.0"), "https://gitlab.com/a/b/-/tags/v1.0.0");
    }

    #[test]
    fn github_uses_its_own_url_shapes() {
        let github = remote("https://github.com/a/b");
        assert_eq!(
            github.compare("v1.0.0", "v1.1.0"),
            "https://github.com/a/b/compare/v1.0.0...v1.1.0"
        );
        assert_eq!(
            github.tag("v1.0.0"),
            "https://github.com/a/b/releases/tag/v1.0.0"
        );
    }

    #[test]
    fn a_url_without_a_path_is_rejected() {
        assert!("https://github.com".parse::<Remote>().is_err());
        assert!("not a url".parse::<Remote>().is_err());
    }

    #[test]
    fn the_footer_links_the_unreleased_head_and_every_release() {
        let remote = remote("git@github.com:Jarvvski/momo.git");
        let versions = [version(1, 2, 0), version(1, 1, 0), version(1, 0, 0)];

        assert_eq!(
            remote.footer(&versions, "v", true),
            vec![
                "[Unreleased]: https://github.com/Jarvvski/momo/compare/v1.2.0...HEAD",
                "[1.2.0]: https://github.com/Jarvvski/momo/compare/v1.1.0...v1.2.0",
                "[1.1.0]: https://github.com/Jarvvski/momo/compare/v1.0.0...v1.1.0",
                "[1.0.0]: https://github.com/Jarvvski/momo/releases/tag/v1.0.0",
            ]
        );
    }

    #[test]
    fn the_footer_omits_the_unreleased_link_when_there_is_no_unreleased_section() {
        let remote = remote("git@github.com:Jarvvski/momo.git");
        let versions = [version(1, 1, 0), version(1, 0, 0)];

        assert_eq!(
            remote.footer(&versions, "v", false),
            vec![
                "[1.1.0]: https://github.com/Jarvvski/momo/compare/v1.0.0...v1.1.0",
                "[1.0.0]: https://github.com/Jarvvski/momo/releases/tag/v1.0.0",
            ]
        );
    }

    #[test]
    fn a_single_release_without_unreleased_links_only_its_tag() {
        let remote = remote("git@github.com:Jarvvski/momo.git");

        assert_eq!(
            remote.footer(&[version(0, 1, 0)], "v", false),
            vec!["[0.1.0]: https://github.com/Jarvvski/momo/releases/tag/v0.1.0"]
        );
    }

    #[test]
    fn the_footer_is_empty_before_any_release() {
        let remote = remote("https://github.com/a/b");
        assert!(remote.footer(&[], "v", true).is_empty());
    }

    #[test]
    fn a_monorepo_prefix_reaches_the_footer() {
        let remote = remote("https://github.com/a/b");
        let footer = remote.footer(&[version(0, 1, 0)], "cargo-momo-v", true);
        assert_eq!(
            footer,
            vec![
                "[Unreleased]: https://github.com/a/b/compare/cargo-momo-v0.1.0...HEAD",
                "[0.1.0]: https://github.com/a/b/releases/tag/cargo-momo-v0.1.0",
            ]
        );
    }
}
