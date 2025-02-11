use std::time::Duration;

use futures::{stream::SplitSink, SinkExt};
use irc::{proto::Message, Codec, Connection};
use tokio::{sync::mpsc, task::JoinHandle};

pub enum SendMsg {
    Immediate(Message),
    Delayed(Message),
}

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
    tokio::task::spawn(async move {
        let mut recv_shut = send_shut.subscribe();
        let mut interval = if send_delay.is_zero() {
            None
        } else {
            let mut i = tokio::time::interval(send_delay);
            i.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            Some(i)
        };
        while let Some(msg) = tokio::select! {
            m = high_prio.recv() => m.map(SendMsg::Immediate),
            m = notify.recv() => m.map(SendMsg::Immediate),
            m = low_prio.recv() => m.map(SendMsg::Delayed),
            _ = recv_shut.recv() => None,
        } {
            let m = match (msg, interval.as_mut()) {
                (SendMsg::Immediate(m), _) => m,
                (SendMsg::Delayed(m), None) => m,
                (SendMsg::Delayed(m), Some(i)) => {
                    i.tick().await;
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
