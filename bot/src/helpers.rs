use std::{collections::HashSet, mem};

use irc::proto::{Command, Message};

pub const CONFLICT_FILLER: &str = "_";

pub fn irc_preamble(nick: &str, pass: &str) -> Vec<Message> {
    let mut preamble: Vec<Message> = vec![
        Command::NICK(nick.to_owned()).into(),
        Command::USER(nick.to_owned(), "moose-irc2".to_owned()).into(),
    ];
    if !pass.is_empty() {
        preamble.push(Command::PASS(pass.to_owned()).into());
    }
    preamble
}

/// create a series of JOIN/PART commands to join a large number of channels in fewer commands.
fn join_part_channels(channels: &HashSet<String>) -> Vec<String> {
    let mut ret = vec![];
    if channels.is_empty() {
        return ret;
    }
    let mut cur = String::default();
    channels.iter().for_each(|channel| {
        // 512 - 2 (CRLF) - 5 (JOIN or PART + SPACE) = 505
        if channel.len() + cur.len() > 505 {
            ret.push(mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(',');
        }
        cur.push_str(channel);
    });
    ret.push(cur);
    ret
}

pub fn join_channels(channels: &HashSet<String>) -> impl Iterator<Item = Command> {
    join_part_channels(channels)
        .into_iter()
        .map(|s| Command::JOIN(s, None))
}

pub fn part_channels(channels: &HashSet<String>) -> impl Iterator<Item = Command> {
    join_part_channels(channels)
        .into_iter()
        .map(|s| Command::PART(s, None))
}

fn security<'a>(tls: bool) -> irc::connection::Security<'a> {
    if tls {
        irc::connection::Security::Secured {
            root_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
        }
    } else {
        irc::connection::Security::Unsecured
    }
}

pub fn client_config(server: &str, port: u16, tls: bool) -> irc::connection::Config {
    irc::connection::Config {
        server,
        port,
        security: security(tls),
    }
}

// #[derive(PartialEq)]
// pub enum CaseMapping {
//     Ascii,
//     Rfc1459,
//     Unicode, // Too much work?
// }

// /// Uppercases a slice and returns a copy.
// /// Note that this function currently only supports CASEMAPPING=ascii or CASEMAPPING=rfc1459
// pub fn irc_uppercase(casemap: &CaseMapping, the_str: &[u8]) -> Vec<u8> {
//     the_str
//         .iter()
//         .map(|&chr| match chr {
//             b'a'..=b'z' => chr - 32u8,
//             b'{'..=b'}' if *casemap == CaseMapping::Rfc1459 => chr - 32u8,
//             b'^' if *casemap == CaseMapping::Rfc1459 => chr + 32,
//             _ => chr,
//         })
//         .collect::<Vec<u8>>()
// }

// /// compare two byte sequences using the given irc casemapping rules.
// pub fn case_cmp(casemap: &CaseMapping, lhs: &[u8], rhs: &[u8]) -> bool {
//     irc_uppercase(casemap, lhs) == irc_uppercase(casemap, rhs)
// }

pub fn is_me(ours: &str, rename_cnt: u8, target: &str) -> bool {
    if rename_cnt == 0 {
        ours == target
    } else if target.len() == ours.len() + rename_cnt as usize {
        &target[..ours.len()] == ours
            && target[ours.len()..] == CONFLICT_FILLER.repeat(rename_cnt as usize)
    } else {
        false
    }
}

#[macro_export]
macro_rules! enclose {
    ( ($( $x:ident ),*) $y:expr ) => {
        {
            $(let $x = $x.clone();)*
            $y
        }
    };
}
