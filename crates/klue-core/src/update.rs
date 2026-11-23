//! Updates: Klue's releases on GitHub. Only releases built from a commit on the main
//! branch count (the dev branch has its own pre-releases); none there yet means no update.

use std::{
    cmp::Ordering,
    fmt,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde_json::Value;
use wreq::header::{ACCEPT, USER_AGENT};

use crate::{FetchError, http, json as j};

const API: &str = "https://api.github.com/repos/karimawi/Klue";
/// Releases count only when built from a commit on this branch.
pub const BRANCH: &str = "main";

/// A release version: `major.minor.patch`, optionally with a pre-release part
/// (`0.3.0-beta.1`), which comes before the release itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl Version {
    /// `v0.3.0`, `0.3.0`, `0.3.0-beta.1`, `0.3.0+build`; `None` for anything else.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches(['v', 'V']);
        let text = text.split('+').next()?;
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) if !pre.is_empty() => (core, Some(pre.to_string())),
            Some(_) => return None,
            None => (text, None),
        };
        let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
        let (major, minor, patch) = (parts.next()??, parts.next()??, parts.next().unwrap_or(Some(0))?);
        if parts.next().is_some() {
            return None;
        }
        Some(Self { major, minor, patch, pre })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)).then_with(|| match (&self.pre, &other.pre) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => compare_pre(a, b),
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Pre-release parts compare field by field, numbers as numbers (`beta.2 < beta.10`).
fn compare_pre(a: &str, b: &str) -> Ordering {
    let mut a = a.split('.');
    let mut b = b.split('.');
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let order = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

/// A downloadable file of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub title: String,
    /// The release notes, in Markdown.
    pub notes: String,
    /// The release's page on GitHub.
    pub page: String,
    pub published: Option<DateTime<Utc>>,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
    /// The commit (or branch) it was built from.
    pub target: String,
}

impl Release {
    /// The first asset whose name ends with one of `suffixes`, in their order.
    pub fn asset(&self, suffixes: &[&str]) -> Option<&Asset> {
        suffixes.iter().find_map(|s| self.assets.iter().find(|a| a.name.ends_with(s)))
    }
}

/// GitHub's release list, newest first, without drafts or tags that aren't versions.
pub(crate) fn parse_releases(v: &Value) -> Vec<Release> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter(|r| !j::truthy(&r["draft"]))
        .filter_map(|r| {
            let tag = j::text(&r["tag_name"])?;
            let assets = r["assets"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|a| {
                    // Only a plain file name: it becomes a path on disk.
                    let name = j::text(&a["name"]).filter(|n| !n.contains(['/', '\\']) && !n.starts_with('.'))?;
                    Some(Asset { name, url: j::text(&a["browser_download_url"])?, size: j::num(&a["size"]).unwrap_or(0.0) as u64 })
                })
                .collect();
            Some(Release {
                version: Version::parse(&tag)?,
                title: j::text(&r["name"]).unwrap_or_else(|| tag.clone()),
                notes: j::text(&r["body"]).unwrap_or_default(),
                page: j::text(&r["html_url"]).unwrap_or_default(),
                published: j::iso_datetime(&r["published_at"]),
                prerelease: j::truthy(&r["prerelease"]),
                target: j::text(&r["target_commitish"]).unwrap_or_default(),
                assets,
                tag,
            })
        })
        .collect()
}

/// Whether GitHub's compare of `main...<commit>` says the commit is in main's history.
pub(crate) fn in_branch(compare: &Value) -> bool {
    matches!(j::text(&compare["status"]).as_deref(), Some("behind" | "identical"))
}

fn is_commit(target: &str) -> bool {
    (7..=40).contains(&target.len()) && target.chars().all(|c| c.is_ascii_hexdigit())
}

async fn get_json(client: &wreq::Client, url: &str) -> Result<Value, FetchError> {
    let resp = client
        .get(url)
        .header(ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header(USER_AGENT, "Klue-updater")
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(FetchError::Unexpected(format!("GitHub answered HTTP {}", status.as_u16())));
    }
    http::json_body(resp, "GitHub").await
}

/// The newest release from the main branch that's newer than `current`, if any.
pub async fn newer_than(current: &Version) -> Result<Option<Release>, FetchError> {
    let client = http::plain_client(Duration::from_secs(30))?;
    let list = get_json(&client, &format!("{API}/releases?per_page=30")).await?;
    let mut candidates: Vec<Release> = parse_releases(&list).into_iter().filter(|r| r.version > *current).collect();
    candidates.sort_by(|a, b| b.version.cmp(&a.version));
    for release in candidates {
        let on_main = if release.target == BRANCH {
            true
        } else if is_commit(&release.target) {
            in_branch(&get_json(&client, &format!("{API}/compare/{BRANCH}...{}", release.target)).await?)
        } else {
            false
        };
        if on_main {
            return Ok(Some(release));
        }
        tracing::debug!("release {} isn't from {BRANCH}", release.tag);
    }
    Ok(None)
}

