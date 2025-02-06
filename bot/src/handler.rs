use std::{collections::HashSet, sync::Arc};

use irc::proto::{command::Numeric, Command, Message, Source, User};
use tokio::sync::{mpsc::Sender, Mutex, RwLock};

use crate::{
    helpers::{join_channels, CONFLICT_FILLER},
    tasks::{invite::InviteMsg, sender::SendMsg},
};

pub struct IrcState {
    original_nick: String,
    current_nick: String,
    nickserv_pass: Option<String>,
    channels: HashSet<String>,
    moose_url: String,
    moose_lock: Mutex<()>,
}

impl IrcState {
    pub fn new(
        nick: String,
        nickserv_pass: Option<String>,
        channels: HashSet<String>,
        moose_url: String,
    ) -> Self {
        Self {
            original_nick: nick.clone(),
            current_nick: nick,
            nickserv_pass,
            channels,
            moose_url,
            moose_lock: Mutex::new(()),
        }
    }
}

const HELP_RESP: &str =
    "usage: ^[.!]?moose(?:img|search|me)? [--latest|--random|--search|--image|--] moosename";

enum MComm {
    Help,
    Bots,
    Search(String),
    Image(String),
    Irc(String),
}

impl<'a> From<(PComm, &'a str)> for MComm {
    fn from(value: (PComm, &'a str)) -> Self {
        let m = value.1.trim().to_owned();
        match value.0 {
            PComm::Search => Self::Search(m),
            PComm::Image => Self::Image(m),
            PComm::Irc => Self::Irc(m),
        }
    }
}

enum PComm {
    Search,
    Image,
    Irc,
}

fn parse_moose_args(msg: &str) -> Option<MComm> {
    // we need any whitespace.
    let mut iter = msg.split(|c: char| c.is_ascii_whitespace());
    let comm = match iter.next()? {
        ".moose" | "!moose" | "moose" | ".mooseme" | "!mooseme" | "mooseme" => PComm::Irc,
        ".mooseimg" | "!mooseimg" | "mooseimg" => PComm::Image,
        ".moosesearch" | "!moosesearch" | "moosesearch" => PComm::Search,
        ".bots" | "!bots" => return Some(MComm::Bots),
        ".help" | "!help" => return Some(MComm::Help),
        _ => return None,
    };
    let arg = iter.next().unwrap_or("random");
    let rest = iter.collect::<Vec<&str>>().join(" ");
    match arg {
        "" | "--" => Some((comm, rest.as_str()).into()),
        "-h" | "--help" => Some(MComm::Help),
        "-s" | "--search" => Some((PComm::Search, rest.as_str()).into()),
        "-i" | "--image" => Some((PComm::Image, rest.as_str()).into()),
        "-r" | "--random" => Some((comm, "random").into()),
        "-l" | "--latest" => Some((comm, "latest").into()),
        "-o" | "--oldest" => Some((comm, "oldest").into()),
        _ => Some((comm, [arg, &rest].join(" ").as_str()).into()),
    }
}

pub async fn handle(
    state: Arc<RwLock<IrcState>>,
    msg: Message,
    disable_search: bool,
    sendi: Sender<InviteMsg>,
    sendo: Sender<SendMsg>,
) {
    let sender = match msg.source {
        Some(Source::Server(server)) => server,
        Some(Source::User(User { nickname, .. })) => nickname,
        None => "".to_owned(),
    };
    let rstate = state.read().await;
    let _ = match msg.command {
        Command::PING(pong) => {
            sendo
                .send(SendMsg::Immediate(Command::PONG(pong, None).into()))
                .await
        }
        Command::ERROR(banned) => {
            eprintln!("ERR: [irc] Banned (?): {banned}");
            sendo
                .send(SendMsg::Immediate(Command::QUIT(None).into()))
                .await
        }
        Command::JOIN(channel, _) if rstate.current_nick == sender => {
            eprintln!("INFO: [irc] Joined {channel}");
            Ok(())
        }
        // shouldn't happen?
        Command::PART(channel, _) if rstate.current_nick == sender => {
            eprintln!("INFO: [irc] Parted {channel}");
            let _ = sendi.try_send(InviteMsg::Kicked(channel));
            Ok(())
        }
        Command::INVITE(target, channel) if rstate.current_nick == target => {
            let _ = sendi.try_send(InviteMsg::Joined(channel.clone()));
            sendo
                .send(SendMsg::Delayed(Command::JOIN(channel, None).into()))
                .await
        }
        Command::KICK(channel, target, reason) if rstate.current_nick == target => {
            eprintln!(
                "INFO: [irc] Kicked from {channel} by {sender}; reason: {}",
                reason.unwrap_or_default()
            );
            let _ = sendi.try_send(InviteMsg::Kicked(channel));
            Ok(())
        }
        Command::PRIVMSG(channel, msg)
            if rstate.current_nick == channel && msg == "\x01VERSION\x01" =>
        {
            sendo
                .send(SendMsg::Delayed(
                    Command::NOTICE(
                        sender,
                        format!(
                            "\x01VERSION {}/{}\x01",
                            env!("CARGO_PKG_NAME"),
                            env!("CARGO_PKG_VERSION")
                        ),
                    )
                    .into(),
                ))
                .await
        }
        Command::PRIVMSG(channel, msg) => {
            if let Some(comm) = parse_moose_args(&msg) {
                let resp = match comm {
                    MComm::Help => HELP_RESP.to_owned(),
                    MComm::Bots => format!(
                        "Moose :: Make moose @ {} :: See .moose --help for usage",
                        rstate.moose_url
                    ),
                    MComm::Search(q) if disable_search => format!(
                        "Search has been disabled on this server. See: {}/gallery/0?q={}",
                        rstate.moose_url,
                        percent_encoding::percent_encode(
                            q.as_bytes(),
                            percent_encoding::NON_ALPHANUMERIC
                        )
                    ),
                    MComm::Search(_) => todo!(),
                    MComm::Image(_) => todo!(),
                    MComm::Irc(_) => todo!(),
                };
                sendo
                    .send(SendMsg::Delayed(Command::PRIVMSG(channel, resp).into()))
                    .await
            } else {
                Ok(())
            }
        }
        Command::Numeric(num, _params) => match num {
            Numeric::RPL_WELCOME => {
                if let Some(ref npass) = rstate.nickserv_pass {
                    let _ = sendo
                        .send(SendMsg::Immediate(
                            Command::Raw(format!("NICKSERV IDENTIFY :{npass}")).into(),
                        ))
                        .await;
                }
                let mut joins = join_channels(&rstate.channels);
                loop {
                    if let Some(j) = joins.next() {
                        if let Err(e) = sendo.send(SendMsg::Immediate(j.into())).await {
                            break Err(e);
                        }
                    } else {
                        break Ok(());
                    }
                }
            }
            Numeric::ERR_ERRONEUSNICKNAME => {
                eprintln!("ERR: [irc] Server does not like our nickname.");
                sendo
                    .send(SendMsg::Immediate(Command::QUIT(None).into()))
                    .await
            }
            Numeric::ERR_NICKNAMEINUSE => {
                drop(rstate);
                let mut wstate = state.write().await;
                wstate.current_nick.push_str(CONFLICT_FILLER);
                if wstate.current_nick.len() - wstate.original_nick.len() > 3 {
                    eprintln!("ERR: [irc] Server asked us to rename ourselves too many times.");
                    let _ = sendo
                        .send(SendMsg::Immediate(Command::QUIT(None).into()))
                        .await;
                    return;
                }
                sendo
                    .send(SendMsg::Immediate(
                        Command::NICK(wstate.current_nick.clone()).into(),
                    ))
                    .await
            }
            _ => Ok(()),
        },
        _ => Ok(()),
    };
}
