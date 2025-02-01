use std::sync::{
    atomic::{AtomicU8, Ordering::Relaxed},
    Arc,
};

use irc::proto::{command::Numeric, Command, Message, Source, User};
use tokio::sync::mpsc::{
    // error::{SendError, TrySendError},
    Sender,
};

use crate::{
    config::Config,
    helpers::{is_me, join_channels, CONFLICT_FILLER},
    tasks::{invite::InviteMsg, sender::SendMsg},
};

// #[derive(Debug, thiserror::Error)]
// enum TaskError {
//     #[error("Failed to send to sender task: shut down.")]
//     Send(#[from] SendError<SendMsg>),
//     #[error("Invite Overloaded or task is shut down")]
//     ISend(#[from] TrySendError<InviteMsg>),
//     #[error("Fatal Protocol Error")]
//     Fatal,
// }

pub async fn handle(
    config: Arc<Config>,
    msg: Message,
    sendi: Sender<InviteMsg>,
    sendo: Sender<SendMsg>,
    rename_cnt: &AtomicU8,
) {
    let sender = match msg.source {
        Some(Source::Server(server)) => server,
        Some(Source::User(User { nickname, .. })) => nickname,
        None => "".to_owned(),
    };
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
        Command::JOIN(channel, _) if is_me(&config.nick, rename_cnt.load(Relaxed), &sender) => {
            eprintln!("INFO: [irc] Joined {channel}");
            Ok(())
        }
        // shouldn't happen?
        Command::PART(channel, _) if is_me(&config.nick, rename_cnt.load(Relaxed), &sender) => {
            eprintln!("INFO: [irc] Parted {channel}");
            let _ = sendi.try_send(InviteMsg::Kicked(channel));
            Ok(())
        }
        Command::INVITE(target, channel)
            if is_me(&config.nick, rename_cnt.load(Relaxed), &target) =>
        {
            let _ = sendi.try_send(InviteMsg::Joined(channel.clone()));
            sendo
                .send(SendMsg::Delayed(Command::JOIN(channel, None).into()))
                .await
        }
        Command::KICK(channel, target, reason)
            if is_me(&config.nick, rename_cnt.load(Relaxed), &target) =>
        {
            eprintln!(
                "INFO: [irc] Kicked from {channel} by {sender}; reason: {}",
                reason.unwrap_or_default()
            );
            let _ = sendi.try_send(InviteMsg::Kicked(channel));
            Ok(())
        }
        Command::PRIVMSG(channel, msg)
            if is_me(&config.nick, rename_cnt.load(Relaxed), &channel)
                && msg == "\x01VERSION\x01" =>
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
            eprintln!("DEBUG: [irc] {channel} :{msg}");
            Ok(())
        }
        Command::NOTICE(channel, msg) => {
            eprintln!("DEBUG: [irc] NOTICE: {channel} :{msg}");
            Ok(())
        }
        Command::NICK(nick) => {
            eprintln!("WARN: [irc] Server has changed our nickname to {nick}");
            Ok(())
        }
        Command::Numeric(num, _params) => match num {
            Numeric::RPL_WELCOME => {
                let mut joins = join_channels(&config.channels);
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
                eprintln!("ERR: [irc] Serve does not like our nickname.");
                sendo
                    .send(SendMsg::Immediate(Command::QUIT(None).into()))
                    .await
            }
            Numeric::ERR_NICKNAMEINUSE | Numeric::ERR_NICKCOLLISION => {
                eprintln!(
                    "WARN: [irc] Serve says our name is in use; asking for same name with a suffix."
                );
                let rcnt = match rename_cnt.fetch_update(Relaxed, Relaxed, |cnt| {
                    if cnt > 3 {
                        None
                    } else {
                        Some(cnt + 1)
                    }
                }) {
                    Ok(c) => c as usize,
                    Err(_) => {
                        eprintln!("ERR: [irc] Server asked us to rename ourselves too many times.");
                        let _ = sendo
                            .send(SendMsg::Immediate(Command::QUIT(None).into()))
                            .await;
                        return;
                    }
                };
                let mut new_nick = config.nick.clone();
                new_nick.push_str(CONFLICT_FILLER.repeat(rcnt).as_str());
                sendo
                    .send(SendMsg::Immediate(Command::NICK(new_nick).into()))
                    .await
            }
            _ => Ok(()),
        },
        _ => Ok(()),
    };
}
