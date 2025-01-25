use std::time::Duration;

use futures::{stream::SplitSink, SinkExt};
use irc::{proto::Message, Codec, Connection};
use tokio::task::JoinHandle;

pub enum SendMsg {
    Immediate(Message),
    Delayed(Message),
}

pub fn sender_task(
    send_delay: Duration,
    mut send: SplitSink<Connection<Codec>, Message>,
    mut recv: tokio::sync::mpsc::Receiver<SendMsg>,
    send_shut: tokio::sync::broadcast::Sender<()>,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let mut recv_shut = send_shut.subscribe();
        println!("{send_delay:?}");
        let mut interval = tokio::time::interval(send_delay);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        while let Some(msg) = tokio::select! {
            m = recv.recv() => m,
            _ = recv_shut.recv() => None,
        } {
            let m = match msg {
                SendMsg::Immediate(m) => m,
                SendMsg::Delayed(m) => {
                    interval.tick().await;
                    m
                }
            };
            if let Err(e) = send.send(m).await {
                eprintln!("ERR: [task/sender] IO error: {e}");
                break;
            };
        }
        let _ = send_shut.send(());
        eprintln!("INFO: [task/sender] Shutting down.");
    })
}
