//! Where release information comes from.
//!
//! [`GitHubSource`] asks the GitHub REST API (one request per app, revalidated with ETags) and,
//! when the API is out of quota, falls back to github.com's release pages: the
//! `releases/latest` redirect names the tag and `SHA256SUMS.txt` lists every file with its
//! checksum, which costs no API quota at all. A later [`ReleaseSource`] will ask the ArtCraft
//! catalog API for every app in one request.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use crate::protocol::{LatestRelease, Source};
use crate::release::{self, Release};
use serde::{Deserialize, Serialize};

use crate::http::{Fetched, Http};

/// One answer from a source.
#[derive(Clone, Debug)]
pub struct Answer {
    pub latest: LatestRelease,
    pub source: Source,
}

/// Looks up latest releases. Called from worker threads.
pub trait ReleaseSource: Send + Sync {
    fn latest(&self, repo: &str, prereleases: bool) -> Answer;
    /// The last answer saved for `repo`, for an instant first paint.
    fn cached(&self, repo: &str, prereleases: bool) -> Option<LatestRelease>;
    /// The tag of `repo`'s newest stable release, by a route that costs no API quota (GitHub's
    /// `releases/latest` redirect), or `None` when it can't be told. Quick: a check pressed by the
    /// user asks this for every app to see whether the feed has fallen behind.
    fn newest_tag(&self, repo: &str) -> Option<String> {
        let _ = repo;
        None
    }
    /// API refused until this time (Unix seconds).
    fn rate_limited_until(&self) -> Option<i64>;
    /// Persist anything worth keeping.
    fn save(&self);
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Entry {
    etag: Option<String>,
    /// `None`: the repo has no releases.
    release: Option<Release>,
    prereleases: bool,
    fetched_at: i64,
}

#[derive(Default, Serialize, Deserialize)]
struct CacheFile {
    entries: BTreeMap<String, Entry>,
}

pub struct GitHubSource {
    http: Http,
    cache: Mutex<CacheFile>,
    cache_path: std::path::PathBuf,
    limited_until: Mutex<Option<i64>>,
}

impl GitHubSource {
    pub fn new(http: Http, cache_path: std::path::PathBuf) -> Self {
        let cache = std::fs::read(&cache_path).ok().and_then(|b| serde_json::from_slice::<CacheFile>(&b).ok()).unwrap_or_default();
        Self { http, cache: Mutex::new(cache), cache_path, limited_until: Mutex::new(None) }
    }

    fn entry(&self, repo: &str, prereleases: bool) -> Option<Entry> {
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        cache.entries.get(repo).filter(|e| e.prereleases == prereleases).cloned()
    }

    fn store(&self, repo: &str, entry: Entry) {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner).entries.insert(repo.to_string(), entry);
    }

    fn limited(&self) -> bool {
        let until = *self.limited_until.lock().unwrap_or_else(PoisonError::into_inner);
        until.is_some_and(|u| u > crate::now())
    }

    fn ask_api(&self, repo: &str, prereleases: bool) -> Result<Option<Answer>, String> {
        let url = if prereleases {
            format!("https://api.github.com/repos/{repo}/releases?per_page=30")
        } else {
            format!("https://api.github.com/repos/{repo}/releases/latest")
        };
        let saved = self.entry(repo, prereleases);
        let found =
            |release: Option<Release>| Answer { latest: release.map_or(LatestRelease::None, LatestRelease::Found), source: Source::GitHubApi };
        match self.http.get_api(&url, saved.as_ref().and_then(|e| e.etag.as_deref()))? {
            Fetched::Body { text, etag } => {
                let release = if prereleases {
                    release::parse_github_release_list(&text, repo, true)?
                } else {
                    Some(release::parse_github_release(&text, repo)?)
                };
                self.store(repo, Entry { etag, release: release.clone(), prereleases, fetched_at: crate::now() });
                Ok(Some(found(release)))
            }
            Fetched::NotModified => match saved {
                Some(e) => {
                    self.store(repo, Entry { fetched_at: crate::now(), ..e.clone() });
                    Ok(Some(found(e.release)))
                }
                None => Err("GitHub said nothing changed, but there's no saved answer".into()),
            },
            Fetched::NotFound => {
                self.store(repo, Entry { etag: None, release: None, prereleases, fetched_at: crate::now() });
                Ok(Some(found(None)))
            }
            Fetched::RateLimited { reset } => {
                *self.limited_until.lock().unwrap_or_else(PoisonError::into_inner) = Some(retry_at(reset, crate::now()));
                Ok(None)
            }
        }
    }

