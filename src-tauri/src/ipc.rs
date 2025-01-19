use futures::{SinkExt, TryStreamExt};
use interprocess::local_socket::tokio::prelude::LocalSocketStream;
use interprocess::local_socket::tokio::{RecvHalf, SendHalf};
use interprocess::local_socket::traits::tokio::Stream;
use serde::{Deserialize, Serialize};
use std::io::Error;
use tokio_serde::formats::SymmetricalJson;
use tokio_serde::SymmetricallyFramed;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

/// This is brought in from the goxlr-ipc crate, we ultimately don't care about the IPC format
/// for requests / responses, and simply want to handle serde_json's 'Value' type, so it might
/// be useful to fix this so that the ipc inherits are optional. Until then, we'll simply copypasta.
#[derive(Debug)]
pub struct Socket<In, Out> {
    reader:
        SymmetricallyFramed<FramedRead<RecvHalf, LengthDelimitedCodec>, In, SymmetricalJson<In>>,
    writer:
        SymmetricallyFramed<FramedWrite<SendHalf, LengthDelimitedCodec>, Out, SymmetricalJson<Out>>,
}

impl<In, Out> Socket<In, Out>
where
    for<'a> In: Deserialize<'a> + Unpin,
    Out: Serialize + Unpin,
{
    // This is basically identical to the existing one, except we take an interprocess LocalSocketStream instead..
    pub fn new(stream: LocalSocketStream) -> Self {
        let (stream_read, stream_write) = stream.split();
        let length_delimited_read = FramedRead::new(stream_read, LengthDelimitedCodec::new());
        let reader = SymmetricallyFramed::new(length_delimited_read, SymmetricalJson::default());

        let length_delimited_write = FramedWrite::new(stream_write, LengthDelimitedCodec::new());
        let writer = SymmetricallyFramed::new(length_delimited_write, SymmetricalJson::default());

        Self { reader, writer }
    }

    pub async fn try_read(&mut self) -> Result<Option<In>, Error> {
        self.reader.try_next().await
    }

    pub async fn send(&mut self, out: Out) -> Result<(), Error> {
        self.writer.send(out).await
    }
}
