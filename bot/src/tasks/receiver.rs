use std::sync::{atomic::AtomicU8, Arc};

use futures::{stream::SplitStream, StreamExt};
use irc::{Codec, Connection};
use tokio::{
    sync::{mpsc::Sender, Semaphore},
    task::JoinHandle,
};

use crate::{capture_clone, config::Config, handler, helpers::irc_preamble};

use super::{invite::InviteMsg, sender::SendMsg};

static RENAME_COUNTER: AtomicU8 = AtomicU8::new(0);

pub fn receiver_task(
    config: Config,
    mut recv: SplitStream<Connection<Codec>>,
    sendo: Sender<SendMsg>,
    sendi: Sender<InviteMsg>,
    send_shut: tokio::sync::broadcast::Sender<()>,
    mut recv_shut: tokio::sync::broadcast::Receiver<()>,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let task_limit = Semaphore::new(64);
        let pass = config.pass.clone().unwrap_or_default();
        let pream = irc_preamble(config.nick.as_str(), pass.as_str());

        let arc_config = Arc::new(config);

        pream.into_iter().for_each(|m| {
            sendo
                .try_send(SendMsg::Immediate(m))
                .expect("to send preamble.");
        });

        while let Some(msg) = tokio::select! {
            m = recv.next() => m,
            _ = recv_shut.recv() => None,
        } {
            match msg {
                Ok(Ok(msg)) => {
                    if task_limit.try_acquire().is_ok() {
                        tokio::spawn(
                            capture_clone! { (sendi, sendo, arc_config) async move { handler::handle(arc_config, msg, sendi, sendo, &RENAME_COUNTER).await } },
                        );
                    } else {
                        eprintln!("WARN: [task/receiver] Too many messages! Dropping some events.");
                    }
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
