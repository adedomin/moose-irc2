use std::sync::Arc;

use irc::proto::{command::Numeric, Command, Message, Source, User};
use tokio::sync::{mpsc::Sender, RwLock};

use crate::{
    debug,
    helpers::{join_channels, CONFLICT_FILLER},
    tasks::{invite::InviteMsg, sender},
    webreq::{get_irclines, get_search, resolve_moosename},
};

use super::{
    ircstate::{IrcState, APP_NAME},
    moosecmd::{parse_moose_args, MComm, HELP_RESP},
};

pub async fn handle(
    state: Arc<RwLock<IrcState>>,
    msg: Message,
    disable_search: bool,
    sendo: sender::Sender,
    sendi: Sender<InviteMsg>,
) {
    let sender = match msg.source {
        Some(Source::Server(server)) => server,
        Some(Source::User(User { nickname, .. })) => nickname,
        None => "".to_owned(),
    };
    let rstate = state.read().await;
    match msg.command {
        Command::PING(pong) => sendo.send(Command::PONG(pong, None).into()).await,
        Command::PONG(pong, _) => {
            debug!("DEBUG: [irc] recv PONG {pong}")
        }
        Command::ERROR(banned) => {
            eprintln!("ERR: [irc] Banned (?): {banned}");
            sendo.send(Command::QUIT(None).into()).await
        }
        Command::JOIN(channel, _) if rstate.current_nick == sender => {
            eprintln!("INFO: [irc] Joined {channel}");
        }
        // shouldn't happen?
        Command::PART(channel, _) if rstate.current_nick == sender => {
            eprintln!("INFO: [irc] Parted {channel}");
            let _ = sendi.send(InviteMsg::Kicked(channel)).await;
        }
        Command::INVITE(target, channel) if rstate.current_nick == target => {
            if sendi.try_send(InviteMsg::Joined(channel.clone())).is_ok() {
                sendo.send(Command::JOIN(channel, None).into()).await;
            } else {
                sendo
                    .send(Command::NOTICE(sender, "Invites are disabled.".to_owned()).into())
                    .await;
            }
        }
        Command::KICK(channel, target, reason) if rstate.current_nick == target => {
            eprintln!(
                "INFO: [irc] Kicked from {channel} by {sender}; reason: {}",
                reason.unwrap_or_default()
            );
            let _ = sendi.send(InviteMsg::Kicked(channel)).await;
        }
        Command::PRIVMSG(channel, msg)
            if rstate.current_nick == channel && msg == "\x01VERSION\x01" =>
        {
            sendo
                .send(Command::NOTICE(sender, format!("\x01VERSION {APP_NAME}\x01")).into())
                .await;
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
                    MComm::Search(q) => get_search(&rstate.moose_client, &rstate.moose_url, &q)
                        .await
                        .unwrap_or_else(|e| e.to_string()),
                    MComm::Image(q) => {
                        match resolve_moosename(&rstate.moose_client, &rstate.moose_url, &q).await {
                            Ok(moose) => format!("{}/img/{}", &rstate.moose_url, &moose),
                            Err(e) => e.to_string(),
                        }
                    }
                    MComm::Irc(q) => {
                        match resolve_moosename(&rstate.moose_client, &rstate.moose_url, &q).await {
                            Ok(moose) => {
                                // TODO: fix this crap.
                                if rstate.moose_delay.try_acquire(1) {
                                    match get_irclines(
                                        &rstate.moose_client,
                                        &rstate.moose_url,
                                        &moose,
                                    )
                                    .await
                                    {
                                        Ok(lines) => {
                                            lines.lines().for_each(|line| {
                                                sendo.send_moose(
                                                    Command::PRIVMSG(
                                                        channel.clone(),
                                                        line.to_owned(),
                                                    )
                                                    .into(),
                                                )
                                            });
                                            return;
                                        }
                                        Err(e) => e.to_string(),
                                    }
                                } else {
                                    sendo.lossy_send(
                                        Command::NOTICE(
                                            sender,
                                            "Please wait before asking for another moose."
                                                .to_owned(),
                                        )
                                        .into(),
                                    );
                                    return;
                                }
                            }
                            Err(e) => e.to_string(),
                        }
                    }
                };
                sendo.send(Command::PRIVMSG(channel, resp).into()).await;
            }
        }
        Command::Numeric(num, _params) => match num {
            Numeric::RPL_WELCOME => {
                if let Some(ref npass) = rstate.nickserv_pass {
                    sendo
                        .send(Command::Raw(format!("NICKSERV IDENTIFY {npass}")).into())
                        .await;
                }
                join_channels(&rstate.channels).for_each(|m| sendo.lossy_send(m.into()));
            }
            Numeric::ERR_ERRONEUSNICKNAME => {
                eprintln!("ERR: [irc] Server does not like our nickname.");
                sendo.send(Command::QUIT(None).into()).await;
            }
            Numeric::ERR_NICKNAMEINUSE | Numeric::ERR_NICKCOLLISION => {
                eprintln!("WARN: [irc] Server claims we have a name conflict.");
                drop(rstate);
                let mut wstate = state.write().await;
                wstate.current_nick.push_str(CONFLICT_FILLER);
                if wstate.current_nick.len() - wstate.original_nick.len() > 3 {
                    eprintln!("ERR: [irc] Server asked us to rename ourselves too many times.");
                    sendo.send(Command::QUIT(None).into()).await;
                    return;
                }
                sendo
                    .send(Command::NICK(wstate.current_nick.clone()).into())
                    .await;
            }
            _ => (),
        },
        _ => (),
    };
}
