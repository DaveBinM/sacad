//! Qobuz cover source

// See https://www.qobuz.com/us-en/open-streaming-platform.html

use std::sync::Arc;

use anyhow::Context as _;
use reqwest::{
    Url,
    header::{HeaderMap, HeaderName, HeaderValue},
};

use crate::{
    cl::SourceName,
    cover::{Cover, Format, Metadata},
    http::SourceHttpClient,
    source::{self, Source, normalize},
};

/// Qobuz cover source
pub(crate) struct Qobuz;

/// Qobuz application ID
const APP_ID: &str = "798273057";

/// Default relevance for Qobuz covers
const QOBUZ_RELEVANCE: source::Relevance = source::Relevance {
    fuzzy: false,
    only_front_covers: true,
    unrelated_risk: false,
};

#[derive(Debug, serde::Deserialize)]
struct Response {
    albums: ResponseAlbums,
}

#[derive(Debug, serde::Deserialize)]
struct ResponseAlbums {
    items: Vec<ResponseAlbum>,
}

#[derive(Debug, serde::Deserialize)]
struct ResponseAlbum {
    title: String,
    /// Optional version/edition suffix, e.g. "2011 Remaster" or "LO'99 Remix"
    version: Option<String>,
    artist: ResponseArtist,
    image: Option<ResponseImage>,
}

#[derive(Debug, serde::Deserialize)]
struct ResponseArtist {
    name: String,
}

#[derive(Debug, serde::Deserialize)]
struct ResponseImage {
    thumbnail: Option<String>,
    small: Option<String>,
    large: Option<String>,
}

/// Build the full display title by combining title and optional version.
/// Qobuz stores "Fake Magic" + version "LO'99 Remix" separately; the display name is
/// "Fake Magic (LO'99 Remix)" — which matches what the user sees (and names their folders).
fn display_title(title: &str, version: Option<&str>) -> String {
    let base = title.trim();
    match version.filter(|v| !v.is_empty()) {
        Some(v) => format!("{base} ({v})"),
        None => base.to_owned(),
    }
}

/// Strip one trailing `(...)` or `[...]` group from an album name, handling nested brackets.
/// e.g., `"Mercy (Album Version (Explicit))"` → `"Mercy"`
/// e.g., `"Title [feat. X] (Remix)"` → `"Title [feat. X]"`
fn strip_one_version_suffix(s: &str) -> &str {
    let trimmed = s.trim_end();
    let (close, open) = if trimmed.ends_with(')') {
        (')', '(')
    } else if trimmed.ends_with(']') {
        (']', '[')
    } else {
        return trimmed;
    };
    // Walk backwards counting bracket depth to find the matching opener
    let mut depth = 0i32;
    for (i, c) in trimmed.char_indices().rev() {
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                return trimmed[..i].trim_end();
            }
        }
    }
    trimmed
}

/// Strip ALL trailing version/edition/remix groups from an album name.
/// e.g., `"Lay Low (Argy Remix) (Extended Mix)"` → `"Lay Low"`
/// e.g., `"Title (2011 Remaster)"` → `"Title"`
fn strip_version_info(s: &str) -> &str {
    let mut current = s.trim_end();
    loop {
        let stripped = strip_one_version_suffix(current);
        if stripped == current {
            return current;
        }
        current = stripped;
    }
}

/// Construct a max-size URL from a Qobuz image URL by replacing the size suffix with `_org`
fn max_size_url(url: &str) -> Option<String> {
    // Qobuz image URLs end with `_{size}.jpg`, e.g. `_600.jpg`
    let without_ext = url.strip_suffix(".jpg")?;
    let (base, _size) = without_ext.rsplit_once('_')?;
    Some(format!("{base}_org.jpg"))
}

