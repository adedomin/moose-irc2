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

use std::{collections::HashSet, path::PathBuf};

use config::parse_args;
use futures::StreamExt;
use helpers::client_config;
use tasks::{
    invite::invite_task,
    receiver::receiver_task,
    sender::{create_send_recv_pair, sender_task},
    shutdown::shutdown_task,
};
use tokio::sync::{broadcast, mpsc};

mod config;
mod handlers;
mod helpers;
mod tasks;
mod webreq;

fn default_port(tls: bool) -> u16 {
    if tls {
        6697
    } else {
        6667
    }
}

fn main() {
    let (config, invites) = parse_args();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("To start async runtime.");
    rt.block_on(async {
        let (sendi, recvi) = mpsc::channel(64);
        let i: Option<(HashSet<String>, PathBuf)> = match (invites, config.invite_file.clone()) {
            (Some(i), Some(ifil)) => Some((i, ifil)),
            _ => None,
        };
        let inviter = invite_task(i, recvi);

        let (sendshut, recvshut) = broadcast::channel::<()>(16);
        let shutdown = shutdown_task(sendshut.clone(), sendi.clone());

        let (server, port) = config
            .host
            .split_once(':')
            .map(|(s, p)| (s, p.parse::<u16>().unwrap_or(default_port(config.tls))))
            .unwrap_or_else(|| (&config.host, default_port(config.tls)));
        let (sendm, recvm) = irc::connection::Connection::new(
            client_config(server, port, config.tls),
            irc::Codec {},
        )
        .await
        .expect("Expected to set up connection.")
        .split();
        let (sendo, recvo) = create_send_recv_pair();
        let sender = sender_task(
            config.send_burst,
            config.send_delay,
            sendm,
            recvo,
            sendshut.clone(),
        );

        let receiver = receiver_task(config, recvm, sendo, sendi, sendshut, recvshut);
        let _ = tokio::join!(sender, receiver, shutdown);
        let _ = inviter.join();
    });
}
