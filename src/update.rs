use std::{env, sync::Arc};

use anyhow::Context;
use futures::AsyncReadExt;

use gpui::{
    http_client::{AsyncBody, HttpClient},
    *,
};
use semver::Version;
use serde_json::Value;

pub async fn check_update(
    http_client: Arc<dyn HttpClient>,
) -> anyhow::Result<Option<SharedString>> {
    let mut result = http_client
        .get(
            "https://api.github.com/repos/him049/fast_clip/releases/latest",
            AsyncBody::empty(),
            true,
        )
        .await?;

    if !result.status().is_success() {
        return Err(anyhow::anyhow!(format!(
            "GitHub API returned {}",
            result.status()
        )));
    }

    let mut body = String::new();
    result
        .body_mut()
        .read_to_string(&mut body)
        .await
        .context("failed to read response")?;

    let json: Value = serde_json::from_str(&body).context("failed to read json")?;

    let latest_tag = json["tag_name"]
        .as_str()
        .context("failed to read tag name")?;
    let url = json["html_url"]
        .as_str()
        .context("failed to read tag name")?;

    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;

    let latest = Version::parse(latest_tag.trim_start_matches('v'))?;

    if latest > current {
        Ok(Some(SharedString::new(url)))
    } else {
        Ok(None)
    }
}
