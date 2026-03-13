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

/// Construct a max-size URL from a Qobuz image URL by replacing the size suffix with `_org`
fn max_size_url(url: &str) -> Option<String> {
    // Qobuz image URLs end with `_{size}.jpg`, e.g. `_600.jpg`
    let without_ext = url.strip_suffix(".jpg")?;
    let (base, _size) = without_ext.rsplit_once('_')?;
    Some(format!("{base}_org.jpg"))
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
        let query = if let Some(nartist) = &nartist {
            format!("{nartist} {nalbum}")
        } else {
            nalbum.clone()
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

            let fuzzy = nartist
                .as_ref()
                .is_some_and(|nartist| &normalize(&result.artist.name) != nartist)
                || (normalize(&result.title) != nalbum);

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
