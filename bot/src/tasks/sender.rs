use std::time::Duration;

use futures::{
    SinkExt, StreamExt,
    stream::{PollNext, SelectWithStrategy, SplitSink, select_with_strategy},
};
use irc::{Codec, Connection, proto::Message};
use leaky_bucket::RateLimiter;
use tokio::{
    sync::mpsc::{self},
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;
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

type StratFnType = for<'a> fn(&'a mut ()) -> PollNext;
pub type Receiver =
    SelectWithStrategy<ReceiverStream<Message>, ReceiverStream<Message>, StratFnType, ()>;

fn prio_left(_: &mut ()) -> PollNext {
    PollNext::Left
}

pub fn create_send_recv_pair() -> (Sender, Receiver) {
    let (msg, msg_r) = mpsc::channel(64);
    let (moose, moose_r) = mpsc::channel(64);
    let recv = select_with_strategy(
        ReceiverStream::new(msg_r),
        ReceiverStream::new(moose_r),
        prio_left as StratFnType, // left = higher priority
    );
    (Sender { msg, moose }, recv)
}

pub fn sender_task(
    send_burst: usize,
    send_delay: Duration,
    mut send: SplitSink<Connection<Codec>, Message>,
    mut recv: Receiver,
    stop_token: CancellationToken,
) -> JoinHandle<()> {
    let send_burst = if send_burst == 0 { 1 } else { send_burst };
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
    tokio::task::spawn(async move {
        let _dropg = stop_token.drop_guard_ref();
        while let Some(msg) = tokio::select! {
            m = recv.next() => m,
            _ = stop_token.cancelled() => None,
        } {
            if let Some(i) = interval.as_ref() {
                i.acquire_one().await;
            }
            if let Err(e) = send.send(msg).await {
                eprintln!("ERR: [task/sender] IO error: {e}");
                break;
            };
        }
        eprintln!("INFO: [task/sender] Shutting down.");
    })
}
