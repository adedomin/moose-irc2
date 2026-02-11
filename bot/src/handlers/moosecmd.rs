use crate::debug;

pub const HELP_RESP: &str =
    "usage: ^[.!]?moose(?:img|search|me)? [--latest|--random|--search|--image|--] [moosename]";

pub enum MComm {
    Help,
    Bots,
    Search(String),
    Image(String),
    Irc(String),
}

impl<'a> From<(PComm, &'a str)> for MComm {
    fn from(value: (PComm, &'a str)) -> Self {
        debug!("DEBUG: CMD PARSED {value:?}");
        let m = value.1.to_owned();
        match value.0 {
            PComm::Search => Self::Search(m),
            PComm::Image => Self::Image(m),
            PComm::Irc => Self::Irc(m),
        }
    }
}

#[derive(Debug)]
enum PComm {
    Search,
    Image,
    Irc,
}

fn ws(c: char) -> bool {
    c.is_ascii_whitespace()
}

pub fn parse_moose_args(msg: &str) -> Option<MComm> {
    // we need any whitespace.
    let (comm, rest) = match msg.split_once(ws) {
        Some(cr) => cr,
        None => (msg, ""),
    };
    let comm = match comm {
        ".moose" | "!moose" | "moose" | ".mooseme" | "!mooseme" | "mooseme" => PComm::Irc,
        ".mooseimg" | "!mooseimg" | "mooseimg" => PComm::Image,
        ".moosesearch" | "!moosesearch" | "moosesearch" => PComm::Search,
        ".bots" | "!bots" => return Some(MComm::Bots),
        ".help" | "!help" => return Some(MComm::Help),
        _ => return None,
    };
    let rest = rest.trim();
    let (arg, r) = if rest.is_empty() {
        ("--random", "")
    } else {
        match rest.split_once(ws) {
            Some((a, r)) => (a, r.trim_start()),
            None => (rest, ""),
        }
    };

    match arg {
        "--" if !r.is_empty() => Some((comm, r).into()),
        "--" => Some((comm, rest).into()),
        "-h" | "--help" => Some(MComm::Help),
        "-s" | "--search" if !r.is_empty() => Some((PComm::Search, r).into()),
        "-s" | "--search" => Some(MComm::Help),
        "-i" | "--image" if !r.is_empty() => Some((PComm::Image, r).into()),
        "-i" | "--image" => Some((PComm::Image, "random").into()),
        "-r" | "--random" => Some((comm, "random").into()),
        "-l" | "--latest" => Some((comm, "latest").into()),
        "-o" | "--oldest" => Some((comm, "oldest").into()),
        _ => Some((comm, rest).into()),
    }
}
