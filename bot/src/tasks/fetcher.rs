use std::{
    sync::mpsc::{Receiver, Sender},
    thread::{self, JoinHandle},
};

pub fn moose_fetcher(sendo: Sender<Vec<u8>>, recvm: Receiver<Vec<u8>>) -> JoinHandle<()> {
    thread::spawn(move || {
        let agent: Agent = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();
        let mut peek = recvm.into_iter().peekable();
        while let Some(moose_name) = peek.peek() {
            agent.get()
            peek.next().unwrap();
        }
    })
}
