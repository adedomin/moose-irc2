use std::sync::Arc;

use irc::proto::{Command, Message, Source, User, command::Numeric};
use tokio::sync::{Semaphore, mpsc::Sender};

use crate::{
    capture_clone, debug,
    handlers::ircstate::MooseLim,
    helpers::{CONFLICT_FILLER, join_channels},
    tasks::{invite::InviteMsg, sender},
    webreq::{get_irclines, get_search, resolve_moosename},
};

use super::{
    ircstate::{APP_NAME, IrcState},
    moosecmd::{HELP_RESP, MComm, parse_moose_args},
};

async fn handle_priv_cmd(
    comm: MComm,
    target: String,
    sender: String,
    disable_search: bool,
    url: String,
    limiter: Arc<MooseLim>,
    http_client: reqwest::Client,
    sendo: sender::Sender,
) {
    let resp = match comm {
        MComm::Help => HELP_RESP.to_owned(),
        MComm::Bots => format!(
            "Moose :: Make moose @ {} :: See .moose --help for usage",
            url
        ),
        MComm::Search(q) if disable_search => format!(
            "Search has been disabled on this server. See: {}/gallery/0?q={}",
            url,
            percent_encoding::percent_encode(q.as_bytes(), percent_encoding::NON_ALPHANUMERIC)
        ),
        MComm::Search(q) => get_search(&http_client, &url, &q)
            .await
            .unwrap_or_else(|e| e.to_string()),
        MComm::Image(q) => match resolve_moosename(&http_client, &url, &q).await {
            Ok(moose) => format!("{}/img/{moose}", url),
            Err(e) => e.to_string(),
        },
        MComm::Irc(q) => {
            match resolve_moosename(&http_client, &url, &q).await {
                Ok(moose) => {
                    // TODO: fix this crap.
                    match limiter.check() {
                        Ok(_) => match get_irclines(&http_client, &url, &moose).await {
                            Ok(lines) => {
                                lines.lines().for_each(|line| {
                                    sendo.send_moose(
                                        Command::PRIVMSG(target.clone(), line.to_owned()).into(),
                                    )
                                });
                                return;
                            }
                            Err(e) => e.to_string(),
                        },
                        Err(retry_after) => {
                            let plural = if retry_after != 1 { "s" } else { "" };
                            sendo.lossy_send(
                                            Command::NOTICE(
                                                sender,
                                                format!("Please wait ~{retry_after} second{plural} before asking for another moose."),
                                            )
                                            .into(),
                                        );
                            return;
                        }
                    }
                }
                Err(e) => e.to_string(),
            }
        }
    };
    sendo
        .send(Command::PRIVMSG(target.to_owned(), resp).into())
        .await;
}

pub fn handle(
    state: &IrcState,
    task_lim: &Semaphore,
    msg: Message,
    disable_search: bool,
    sendo: sender::Sender,
    sendi: Sender<InviteMsg>,
) -> Option<String> {
    macro_rules! spawn_task {
        ( $($fut:tt)* ) => {{
            if let Ok(s) = task_lim.try_acquire() {
                tokio::spawn(async move { $($fut)*.await; });
                drop(s);
            } else {
                eprintln!("WARN: [irc] Too many I/O tasks; dropping messages.");
            }
        }};
    }
    let sender = match msg.source {
        Some(Source::Server(server)) => server,
        Some(Source::User(User { nickname, .. })) => nickname,
        None => "".to_owned(),
    };
    match msg.command {
        Command::PING(pong) => {
            spawn_task!(sendo.send(Command::PONG(pong, None).into()))
        }
        Command::PONG(_pong, _) => {
            debug!("DEBUG: [irc] recv PONG {_pong}");
        }
        Command::ERROR(banned) => {
            eprintln!("ERR: [irc] Banned (?): {banned}");
            spawn_task!(sendo.send(Command::QUIT(None).into()));
        }
        Command::JOIN(channel, _) if state.current_nick == sender => {
            eprintln!("INFO: [irc] Joined {channel}");
        }
        // shouldn't happen?
        Command::PART(channel, _) if state.current_nick == sender => {
            eprintln!("INFO: [irc] Parted {channel}");
            tokio::spawn(async move {
                let _ = sendi.send(InviteMsg::Kicked(channel)).await;
            });
        }
        Command::INVITE(target, channel) if state.current_nick == target => {
            if sendi.try_send(InviteMsg::Joined(channel.clone())).is_ok() {
                spawn_task!(sendo.send(Command::JOIN(channel, None).into()))
            } else {
                spawn_task!(
                    sendo.send(Command::NOTICE(sender, "Invites are disabled.".to_owned()).into())
                )
            }
        }
        Command::KICK(channel, target, reason) if state.current_nick == target => {
            eprintln!(
                "INFO: [irc] Kicked from {channel} by {sender}; reason: {}",
                reason.unwrap_or_default()
            );
            tokio::spawn(async move {
                let _ = sendi.send(InviteMsg::Kicked(channel)).await;
            });
        }
        Command::PRIVMSG(channel, msg)
            if state.current_nick == channel && msg == "\x01VERSION\x01" =>
        {
            spawn_task!(
                sendo.send(Command::NOTICE(sender, format!("\x01VERSION {APP_NAME}\x01")).into())
            )
        }
        Command::PRIVMSG(channel, msg) => {
            if let Some(comm) = parse_moose_args(&msg) {
                capture_clone! {
                    (state.moose_url, state.moose_delay, state.moose_client)
                    spawn_task!(handle_priv_cmd(
                        comm,
                        channel,
                        sender,
                        disable_search,
                        moose_url,
                        moose_delay,
                        moose_client,
                        sendo,
                    ))
                };
            }
        }
        Command::Numeric(num, _params) => match num {
            Numeric::RPL_WELCOME => {
                if let Some(npass) = state.nickserv_pass.as_ref() {
                    capture_clone! { (sendo, npass)
                        tokio::spawn(async move {
                            sendo
                                .send(Command::Raw(format!("NICKSERV IDENTIFY {npass}")).into())
                                .await;
                        })
                    };
                }
                join_channels(&state.channels).for_each(move |m| sendo.lossy_send(m.into()));
            }
            Numeric::ERR_ERRONEUSNICKNAME => {
                eprintln!("ERR: [irc] Server does not like our nickname.");
                tokio::spawn(async move {
                    sendo.send(Command::QUIT(None).into()).await;
                });
            }
            Numeric::ERR_NICKNAMEINUSE | Numeric::ERR_NICKCOLLISION => {
                eprintln!("WARN: [irc] Server claims we have a name conflict.");
                let mut new_nick = state.current_nick.clone();
                new_nick.push_str(CONFLICT_FILLER);

                if new_nick.len() - state.original_nick.len() > 3 {
                    eprintln!("ERR: [irc] Server asked us to rename ourselves too many times.");
                    capture_clone! { (sendo)
                        tokio::spawn(async move { sendo.send(Command::QUIT(None).into()).await; })
                    };
                } else {
                    capture_clone! {
                        (new_nick)
                        spawn_task!(sendo.send(Command::NICK(new_nick).into()))
                    }
                    return Some(new_nick);
                }
            }
            _ => (),
        },
        _ => (),
    }
    None
}
