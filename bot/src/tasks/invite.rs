use std::{
    collections::HashSet,
    path::PathBuf,
    thread::{self, JoinHandle},
};

use tokio::sync::mpsc::Receiver;

use crate::config::save_invite;

pub enum InviteMsg {
    Joined(String),
    Kicked(String),
    Quit,
}

pub fn invite_task(
    invites: Option<(HashSet<String>, PathBuf)>,
    mut recv: Receiver<InviteMsg>,
) -> JoinHandle<()> {
    // tokio spawn_blocking is not intended for long (infinite) lived tasks.
    thread::spawn(move || {
        if let Some((mut invites, ifile)) = invites {
            while let Some(invite) = recv.blocking_recv() {
                let changed = match invite {
                    InviteMsg::Joined(chan) => invites.insert(chan),
                    InviteMsg::Kicked(chan) => invites.remove(&chan),
                    InviteMsg::Quit => break,
                };
                if changed {
                    if let Err(e) = save_invite(&ifile, &invites) {
                        eprintln!("WARN: Failed to save invite changes: {e}");
                    }
                }
            }
        }
        eprintln!("INFO: [tasks/invite] Shutting down.");
    })
}
