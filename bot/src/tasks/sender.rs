use std::time::Duration;

use futures::{stream::SplitSink, SinkExt};
use irc::{proto::Message, Codec, Connection};
use leaky_bucket::RateLimiter;
use tokio::{sync::mpsc, task::JoinHandle};

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
    msg: mpsc::Receiver<Message>,
    moose: mpsc::Receiver<Message>,
}

pub fn create_send_recv_pair() -> (Sender, Receiver) {
    let (msg, msg_r) = mpsc::channel(64);
    let (moose, moose_r) = mpsc::channel(64);
    (
        Sender { msg, moose },
        Receiver {
            msg: msg_r,
            moose: moose_r,
        },
    )
}

pub fn sender_task(
    send_burst: usize,
    send_delay: Duration,
    mut send: SplitSink<Connection<Codec>, Message>,
    recv: Receiver,
    send_shut: tokio::sync::broadcast::Sender<()>,
) -> JoinHandle<()> {
    let send_burst = if send_burst == 0 { 1 } else { send_burst };
    tokio::task::spawn(async move {
        let mut recv_shut = send_shut.subscribe();
        let interval = if send_delay.is_zero() {
            None
        } else {
            let rl = RateLimiter::builder()
                .fair(false)
                .max(send_burst)
                .initial(send_burst)
                .interval(send_delay)
                .refill(1)
                .build();
            Some(rl)
        };
        let Receiver { mut msg, mut moose } = recv;
        while let Some(msg) = tokio::select! {
            m = msg.recv() => m,
            m = moose.recv() => m,
            _ = recv_shut.recv() => None,
        } {
            if let Some(i) = interval.as_ref() {
                i.acquire_one().await;
            }
            if let Err(e) = send.send(msg).await {
                eprintln!("ERR: [task/sender] IO error: {e}");
                break;
            };
        }
        let _ = send_shut.send(());
        eprintln!("INFO: [task/sender] Shutting down.");
    })
}
