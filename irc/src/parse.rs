// Copyright (C) 2021  Anthony DeDominic <adedomin@gmail.com>

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

#[derive(PartialEq)]
enum ParseState {
    Prefix,
    Command,
    Params,
}

/// A non-general purpose IRCv2 parsed message.
/// This struct does not support tags as I do not use them or need them.
/// It also assumes the content is free of line delimiters.
/// This type was constructed to zero-copy view into a raw read buffer returned in parts
/// from crate::irc::iter::BufIterator.
#[derive(Default)]
pub struct Message<'a> {
    pub nick: Option<&'a [u8]>,
    pub user: Option<&'a [u8]>,
    pub host: Option<&'a [u8]>,
    pub command: Option<&'a [u8]>,
    pub params: Vec<&'a [u8]>,
}

fn parse_params(params: &[u8]) -> Vec<&[u8]> {
    let mut ret = vec![];
    let mut pos = 0;
    'out: while pos < params.len() {
        let mut i = pos;
        // find the start of the next argument
        let start = loop {
            if i < params.len() {
                if params[i] != b' ' {
                    break i;
                }
                i += 1;
            } else {
                break 'out;
            }
        };

        // if the next argument is a trailing
        // collect the rest of the string and return
        if params[start] == b':' {
            ret.push(&params[start + 1..]);
            break 'out;
        }

        // else try and find the next separator or end of string
        let end = loop {
            if i < params.len() {
                if params[i] == b' ' {
                    break i;
                }
                i += 1;
            } else {
                break params.len();
            }
        };

        if start < end {
            pos = end + 1;
            ret.push(&params[start..end]);
        } else {
            break;
        }
    }
    ret
}

fn parse_prefix(b: &[u8]) -> (Option<&[u8]>, Option<&[u8]>, Option<&[u8]>) {
    let user_start = b.iter().position(|&chr| chr == b'!');
    let host_start = b.iter().position(|&chr| chr == b'@');
    match (user_start, host_start) {
        (None, None) => (Some(b), None, None),
        (None, Some(host)) => (Some(&b[0..host]), None, Some(&b[host + 1..])),
        (Some(user), None) => (Some(&b[0..user]), Some(&b[user + 1..]), None),
        // the expected path
        (Some(user), Some(host)) if user < host => (
            Some(&b[0..user]),
            Some(&b[user + 1..host]),
            Some(&b[host + 1..]),
        ),
        // this shouldn't happen, but it's not exactly hard to support it.
        // basically instead of x!y@z we got x@z!y
        (Some(user), Some(host)) => (
            Some(&b[0..host]),
            Some(&b[user + 1..]),
            Some(&b[host + 1..user]),
        ),
    }
}

impl<'a> Message<'a> {
    pub fn is_empty(&self) -> bool {
        self.nick.is_none()
            && self.user.is_none()
            && self.host.is_none()
            && self.command.is_none()
            && self.params.is_empty()
    }

    pub fn new(raw: &'a [u8]) -> Self {
        let mut ret = Message::default();
        let mut arg_state = ParseState::Prefix;

        for part in raw.split(|&chr| chr == b' ') {
            if part.is_empty() {
                continue;
            }

            arg_state = match arg_state {
                ParseState::Prefix => {
                    let has_prefix = if let Some(chr) = part.first() {
                        *chr == b':'
                    } else {
                        false
                    };
                    if has_prefix {
                        let (nick, user, host) = parse_prefix(&part[1..]);
                        ret.nick = nick;
                        ret.user = user;
                        ret.host = host;
                        ParseState::Command
                    } else {
                        ret.command = Some(part);
                        ParseState::Params
                    }
                }
                ParseState::Command => {
                    ret.command = Some(part);
                    ParseState::Params
                }
                ParseState::Params => {
                    // calculate rest of buffer unconsumed.
                    let idx = part.as_ptr() as usize - raw.as_ptr() as usize;
                    ret.params = parse_params(&raw[idx..]);
                    break;
                }
            }
        }
        ret
    }

    /// format message to send back to a server.
    pub fn to_server_vec(&self) -> Vec<u8> {
        let mut ret = Vec::with_capacity(512);
        // I just realized this info shouldn't be sent.
        // match self.nick {
        //     Some(nick) => {
        //         ret.push(b':');
        //         ret.extend(nick);
        //         match (self.user, self.host) {
        //             (Some(user), Some(host)) => {
        //                 ret.push(b'!');
        //                 ret.extend(user);
        //                 ret.push(b'@');
        //                 ret.extend(host);
        //             }
        //             _ => (),
        //         };
        //         ret.push(b' ');
        //     }
        //     None => todo!(),
        // };
        let command = self.command.unwrap_or(b"PING");
        ret.extend(command);
        self.params.iter().enumerate().for_each(|(pos, &p)| {
            ret.push(b' ');
            if pos == self.params.len() - 1 {
                ret.push(b':');
            }
            ret.extend(p);
        });
        ret
    }
}

#[cfg(test)]
mod test {
    use super::Message;