/// Issue one Qobuz album search and build `Cover` results.
/// `nalbum` is the full normalized album name used for fuzzy detection.
async fn query_covers(
    nartist: &Option<String>,
    query_album: &str,
    nalbum: &str,
    http: &mut Arc<SourceHttpClient>,
) -> anyhow::Result<Vec<Cover>> {
    let query = if let Some(na) = nartist {
        format!("{na} {query_album}")
    } else {
        query_album.to_owned()
    };
    let url_params = [
        ("query", query.as_str()),
        ("app_id", APP_ID),
        ("limit", "20"),
    ];
    #[expect(clippy::unwrap_used)] // base URL is absolute
    let search_url =
        Url::parse_with_params("https://www.qobuz.com/api.json/0.2/album/search", url_params)
            .unwrap();

    let resp: Response = http.get_json(search_url).await?;

    let mut results = Vec::new();
    for (rank, result) in resp.albums.items.into_iter().enumerate() {
        let Some(image) = result.image else {
            continue;
        };
        let Some(thumbnail_url_str) = image.thumbnail.as_deref() else {
            continue;
        };
        let thumbnail_url: Url = thumbnail_url_str
            .parse()
            .with_context(|| format!("Failed to parse thumbnail URL {thumbnail_url_str:?}"))?;

        let dtitle = display_title(&result.title, result.version.as_deref());
        let fuzzy = nartist
            .as_ref()
            .is_some_and(|na| &normalize(&result.artist.name) != na)
            || normalize(&dtitle) != nalbum;

        let relevance = source::Relevance {
            fuzzy,
            ..QOBUZ_RELEVANCE
        };

        // Add known sizes from the API response
        for (url_opt, size) in [
            (image.small.as_deref(), 230_u32),
            (image.large.as_deref(), 600),
        ] {
            let Some(url_str) = url_opt else {
                continue;
            };
            let url: Url = url_str
                .parse()
                .with_context(|| format!("Failed to parse cover URL {url_str:?}"))?;
            results.push(Cover {
                url,
                thumbnail_url: thumbnail_url.clone(),
                size_px: Metadata::known((size, size)),
                format: Metadata::known(Format::Jpeg),
                source_name: SourceName::Qobuz,
                source_http: Arc::clone(http),
                relevance: relevance.clone(),
                rank,
            });
        }

        // Attempt a max-size variant by replacing the size suffix with `_org`
        if let Some(large_url_str) = image.large.as_deref() {
            if let Some(max_url_str) = max_size_url(large_url_str) {
                if let Ok(max_url) = max_url_str.parse::<Url>() {
                    results.push(Cover {
                        url: max_url,
                        thumbnail_url: thumbnail_url.clone(),
                        // Actual size is unknown; use a generous hint
                        size_px: Metadata::uncertain((1500, 1500)),
                        format: Metadata::known(Format::Jpeg),
                        source_name: SourceName::Qobuz,
                        source_http: Arc::clone(http),
                        relevance: relevance.clone(),
                        rank,
                    });
                }
            }
        }
    }

    Ok(results)
}

#[async_trait::async_trait]
impl Source for Qobuz {
    async fn search(
        &self,
        artist: Option<&str>,
        album: &str,
        http: &mut Arc<SourceHttpClient>,
    ) -> anyhow::Result<Vec<Cover>> {
        let nartist = artist.map(normalize);
        let nalbum = normalize(album);

        // Try the full album name first so we match the exact edition/version.
        let results = query_covers(&nartist, &nalbum, &nalbum, http).await?;
        if !results.is_empty() {
            return Ok(results);
        }

        // If the album name contains a trailing parenthetical (e.g. "(2011 Remaster)"),
        // the Qobuz search API may return nothing for it. Fall back to the base name.
        let nalbum_base = normalize(strip_version_info(album));
        if nalbum_base != nalbum {
            let results = query_covers(&nartist, &nalbum_base, &nalbum, http).await?;
            if !results.is_empty() {
                return Ok(results);
            }
        }

        // For multi-title releases (e.g. double A-sides "Song A (Extended) / Song B (Extended)"),
        // "/" URL-encodes to %2F which Qobuz search cannot handle. Try just the first title,
        // with its own version suffix stripped.
        let pre_slash = nalbum_base
            .split_once(" / ")
            .or_else(|| nalbum.split_once(" / "))
            .map(|(pre, _)| normalize(strip_version_info(pre)));
        if let Some(first_title) = pre_slash {
            if !first_title.is_empty() && first_title != nalbum_base && first_title != nalbum {
                return query_covers(&nartist, &first_title, &nalbum, http).await;
            }
        }

        Ok(Vec::new())
    }

    fn common_headers(&self) -> HeaderMap {
        [(
            HeaderName::from_static("x-app-id"),
            HeaderValue::from_static(APP_ID),
        )]
        .into_iter()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::tests::{
        source_has_results, source_has_results_compilation, source_no_results,
    };

    #[test]
    fn strip_version_info_simple() {
        assert_eq!(strip_version_info("A Kind Of Magic (2011 Remaster)"), "A Kind Of Magic");
        assert_eq!(strip_version_info("In the Dark (Remixes)"), "In the Dark");
        assert_eq!(strip_version_info("No parens"), "No parens");
    }

    #[test]
    fn strip_version_info_nested() {
        // rfind would have returned "Mercy (Album Version" — nested bracket matching fixes this
        assert_eq!(
            strip_version_info("Mercy (Album Version (Explicit))"),
            "Mercy"
        );
    }

    #[test]
    fn strip_version_info_multiple_groups() {
        assert_eq!(
            strip_version_info("Lay Low (Argy Remix) (Extended Mix)"),
            "Lay Low"
        );
    }

    #[test]
    fn strip_version_info_square_brackets() {
        // All trailing groups stripped: "(Remixes)" → "[feat. Kid Ink]" → "(Boneless)"
        assert_eq!(
            strip_version_info("Delirious (Boneless) [feat. Kid Ink] (Remixes)"),
            "Delirious"
        );
    }

    #[tokio::test]
    async fn has_results() {
        let _ = simple_logger::init_with_env();
        let source = Qobuz;
        source_has_results(source, SourceName::Qobuz).await;
    }

    #[tokio::test]
    async fn has_results_compilation() {
        let _ = simple_logger::init_with_env();
        let source = Qobuz;
        source_has_results_compilation(source, SourceName::Qobuz).await;
    }

    #[tokio::test]
    async fn has_no_results() {
        let _ = simple_logger::init_with_env();
        let source = Qobuz;
        source_no_results(source, SourceName::Qobuz).await;
    }
}
