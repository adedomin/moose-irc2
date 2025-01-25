use std::io;

use proto::{format, parse, Message};
use tokio_util::bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

pub type ParseResult<T = Message, E = parse::Error> = std::result::Result<T, E>;

pub struct Codec;

const BUF_MAX: usize = 2usize.pow(16);
const LINE_END: [u8; 2] = [b'\r', b'\n'];

fn find_eom(buf: &[u8]) -> Option<usize> {
    buf.iter().position(|chr| LINE_END.contains(chr))
}

fn find_start(buf: &[u8]) -> Option<usize> {
    buf.iter().position(|chr| !LINE_END.contains(chr))
}

impl Decoder for Codec {
    type Item = ParseResult;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // remove any leading line endings.
        let Some(start) = find_start(src) else {
            if src.len() > BUF_MAX {
                return Err(Error::LineTooLong);
            } else {
                return Ok(None);
            }
        };
        src.advance(start);

        // get message.
        let Some(eom) = find_eom(src) else {
            if src.len() > BUF_MAX {
                return Err(Error::LineTooLong);
            } else {
                return Ok(None);
            }
        };
        let mut bytes = Vec::from(src.split_to(eom));
        bytes.extend(LINE_END);

        // remove any trailing line endings.
        match find_start(src) {
            Some(trailing) => src.advance(trailing),
            None => src.clear(),
        };

        Ok(Some(parse::message_bytes(bytes)))
    }
}

impl Encoder<Message> for Codec {
    type Error = Error;

    fn encode(&mut self, message: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let encoded = format::message(message);

        dst.extend(encoded.into_bytes());

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("irc server is sending a very long line of junk.")]
    LineTooLong,
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use futures::{SinkExt, StreamExt};
    use proto::Command;
    use tokio_util::codec::{FramedRead, FramedWrite};

    use super::Codec;

    #[tokio::test]
    async fn test_decode() {
        let message = Cursor::new(
            b"\r\n\r\n:test!test@test.example.com PRIVMSG you :Hello, World!\r\nPING :xyz\r\nPONG\n\n",
        );
        let decoder = Codec {};
        let mut reader = FramedRead::new(message, decoder);
        let mut full_cnt = 0;
        while let Some(frame) = reader.next().await {
            let _frame = frame.expect("no codec errors").expect("parsed message.");
            println!("{_frame:?}");
            full_cnt += 1;
        }
        assert!(
            full_cnt == 3,
            "there should only be three full irc lines to parse."
        );
    }

    #[tokio::test]
    async fn test_encode() {
        let mut buf = Cursor::new(vec![]);
        {
            let encoder = Codec {};

            let mut writer = FramedWrite::new(&mut buf, encoder);
            let hello_world =
                Command::PRIVMSG("#test".to_owned(), "Hello, world!".to_owned()).into();
            writer.send(hello_world).await.unwrap();
            writer
                .send(Command::QUIT(Some("Bye, world.".to_owned())).into())
                .await
                .unwrap();
        }
        let msg = buf.into_inner();
        assert_eq!(
            msg,
            b"PRIVMSG #test :Hello, world!\r\nQUIT :Bye, world.\r\n"
        );
    }
}
