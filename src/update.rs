use std::{env, sync::Arc};

use anyhow::Context;
use futures::AsyncReadExt;

use gpui::{
    http_client::{AsyncBody, HttpClient},
    *,
};
use gpui_component::WindowExt;
use rust_i18n::t;
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

pub fn show_update_dialog(app_window: AnyWindowHandle, url: SharedString, cx: &mut AsyncApp) {
    app_window
        .update(cx, move |_, window, cx| {
            window.open_alert_dialog(cx, move |alert, _, _| {
                let url = url.clone();
                alert
                    .title(t!("update_dialog.title"))
                    .description(t!("update_dialog.description"))
                    .show_cancel(true)
                    .on_ok(move |_, _, cx| {
                        cx.open_url(url.as_ref());
                        true
                    })
            });
        })
        .unwrap();
}
