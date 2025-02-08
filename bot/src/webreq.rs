use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct ResolveRequest {
    status: String,
    msg: String,
}

impl From<ResolveRequest> for ResolveError {
    fn from(value: ResolveRequest) -> Self {
        Self::Upstream(value.msg)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ResolveError {
    #[error("Reqwest failed to send request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("{0}")]
    Upstream(String),
}

pub async fn resolve_moosename(
    client: &Client,
    url: &str,
    moose: &str,
) -> Result<String, ResolveError> {
    let res = client
        .get(format!(
            "{}/api-helper/resolve/{}",
            url,
            percent_encoding::percent_encode(moose.as_bytes(), percent_encoding::NON_ALPHANUMERIC)
        ))
        .send()
        .await?
        .json::<ResolveRequest>()
        .await?;
    if res.status == "error" {
        Err(ResolveError::Upstream(res.msg))
    } else {
        // is percent encoded for us by upstream, uses same code as Location: redirect for random/latest/oldest.
        Ok(res.msg)
    }
}

pub async fn get_irclines(client: &Client, url: &str, moose: &str) -> Result<String, ResolveError> {
    let res = client.get(format!("{}/irc/{}", url, moose)).send().await?;
    if res.status().is_success() {
        Ok(res.text().await?)
    } else {
        Err(res.json::<ResolveRequest>().await?.into())
    }
}
