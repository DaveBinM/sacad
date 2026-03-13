//! sacad main binary

use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser as _;
use sacad::{SearchStatus, cl, search_and_download};

#[tokio::main]
async fn main() -> anyhow::Result<SearchStatus> {
    // Parse CL args
    let cl_args = cl::SacadArgs::parse();

    // Init logger
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Error)
        .with_module_level(env!("CARGO_PKG_NAME"), cl_args.verbosity.into())
        .init()
        .context("Failed to setup logger")?;

    // Run
    let query = Arc::new(cl_args.query);
    let status = search_and_download(
        &cl_args.output_filepath,
        Arc::clone(&query),
        Arc::new(cl_args.search_opts),
        &cl_args.image_proc,
    )
    .await?;
    if matches!(status, SearchStatus::NotFound) {
        log::warn!(
            "No cover found for {} / {}",
            query.artist.as_deref().unwrap_or("(no artist)"),
            query.album,
        );
    }
    Ok(status)
}
