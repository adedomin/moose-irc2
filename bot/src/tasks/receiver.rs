use futures::{stream::SplitStream, StreamExt, TryFutureExt};
use irc::{
    proto::{command::Numeric, Command, Source, User},
    Codec, Connection,
};
use tokio::{sync::mpsc::Sender, task::JoinHandle};

use crate::{
    config::Config,
    helpers::{irc_preamble, join_channels},
};

use super::{invite::InviteMsg, sender::SendMsg};

pub fn receiver_task(
    config: Config,
    mut recv: SplitStream<Connection<Codec>>,
    sendo: Sender<SendMsg>,
    sendi: Sender<InviteMsg>,
    send_shut: tokio::sync::broadcast::Sender<()>,
    mut recv_shut: tokio::sync::broadcast::Receiver<()>,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let pream = irc_preamble(
            config.nick.as_str(),
            config.pass.unwrap_or_default().as_str(),
        );
        let mut curr_nick = config.nick;
        pream.into_iter().for_each(|m| {
            sendo
                .try_send(SendMsg::Immediate(m))
                .expect("to send preamble.");
        });
        while let Some(msg) = tokio::select! {
            m = recv.next() => m,
            _ = recv_shut.recv() => None,
        } {
            if let Err(_) = match msg {
                Ok(Ok(m)) => {
                    let sender = match m.source {
                        Some(Source::Server(server)) => server,
                        Some(Source::User(User { nickname, .. })) => nickname,
                        None => "".to_owned(),
                    };
                    match m.command {
                        Command::PING(pong) => {
                            sendo
                                .send(SendMsg::Immediate(Command::PONG(pong, None).into()))
                                .await
                        }
                        Command::ERROR(banned) => {
                            eprintln!("ERR: [irc] Banned (?): {banned}");
                            let _ = sendo.try_send(SendMsg::Immediate(Command::QUIT(None).into()));
                            break;
                        }
                        Command::JOIN(channel, _) if sender == curr_nick => {
                            eprintln!("INFO: [irc] Joined {channel}");
                            Ok(())
                        }
                        // shouldn't happen?
                        Command::PART(channel, _) if sender == curr_nick => {
                            eprintln!("INFO: [irc] Parted {channel}");
                            let _ = sendi.try_send(InviteMsg::Kicked(channel));
                            Ok(())
                        }
                        Command::INVITE(target, channel) if target == curr_nick => {
                            let _ = sendi.try_send(InviteMsg::Joined(channel.clone()));
                            sendo
                                .send(SendMsg::Delayed(Command::JOIN(channel, None).into()))
                                .await
                        }
                        Command::KICK(channel, target, reason) if target == curr_nick => {
                            eprintln!(
                                "INFO: [irc] Kicked from {channel} by {sender}; reason: {}",
                                reason.unwrap_or_default()
                            );
                            let _ = sendi.try_send(InviteMsg::Kicked(channel));
                            Ok(())
                        }
                        Command::PRIVMSG(channel, msg)
                            if channel == curr_nick && msg == "\x01VERSION\x01" =>
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
                        Command::Numeric(num, _params) => match num {
                            Numeric::RPL_WELCOME => {
                                join_channels(&config.channels)
                                    .try_for_each(|joins| {
                                        sendo.try_send(SendMsg::Immediate(joins.into()))
                                    })
                                    .expect("expected to send all joins.");
                                Ok(())
                            }
                            _ => Ok(()),
                        },
                        _ => Ok(()),
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("WARN: [task/receiver] IRC Parser error, continuing...: {e}.");
                    Ok(())
                }
                Err(e) => {
                    eprintln!("ERR: [task/receiver] Framed Reader error: {e}.");
                    break;
                }
            } {
                eprintln!("ERR: [task/receiver] Sender closed.");
                break;
            }
        }
        let _ = send_shut.send(());
        eprintln!("INFO: [task/receiver] Shutting down.")
    })
}
