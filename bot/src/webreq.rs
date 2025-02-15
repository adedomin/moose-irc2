use percent_encoding::PercentEncode;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct ResolveRequest {
    status: String,
    msg: String,
}

#[derive(Deserialize)]
struct SearchResult {
    result: Vec<SearchMoose>,
}

#[derive(Deserialize)]
struct SearchMoose {
    page: usize,
    moose: SearchMooseName,
}

#[derive(Deserialize)]
struct SearchMooseName {
    name: String,
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

fn urlencode(q: &[u8]) -> PercentEncode<'_> {
    percent_encoding::percent_encode(q, percent_encoding::NON_ALPHANUMERIC)
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
            urlencode(moose.as_bytes()),
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

// note this api should always succeed.
pub async fn get_search(client: &Client, url: &str, query: &str) -> Result<String, ResolveError> {
    let resp = client
        .get(format!(
            "{}/search?p=0&q={}",
            url,
            urlencode(query.as_bytes())
        ))
        .send()
        .await?
        .json::<SearchResult>()
        .await?
        .result
        .into_iter()
        .fold((String::with_capacity(256), true), |(mut acc, first), s| {
            if first {
                acc.push_str(format!("\u{2}{}\u{2} p.{}", s.moose.name, s.page).as_str());
                (acc, false)
            } else {
                acc.push_str(format!(", \u{2}{}\u{2} p.{}", s.moose.name, s.page).as_str());
                (acc, false)
            }
        })
        .0;
    if resp.is_empty() {
        Ok("Error: No results found.".to_owned())
    } else {
        Ok(resp)
    }
}
