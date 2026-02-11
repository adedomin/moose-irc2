use tokio::{
    signal::unix::{SignalKind, signal},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use super::invite::InviteMsg;

pub fn shutdown_task(
    stop_token: CancellationToken,
    send_invite: mpsc::Sender<InviteMsg>,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let _dropg = stop_token.drop_guard_ref();
        let mut sigterm = signal(SignalKind::terminate()).unwrap();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                eprintln!("WARN: [task/shutdown] SIGINT: Shutting down.");
            }
            _ = sigterm.recv() => {
                eprintln!("WARN: [task/shutdown] SIGTERM: Shutting down.");
            }
            _ = stop_token.cancelled() => {
                eprintln!("WARN: [task/shutdown] SHUTDOWN: Shutting down.");
            }
        }
        // Sometimes the invite task will permanently block on blocking_recv().
        // This will make sure the invite task gets an explicit quit message so it shuts down.
        let _ = send_invite.send(InviteMsg::Quit).await;
    })
}
