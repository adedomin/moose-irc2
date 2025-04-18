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
        let m = value.1.trim().to_owned();
        match value.0 {
            PComm::Search => Self::Search(m),
            PComm::Image => Self::Image(m),
            PComm::Irc => Self::Irc(m),
        }
    }
}

enum PComm {
    Search,
    Image,
    Irc,
}

pub fn parse_moose_args(msg: &str) -> Option<MComm> {
    // we need any whitespace.
    let mut iter = msg.split(|c: char| c.is_ascii_whitespace());
    let comm = match iter.next()? {
        ".moose" | "!moose" | "moose" | ".mooseme" | "!mooseme" | "mooseme" => PComm::Irc,
        ".mooseimg" | "!mooseimg" | "mooseimg" => PComm::Image,
        ".moosesearch" | "!moosesearch" | "moosesearch" => PComm::Search,
        ".bots" | "!bots" => return Some(MComm::Bots),
        ".help" | "!help" => return Some(MComm::Help),
        _ => return None,
    };
    let arg = iter.next().unwrap_or("random");
    let rest = iter.collect::<Vec<&str>>().join(" ");
    match arg {
        "" | "--" => Some((comm, rest.as_str()).into()),
        "-h" | "--help" => Some(MComm::Help),
        "-s" | "--search" => Some((PComm::Search, rest.as_str()).into()),
        "-i" | "--image" => Some((PComm::Image, rest.as_str()).into()),
        "-r" | "--random" => Some((comm, "random").into()),
        "-l" | "--latest" => Some((comm, "latest").into()),
        "-o" | "--oldest" => Some((comm, "oldest").into()),
        _ => Some((comm, [arg, &rest].join(" ").as_str()).into()),
    }
}