    /// The quota-free route through github.com (stable releases only).
    fn ask_pages(&self, repo: &str) -> Result<Answer, String> {
        let location = self.http.redirect_target(&format!("https://github.com/{repo}/releases/latest"))?;
        let Some(tag) = location.as_deref().and_then(|l| release::tag_from_redirect(l, repo)) else {
            // No published release: github.com sends us to the release list instead.
            return Ok(Answer { latest: LatestRelease::None, source: Source::GitHubPages });
        };
        if let Some(e) = self.entry(repo, false)
            && let Some(r) = e.release.as_ref().filter(|r| r.tag == tag)
        {
            return Ok(Answer { latest: LatestRelease::Found(r.clone()), source: Source::GitHubPages });
        }
        let sums_url = format!("{}{tag}/SHA256SUMS.txt", release::download_prefix(repo));
        let sums = self.http.get_text(&sums_url)?.map(|t| release::parse_sha256sums(&t)).unwrap_or_default();
        let r = release::release_from_sums(repo, &tag, &sums);
        self.store(repo, Entry { etag: None, release: Some(r.clone()), prereleases: false, fetched_at: crate::now() });
        Ok(Answer { latest: LatestRelease::Found(r), source: Source::GitHubPages })
    }
}

/// GitHub's quota refills within the hour, so a later reset is a bad answer.
const MAX_LIMIT_WAIT: i64 = 60 * 60;

/// When to ask the API again after a refusal.
fn retry_at(reset: Option<i64>, now: i64) -> i64 {
    reset.unwrap_or(now + 15 * 60).min(now + MAX_LIMIT_WAIT)
}

/// The former repositories of `repo`'s app (see [`release::repo_aliases`] without `repo` itself).
fn former_repos(repo: &str) -> Vec<&str> {
    release::repo_aliases(repo).into_iter().filter(|r| *r != repo).collect()
}

/// Asks `ask` about `repo` first and, only when that repository says it has no releases, about
/// each former repository in order, answering with the first that has releases (or fails). A rate limit or any
/// other failed answer (including one served from the cache) ends the search: those say nothing
/// about the repository, so another one is not asked.
fn latest_or_former(repo: &str, ask: impl Fn(&str) -> Answer) -> Answer {
    let answer = ask(repo);
    if !is_no_releases(&answer) {
        return answer;
    }
    for former in former_repos(repo) {
        let older = ask(former);
        if !is_no_releases(&older) {
            return older;
        }
    }
    answer
}

/// An answer that says the repository has no releases (not one that fell back to the cache).
fn is_no_releases(answer: &Answer) -> bool {
    answer.latest == LatestRelease::None && answer.source != Source::Cache
}

impl GitHubSource {
    /// [`ReleaseSource::latest`] for exactly `repo`, former repositories not consulted.
    fn latest_of(&self, repo: &str, prereleases: bool) -> Answer {
        let api_error = if self.limited() {
            None
        } else {
            match self.ask_api(repo, prereleases) {
                Ok(Some(answer)) => return answer,
                Ok(None) => None,
                Err(e) => Some(e),
            }
        };
        if !prereleases && api_error.is_none() {
            match self.ask_pages(repo) {
                Ok(answer) => return answer,
                Err(e) => log::warn!("{repo}: release pages: {e}"),
            }
        }
        let why = api_error.unwrap_or_else(|| "GitHub's request limit was reached".into());
        match self.cached_of(repo, prereleases) {
            Some(latest) => Answer { latest, source: Source::Cache },
            None => Answer { latest: LatestRelease::Failed(why), source: Source::Cache },
        }
    }

    /// The last answer saved for exactly `repo`.
    fn cached_of(&self, repo: &str, prereleases: bool) -> Option<LatestRelease> {
        self.entry(repo, prereleases).map(|e| e.release.map_or(LatestRelease::None, LatestRelease::Found))
    }

    fn newest_tag_of(&self, repo: &str) -> Option<String> {
        match self.http.redirect_target(&format!("https://github.com/{repo}/releases/latest")) {
            Ok(location) => location.as_deref().and_then(|l| release::tag_from_redirect(l, repo)),
            Err(e) => {
                log::debug!("{repo}: couldn't read the latest-release redirect: {e}");
                None
            }
        }
    }
}

impl ReleaseSource for GitHubSource {
    fn latest(&self, repo: &str, prereleases: bool) -> Answer {
        latest_or_former(repo, |r| self.latest_of(r, prereleases))
    }

    /// The current repository's saved answer, or the first former repository's that has a release.
    fn cached(&self, repo: &str, prereleases: bool) -> Option<LatestRelease> {
        let mut first = self.cached_of(repo, prereleases);
        if matches!(first, Some(LatestRelease::None) | None) {
            for former in former_repos(repo) {
                match self.cached_of(former, prereleases) {
                    found @ Some(LatestRelease::Found(_)) => return found,
                    other => first = first.or(other),
                }
            }
        }
        first
    }

    fn newest_tag(&self, repo: &str) -> Option<String> {
        std::iter::once(repo).chain(former_repos(repo)).find_map(|r| self.newest_tag_of(r))
    }

    fn rate_limited_until(&self) -> Option<i64> {
        let until = *self.limited_until.lock().unwrap_or_else(PoisonError::into_inner);
        until.filter(|u| *u > crate::now())
    }

