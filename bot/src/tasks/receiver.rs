use std::{sync::Arc, time::Duration};

use futures::{stream::SplitStream, StreamExt};
use irc::{proto::Command, Codec, Connection};
use tokio::{
    sync::{mpsc::Sender, RwLock, Semaphore},
    task::JoinHandle,
    time,
};

use crate::{
    capture_clone,
    config::Config,
    handler::{self, IrcState},
    helpers::irc_preamble,
};

use super::{invite::InviteMsg, sender};

pub fn receiver_task(
    config: Config,
    mut recv: SplitStream<Connection<Codec>>,
    sendo: sender::Sender,
    sendi: Sender<InviteMsg>,
    send_shut: tokio::sync::broadcast::Sender<()>,
    mut recv_shut: tokio::sync::broadcast::Receiver<()>,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let pass = config.pass.clone().unwrap_or_default();
        let pream = irc_preamble(config.nick.as_str(), pass.as_str());
        pream
            .into_iter()
            .for_each(|m| sendo.lossy_send_high_prio(m));

        let irc_state = Arc::new(RwLock::new(IrcState::new(
            config.nick,
            config.nickserv,
            config.channels,
            config.moose_url,
            config.moose_delay,
        )));
        let task_limit = Arc::new(Semaphore::new(64));
        let mut double_timeout = false;
        'l: while let Some(msg) = tokio::select! {
                m = recv.next() => m,
                _ = recv_shut.recv() => None,
                _ = time::sleep(Duration::from_secs(60)) => {
                    if double_timeout {
                        eprintln!("ERR: [task/receiver] TCP Connection is likely half open or the IRC server is broken.");
                        None
                    } else {
                        double_timeout = true;
                        sendo.send(Command::PING("PING".to_owned()).into()).await;
                        continue 'l;
                    }
                },
        } {
            double_timeout = false;
            match msg {
                Ok(Ok(msg)) => {
                    tokio::spawn(capture_clone! {
                        (irc_state, sendo, sendi, task_limit)
                        async move {
                            if task_limit.try_acquire().is_ok() {
                                handler::handle(irc_state, msg, config.disable_search, sendo, sendi).await;
                            } else {
                                eprintln!("WARN: [irc] Too many tasks; dropping messages.");
                            }
                        }
                    });
                }
                Ok(Err(e)) => {
                    eprintln!("WARN: [task/receiver] Invalid IRC line: {e}");
                }
                Err(e) => {
                    eprintln!("ERR: [tast/receiver] Stream error: {e}");
                    break;
                }
            }
        }
        let _ = send_shut.send(());
        eprintln!("INFO: [task/receiver] Shutting down.")
    })
}
