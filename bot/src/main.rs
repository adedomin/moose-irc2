/* Copyright (C) 2025  Anthony DeDominic
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::mpsc,
    thread,
    time::Instant,
};

use config::{parse_args, save_invite};
use helpers::irc_preamble;
use irc::{
    iter::{BufIterator, TruncStatus},
    parse::Message,
};

mod config;
mod helpers;
mod tasks;
mod tls;

enum Invite {
    Joined(String),
    Kicked(String),
}

// macro_rules! enclose {
//     ( ($( $x:ident ),*) $y:expr ) => {
//         {
//             $(let $x = $x.clone();)*
//             $y
//         }
//     };
// }

fn main() {
    let (config, invites) = parse_args();

    let mut tcpr = TcpStream::connect(config.host.as_str()).expect("Could not connect to server");

    let (sendi, recvi) = mpsc::channel();
    let inviter = thread::spawn(move || {
        if let Some(mut invites) = invites {
            let ifile = config.invite_file.unwrap();
            while let Ok(invite) = recvi.recv() {
                let changed = match invite {
                    Invite::Joined(chan) => invites.insert(chan),
                    Invite::Kicked(chan) => invites.remove(&chan),
                };
                if changed {
                    if let Err(e) = save_invite(&ifile, &invites) {
                        eprintln!("WARN: Failed to save invite changes: {e}");
                    }
                }
            }
        }
    });

    let (sendo, recvo) = mpsc::channel::<Vec<u8>>();
    let mut tcps = tcpr.try_clone().expect("expected to make clonable sender");
    let sender = thread::spawn(move || {
        let mut last = Instant::now();
        while let Ok(line) = recvo.recv() {
            let delay = config.send_delay.saturating_sub(last.elapsed());
            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
            if let Err(e) = tcps.write_all(&line) {
                eprintln!("ERR: {e}");
                break;
            };
            last = Instant::now();
        }
    });

    let (sendm, recvm) = mpsc::sync_channel::<Vec<u8>>(1);
    let sendo2 = sendo.clone();
    let moose_fetcher = thread::spawn(move || {
        let mut peek = recvm.into_iter().peekable();
        while let Some(moose_name) = peek.peek() {
            peek.next().unwrap();
        }
    });

    let mut head = 0;
    let mut buf = [0u8; 2usize.pow(16)];
    sendo
        .send(irc_preamble(
            &config.nick,
            &config.pass.unwrap_or_else(|| "".to_owned()),
        ))
        .expect("should not be closed");
    'ev: loop {
        let len = match tcpr.read(&mut buf[head..]) {
            Ok(len) => len,
            Err(e) => {
                eprintln!("ERR: {e}");
                break 'ev;
            }
        };
        let mut off = 0;
        let mut offend = 0;
        for line in BufIterator::new(&buf[..len + head]) {
            match line {
                TruncStatus::Full(msg) => {
                    let mut msg = Message::new(msg);
                    if let Some(command) = msg.command {
                        match command {
                            b"PING" => {
                                msg.command = Some(b"PONG");
                                if let Err(e) = sendo.send(msg.to_server_vec()) {
                                    eprintln!("ERR: Sender shutdown: {e}.");
                                    break 'ev;
                                }
                            }
                            _ => (),
                        }
                    }
                }
                TruncStatus::Part(residue) => {
                    if residue.len() == buf.len() {
                        eprintln!("ERR: Server is sending junk.");
                        break 'ev;
                    }
                    off = (residue.as_ptr() as usize)
                        .checked_sub(buf.as_ptr() as usize)
                        .expect("residue likely did not come from our buffer!");
                    offend = off + residue.len();
                    head = residue.len();
                }
            }
        }
        buf.copy_within(off..offend, 0);
    }
}