    fn save(&self) {
        let json = {
            let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
            serde_json::to_vec(&*cache)
        };
        match json {
            Ok(bytes) => {
                if let Err(e) = crate::paths::write_atomic(&self.cache_path, &bytes) {
                    log::warn!("release cache: {e}");
                }
            }
            Err(e) => log::warn!("release cache: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_waits_for_the_reset_github_names() {
        assert_eq!(retry_at(Some(1_000 + 600), 1_000), 1_600);
        assert_eq!(retry_at(None, 1_000), 1_000 + 15 * 60);
    }

    #[test]
    fn a_reset_past_the_quota_window_cannot_stop_the_api_for_good() {
        assert_eq!(retry_at(Some(i64::MAX), 1_000), 1_000 + 60 * 60);
        assert_eq!(retry_at(Some(99_999_999_999), 1_000), 1_000 + 60 * 60);
    }

    const NEW: &str = "Styx-Studios/Local-Writer";
    const OLD: &str = "Anon5T4R/local-office";

    fn release_tagged(tag: &str) -> Release {
        Release { tag: tag.into(), ..Release::default() }
    }

    fn said(latest: LatestRelease, source: Source) -> Answer {
        Answer { latest, source }
    }

    /// Looks `repo` up as the repositories answer (by name); returns the answer and who was asked, in order.
    fn lookup(answers: &[(&str, Answer)]) -> (Answer, Vec<String>) {
        let asked = std::cell::RefCell::new(Vec::new());
        let answer = latest_or_former(NEW, |repo| {
            asked.borrow_mut().push(repo.to_string());
            answers.iter().find(|(r, _)| *r == repo).map(|(_, a)| a.clone()).expect("an unexpected repository was asked")
        });
        (answer, asked.into_inner())
    }

    #[test]
    fn a_repository_with_releases_is_the_only_one_asked() {
        let (answer, asked) = lookup(&[(NEW, said(LatestRelease::Found(release_tagged("v2")), Source::GitHubApi))]);
        assert_eq!(answer.latest, LatestRelease::Found(release_tagged("v2")));
        assert_eq!(asked, [NEW]);
    }

    #[test]
    fn a_repository_without_releases_falls_back_to_the_former_one() {
        let (answer, asked) = lookup(&[
            (NEW, said(LatestRelease::None, Source::GitHubApi)),
            (OLD, said(LatestRelease::Found(release_tagged("v1")), Source::GitHubApi)),
        ]);
        assert_eq!(answer.latest, LatestRelease::Found(release_tagged("v1")));
        assert_eq!(asked, [NEW, OLD]);
        // The quota-free route's "no release" counts the same.
        let (answer, _) = lookup(&[
            (NEW, said(LatestRelease::None, Source::GitHubPages)),
            (OLD, said(LatestRelease::Found(release_tagged("v1")), Source::GitHubPages)),
        ]);
        assert!(matches!(answer.latest, LatestRelease::Found(_)));
    }

    #[test]
    fn nothing_anywhere_is_no_releases() {
        let (answer, asked) = lookup(&[(NEW, said(LatestRelease::None, Source::GitHubApi)), (OLD, said(LatestRelease::None, Source::GitHubApi))]);
        assert_eq!(answer.latest, LatestRelease::None);
        assert_eq!(asked, [NEW, OLD]);
    }

    #[test]
    fn a_rate_limit_or_an_error_does_not_fall_back() {
        // A limit or error reaches the caller as an answer from the cache (see `latest_of`): even a
        // cached "no releases" says nothing about the repository.
        for refused in [
            said(LatestRelease::Failed("GitHub's request limit was reached".into()), Source::Cache),
            said(LatestRelease::None, Source::Cache),
            said(LatestRelease::Found(release_tagged("v0")), Source::Cache),
        ] {
            let (answer, asked) = lookup(&[(NEW, refused.clone())]);
            assert_eq!(answer.latest, refused.latest);
            assert_eq!(asked, [NEW]);
        }
    }

    #[test]
    fn a_former_repository_that_fails_ends_the_search_with_its_error() {
        let failed = said(LatestRelease::Failed("offline".into()), Source::Cache);
        let (answer, asked) = lookup(&[(NEW, said(LatestRelease::None, Source::GitHubApi)), (OLD, failed.clone())]);
        assert_eq!(answer.latest, failed.latest);
        assert_eq!(asked, [NEW, OLD]);
    }

    #[test]
    fn an_app_without_former_repositories_is_asked_once() {
        let asked = std::cell::RefCell::new(Vec::new());
        let answer = latest_or_former("storytold/photocraft", |repo| {
            asked.borrow_mut().push(repo.to_string());
            said(LatestRelease::None, Source::GitHubApi)
        });
        assert_eq!(answer.latest, LatestRelease::None);
        assert_eq!(asked.into_inner(), ["storytold/photocraft"]);
    }
}
