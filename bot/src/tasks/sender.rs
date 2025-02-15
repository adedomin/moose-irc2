use std::time::Duration;

use futures::{stream::SplitSink, SinkExt};
use irc::{proto::Message, Codec, Connection};
use leaky_bucket::RateLimiter;
use tokio::{sync::mpsc, task::JoinHandle};

#[derive(Clone)]
pub struct Sender {
    high_prio: mpsc::Sender<Message>,
    low_prio: mpsc::Sender<Message>,
    notify: mpsc::Sender<Message>,
}

impl Sender {
    pub async fn send(&self, m: Message) {
        let _ = self.low_prio.send(m).await;
    }

    pub fn lossy_send(&self, m: Message) {
        let _ = self.low_prio.try_send(m);
    }

    pub async fn send_high_prio(&self, m: Message) {
        let _ = self.high_prio.send(m).await;
    }

    pub fn lossy_send_high_prio(&self, m: Message) {
        let _ = self.high_prio.try_send(m);
    }

    pub fn send_notify(&self, m: Message) {
        let _ = self.notify.try_send(m);
    }
}

pub struct Receiver {
    high_prio: mpsc::Receiver<Message>,
    low_prio: mpsc::Receiver<Message>,
    notify: mpsc::Receiver<Message>,
}

pub fn create_send_recv_pair() -> (Sender, Receiver) {
    let (high_prio, high_prior) = mpsc::channel(64);
    let (low_prio, low_prior) = mpsc::channel(64);
    let (notify, notifyr) = mpsc::channel(1);
    (
        Sender {
            high_prio,
            low_prio,
            notify,
        },
        Receiver {
            high_prio: high_prior,
            low_prio: low_prior,
            notify: notifyr,
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
    let Receiver {
        mut high_prio,
        mut low_prio,
        mut notify,
    } = recv;
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
        while let Some(msg) = tokio::select! {
            m = high_prio.recv() => m,
            m = notify.recv() => m,
            m = low_prio.recv() => m,
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