    fn assert_all_of_the_parameters(
        m: Message,
        nick: Option<&[u8]>,
        user: Option<&[u8]>,
        host: Option<&[u8]>,
        command: Option<&[u8]>,
        params: Vec<&[u8]>,
    ) {
        assert_eq!(m.nick, nick);
        assert_eq!(m.user, user);
        assert_eq!(m.host, host);
        assert_eq!(m.command, command);
        assert_eq!(m.params, params)
    }

    #[test]
    fn test_irc_message_parse_full() {
        let m = Message::new(b":happy!test@case command 1 2 3 :trailing param.");
        assert_all_of_the_parameters(
            m,
            Some(b"happy"),
            Some(b"test"),
            Some(b"case"),
            Some(b"command"),
            vec![b"1", b"2", b"3", b"trailing param."],
        );
    }

    #[test]
    fn test_irc_message_parse_no_prefix() {
        let m = Message::new(b"command 1 2 3 :trailing param.");
        assert_all_of_the_parameters(
            m,
            None,
            None,
            None,
            Some(b"command"),
            vec![b"1", b"2", b"3", b"trailing param."],
        );
    }

    #[test]
    fn test_irc_message_parse_prefix_server() {
        let m = Message::new(b":some.irc.server command 1 2 3 :trailing param.");
        assert_all_of_the_parameters(
            m,
            Some(b"some.irc.server"),
            None,
            None,
            Some(b"command"),
            vec![b"1", b"2", b"3", b"trailing param."],
        );
    }

    #[test]
    fn test_irc_message_parse_prefix_user_host_swap() {
        let m = Message::new(b":happy@case!test command 1 2 3 :trailing param.");
        assert_all_of_the_parameters(
            m,
            Some(b"happy"),
            Some(b"test"),
            Some(b"case"),
            Some(b"command"),
            vec![b"1", b"2", b"3", b"trailing param."],
        );
    }

    #[test]
    fn test_irc_message_parse_prefix_blank() {
        let m = Message::new(b": com arg1 arg2");
        assert_all_of_the_parameters(
            m,
            Some(b""), // hard to say what the intended behavior would be, leave it as an "empty" sender.
            None,
            None,
            Some(b"com"),
            vec![b"arg1", b"arg2"],
        );
    }

    #[test]
    fn test_irc_message_parse_prefix_no_user() {
        let m = Message::new(b":x@y com arg1 arg2");
        assert_all_of_the_parameters(
            m,
            Some(b"x"),
            None,
            Some(b"y"),
            Some(b"com"),
            vec![b"arg1", b"arg2"],
        );
    }

    #[test]
    fn test_irc_message_parse_prefix_no_host() {
        let m = Message::new(b":x!y com arg1 arg2");
        assert_all_of_the_parameters(
            m,
            Some(b"x"),
            Some(b"y"),
            None,
            Some(b"com"),
            vec![b"arg1", b"arg2"],
        );
    }

    #[test]
    fn test_irc_message_parse_prefix_only() {
        let m = Message::new(b":x!y@z");
        assert_all_of_the_parameters(m, Some(b"x"), Some(b"y"), Some(b"z"), None, vec![]);
    }

    #[test]
    fn test_irc_message_parse_command_only() {
        let m = Message::new(b"PING");
        assert_all_of_the_parameters(m, None, None, None, Some(b"PING"), vec![]);
    }

    #[test]
    fn test_irc_message_parse_command_trailing_only() {
        let m = Message::new(b"PING : PONG");
        assert_all_of_the_parameters(m, None, None, None, Some(b"PING"), vec![b" PONG"]);
    }

    #[test]
    fn test_irc_message_parse_command_trailing_blank() {
        let m = Message::new(b"PING :");
        assert_all_of_the_parameters(m, None, None, None, Some(b"PING"), vec![b""]);
    }

    #[test]
    fn test_irc_message_parse_weird_spacing() {
        let m = Message::new(b":x     command    arg1  arg2        :     afdasfda  fdas   a .");
        assert_all_of_the_parameters(
            m,
            Some(b"x"),
            None,
            None,
            Some(b"command"),
            vec![b"arg1", b"arg2", b"     afdasfda  fdas   a ."],
        );
    }

    #[test]
    fn test_irc_message_parse_weird_spacing_no_trailer() {
        let m = Message::new(b":x     command    arg1  arg2             afdasfda  fdas   a .  ");
        assert_all_of_the_parameters(
            m,
            Some(b"x"),
            None,
            None,
            Some(b"command"),
            vec![b"arg1", b"arg2", b"afdasfda", b"fdas", b"a", b"."],
        );
    }

    #[test]
    fn test_irc_message_parse_weird_spacing_no_param() {
        let m = Message::new(b":x     command                 ");
        assert_all_of_the_parameters(m, Some(b"x"), None, None, Some(b"command"), vec![]);
    }

    #[test]
    fn test_irc_message_is_empty() {
        let t1 = Message::new(b"");
        assert!(t1.is_empty());
    }
}
