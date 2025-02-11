use tokio::{
    signal::unix::{signal, SignalKind},
    sync::broadcast::Sender,
    task::JoinHandle,
};

pub fn shutdown_task(send: Sender<()>) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let mut recv = send.subscribe();
        let mut sigterm = signal(SignalKind::terminate()).unwrap();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                eprintln!("WARN: [task/shutdown] SIGINT: Shutting down.");
                let _ = send.send(());
            }
            _ = sigterm.recv() => {
                eprintln!("WARN: [task/shutdown] SIGTERM: Shutting down.");
                let _ = send.send(());
            }
            _ = recv.recv() => {
                eprintln!("WARN: [task/shutdown] SHUTDOWN: Shutting down.");
            }
        }
    })
}
