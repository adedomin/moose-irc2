use std::{num::NonZero, time::Duration};

use futures::{SinkExt, stream::SplitSink};
use governor::{Quota, RateLimiter};
use irc::{Codec, Connection, proto::Message};
use tokio::{
    sync::mpsc::{self},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Sender {
    msg: mpsc::Sender<Message>,
    moose: mpsc::Sender<Message>,
}

impl Sender {
    pub async fn send(&self, m: Message) {
        let _ = self.msg.send(m).await;
    }

    pub fn lossy_send(&self, m: Message) {
        let _ = self.msg.try_send(m);
    }

    pub fn send_moose(&self, m: Message) {
        let _ = self.moose.try_send(m);
    }
}

pub struct Receiver {
    msg_r: mpsc::Receiver<Message>,
    moose_r: mpsc::Receiver<Message>,
}

pub fn create_send_recv_pair() -> (Sender, Receiver) {
    let (msg, msg_r) = mpsc::channel(64);
    let (moose, moose_r) = mpsc::channel(64);
    (Sender { msg, moose }, Receiver { msg_r, moose_r })
}

pub fn sender_task(
    send_burst: Option<NonZero<u32>>,
    send_delay: Duration,
    mut send: SplitSink<Connection<Codec>, Message>,
    recv: Receiver,
    stop_token: CancellationToken,
) -> JoinHandle<()> {
    let interval = if send_delay.is_zero() {
        None
    } else {
        let send_burst = send_burst.unwrap_or_else(|| NonZero::<u32>::new(1).unwrap());
        let rl = RateLimiter::direct(
            Quota::with_period(send_delay)
                .unwrap()
                .allow_burst(send_burst),
        );
        Some(rl)
    };
    let Receiver {
        mut msg_r,
        mut moose_r,
    } = recv;
    tokio::task::spawn(async move {
        let _dropg = stop_token.drop_guard_ref();
        while let Some(msg) = tokio::select! {
            biased;
            m = msg_r.recv() => m,
            m = moose_r.recv() => m,
            _ = stop_token.cancelled() => None,
        } {
            if let Some(i) = interval.as_ref() {
                i.until_ready().await;
            }
            if let Err(e) = send.send(msg).await {
                eprintln!("ERR: [task/sender] IO error: {e}");
                break;
            };
        }
        eprintln!("INFO: [task/sender] Shutting down.");
    })
}
