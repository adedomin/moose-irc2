use std::collections::HashSet;

pub fn irc_preamble(nick: &str, pass: &str) -> Vec<u8> {
    let mut pre = format!(
        "NICK {0}\r
USER {1} +i * {0}\r
",
        nick, "moose-irc2"
    )
    .as_bytes()
    .to_vec();
    if !pass.is_empty() {
        pre.extend(b"PASS ");
        pre.extend(pass.as_bytes());
        pre.extend(b"\r\n");
    }
    pre
}

fn join_part_channels(command: &[u8], channels: &HashSet<String>) -> Vec<u8> {
    let mut ret = vec![];
    let mut lsize = ret.len();
    let mut first = true;

    for channel in channels {
        if channel.len() + lsize >= 510 {
            lsize = 0usize;
            first = true;
            ret.extend(b"\r\n");
        }

        if !first {
            ret.push(b',');
        } else {
            ret.extend(command);
            ret.push(b' ');
            lsize = command.len();
            first = false;
        }
        ret.extend(channel.as_bytes());
        lsize += channel.len() + 1;
    }
    ret.extend(b"\r\n");

    ret
}

pub fn join_channels(channels: &HashSet<String>) -> Vec<u8> {
    join_part_channels(b"JOIN", channels)
}

pub fn part_channels(channels: &HashSet<String>) -> Vec<u8> {
    join_part_channels(b"PART", channels)
}
