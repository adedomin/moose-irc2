use std::{collections::HashSet, num::NonZero, time::Duration};

use governor::{
    Quota, RateLimiter,
    clock::{Clock as _, DefaultClock},
    state::{InMemoryState, NotKeyed},
};

pub const APP_NAME: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub enum MooseLim {
    None,
    RateLim(RateLimiter<NotKeyed, InMemoryState, DefaultClock>),
}

impl MooseLim {
    pub fn check(&self) -> Result<(), u64> {
        match self {
            MooseLim::None => Ok(()),
            MooseLim::RateLim(rl) => {
                if let Err(not_until) = rl.check() {
                    let start = rl.clock().now();
                    let retry_after = not_until.wait_time_from(start).as_secs();
                    Err(retry_after)
                } else {
                    Ok(())
                }
            }
        }
    }
}

pub struct IrcState {
    pub original_nick: String,
    pub current_nick: String,
    pub nickserv_pass: Option<String>,
    pub channels: HashSet<String>,
    pub moose_url: String,
    pub moose_client: reqwest::Client,
    pub moose_delay: MooseLim,
}

impl IrcState {
    pub fn new(
        nick: String,
        nickserv_pass: Option<String>,
        channels: HashSet<String>,
        moose_url: String,
        moose_delay: Duration,
    ) -> Self {
        let moose_delay = if moose_delay.is_zero() {
            MooseLim::None
        } else {
            MooseLim::RateLim(RateLimiter::direct(
                Quota::with_period(moose_delay)
                    .unwrap()
                    .allow_burst(NonZero::<u32>::new(1).unwrap()),
            ))
        };
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
