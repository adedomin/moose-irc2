use std::{collections::HashSet, time::Duration};

use leaky_bucket::RateLimiter;

pub const APP_NAME: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub struct IrcState {
    pub original_nick: String,
    pub current_nick: String,
    pub nickserv_pass: Option<String>,
    pub channels: HashSet<String>,
    pub moose_url: String,
    pub moose_client: reqwest::Client,
    pub moose_delay: RateLimiter,
}

impl IrcState {
    pub fn new(
        nick: String,
        nickserv_pass: Option<String>,
        channels: HashSet<String>,
        moose_url: String,
        moose_delay: Duration,
    ) -> Self {
        // TODO: better way of handling this...
        let moose_delay = RateLimiter::builder()
            .fair(false)
            .max(1)
            .initial(1)
            .interval(if moose_delay.is_zero() {
                Duration::from_secs(1)
            } else {
                moose_delay
            })
            .refill(1)
            .build();
        let moose_client = reqwest::Client::builder()
            .user_agent(APP_NAME)
            .timeout(Duration::from_secs(5))
            .build()
            .expect("FATAL: [irc] Expected to build HTTP client.");
        Self {
            original_nick: nick.clone(),
            current_nick: nick,
            nickserv_pass,
            channels,
            moose_url,
            moose_client,
            moose_delay,
        }
    }
}