/// Download `asset` into `dir`, reporting progress as (bytes so far, total).
pub async fn download(asset: &Asset, dir: &Path, mut progress: impl FnMut(u64, Option<u64>)) -> Result<PathBuf, FetchError> {
    let client = http::plain_client(Duration::from_secs(15 * 60))?;
    let resp = client.get(&asset.url).header(USER_AGENT, "Klue-updater").send().await?;
    if !resp.status().is_success() {
        return Err(FetchError::Unexpected(format!("the download answered HTTP {}", resp.status().as_u16())));
    }
    let total = resp.content_length().or((asset.size > 0).then_some(asset.size));
    std::fs::create_dir_all(dir).map_err(io)?;
    let path = dir.join(&asset.name);
    let mut file = std::fs::File::create(&path).map_err(io)?;
    let mut got = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).map_err(io)?;
        got += chunk.len() as u64;
        progress(got, total);
    }
    file.flush().map_err(io)?;
    if asset.size > 0 && got != asset.size {
        let _ = std::fs::remove_file(&path);
        return Err(FetchError::Unexpected(format!("the download stopped at {got} of {} bytes", asset.size)));
    }
    Ok(path)
}

fn io(e: std::io::Error) -> FetchError {
    FetchError::Unexpected(e.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn versions_compare_like_semver() {
        assert!(v("v0.3.0") > v("0.2.0"));
        assert!(v("0.10.0") > v("0.9.9"));
        assert!(v("1.0.0") > v("1.0.0-rc.1"));
        assert!(v("1.0.0-beta.10") > v("1.0.0-beta.2"));
        assert!(v("1.0.0-beta") > v("1.0.0-alpha"));
        assert_eq!(v("0.3"), v("0.3.0"));
        assert_eq!(v("0.3.0+abc").to_string(), "0.3.0");
        assert_eq!(v("0.3.0-beta.1").to_string(), "0.3.0-beta.1");
        assert_eq!(Version::parse("latest"), None);
        assert_eq!(Version::parse("1.2.3.4"), None);
    }

    #[test]
    fn reads_releases_and_their_files() {
        let list = json!([
            {"tag_name": "v0.3.0", "name": "Klue v0.3.0", "body": "- Graphs", "draft": false, "prerelease": false,
             "html_url": "https://github.com/karimawi/Klue/releases/tag/v0.3.0", "target_commitish": "main",
             "published_at": "2026-12-01T10:00:00Z",
             "assets": [
                {"name": "Klue-0.3.0-windows-x64.zip", "browser_download_url": "https://example.invalid/w.zip", "size": 9000000},
                {"name": "../evil.zip", "browser_download_url": "https://example.invalid/e.zip", "size": 1}
             ]},
            {"tag_name": "v0.4.0", "draft": true, "assets": []},
            {"tag_name": "nightly", "draft": false, "assets": []}
        ]);
        let releases = parse_releases(&list);
        assert_eq!(releases.len(), 1, "no drafts, no tags that aren't versions");
        let r = &releases[0];
        assert_eq!((r.version.to_string(), r.title.as_str(), r.notes.as_str()), ("0.3.0".into(), "Klue v0.3.0", "- Graphs"));
        assert_eq!(r.assets.len(), 1, "no path tricks in file names");
        assert_eq!(r.asset(&["-windows-x64.zip"]).map(|a| a.size), Some(9_000_000));
        assert_eq!(r.asset(&["-linux-x64.tar.gz"]), None);
    }

    /// Against GitHub: `cargo test -p klue-core --no-default-features update -- --ignored`.
    #[tokio::test]
    #[ignore = "needs the network"]
    async fn live_releases_count_only_from_main() {
        let found = newer_than(&v("0.0.1")).await.unwrap();
        println!("newest release from {BRANCH}: {:?}", found.as_ref().map(|r| &r.tag));
    }

    #[test]
    fn a_commit_is_on_main_when_main_has_it() {
        assert!(in_branch(&json!({"status": "behind"})));
        assert!(in_branch(&json!({"status": "identical"})));
        assert!(!in_branch(&json!({"status": "ahead"})));
        assert!(!in_branch(&json!({"status": "diverged"})));
        assert!(is_commit("3ebe1823b08299a8061a5742bc2ad527dd8688f1"));
        assert!(!is_commit("dev"));
    }
}
