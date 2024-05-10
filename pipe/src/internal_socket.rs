use super::socket::*;

use flo_scene::*;
use flo_scene::programs::*;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, AsyncWriteExt};
use futures::prelude::*;
use futures::stream::{BoxStream, ReadyChunks};

use std::io;
use std::io::{Error};
use std::pin::{Pin};
use std::result::{Result};
use std::task::{Context, Poll};
use std::sync::*;

///
/// Requests that can be made to the internal socket program
///
pub enum InternalSocketMessage {
    ///
    /// Subscribes to connection requests for an internal socket program
    ///
    Subscribe,

    ///
    /// Creates an internal socket connection
    ///
    CreateInternalSocket(Box<dyn Send + AsyncRead>, Box<dyn Send + AsyncWrite>),
}

impl SceneMessage for InternalSocketMessage { }

///
/// The stream reader is used to convert an input stream of bytes into an AsyncRead implementation
///
struct StreamReader<TSourceStream: ?Sized> {
    source:     Option<Pin<Box<TSourceStream>>>,
    pending:    Vec<u8>,
}

impl<TSourceStream> AsyncRead for StreamReader<ReadyChunks<TSourceStream>>
where
    TSourceStream: Stream<Item=u8>,
{
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        loop {
            // If 0 bytes are read but 'OK' is returned, we keep trying to read until the source blocks (as otherwise we'll get stuck)
            if self.pending.is_empty() {
                // If pending is empty we've got no bytes to return: try to read from teh source stream
                if let Some(source) = self.source.as_mut() {
                    // Poll for data from the source stream
                    match ReadyChunks::<TSourceStream>::poll_next(source.as_mut(), cx) {
                        Poll::Pending => {
                            // No more bytes to read at the moment
                            break Poll::Pending;
                        }

                        Poll::Ready(None) => {
                            // EOF, nothing more to read (disconnect from the source stream at this point)
                            self.source = None;
                            break Poll::Ready(Ok(()));
                        }

                        Poll::Ready(Some(new_bytes)) => {
                            // Add the bytes that we read to the internal buffer
                            self.pending.extend(new_bytes);
                        }
                    }
                } else {
                    // EOF has been hit before
                    break Poll::Ready(Ok(()));
                }
            }

            if !self.pending.is_empty() {
                // Read from the pending buffer into the read buffer
                let to_copy = self.pending.len().min(buf.remaining());

                // Write the bytes to the output buffer
                buf.put_slice(&self.pending[0..to_copy]);
                self.pending.splice(0..to_copy, []);

                // Some bytes were read into the pending or the read buffer
                break Poll::Ready(Ok(()));
            }
        }
    }
}

enum StreamWriterState {
    Idle,
    Writing(usize),
    WaitingForReady,
}

///
/// The stream writer converts an output sink of bytes into an AsyncWrite implementation
///
struct StreamWriter<TTargetSink> {
    state:  StreamWriterState,
    target: Pin<Box<TTargetSink>>,
}

impl<TTargetSink> AsyncWrite for StreamWriter<TTargetSink>
where
    TTargetSink: Sink<u8>,
{
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        let mut num_written = match self.state {
            StreamWriterState::Idle                 |
            StreamWriterState::WaitingForReady      => 0,
            StreamWriterState::Writing(num_bytes)   => num_bytes,
        };

        loop {
            // Indicate 'ready' if all the bytes are written
            if num_written >= buf.len() {
                self.state = StreamWriterState::Idle;
                return Poll::Ready(Ok(num_written));
            }

            // Poll for readiness
            match TTargetSink::poll_ready(self.target.as_mut(), cx) {
                Poll::Pending => {
                    // Can't send any bytes immediately
                    self.state = StreamWriterState::Writing(num_written);

                    // Wait until the sink is ready before writing more
                    return Poll::Pending;
                }

                Poll::Ready(Ok(())) => {
                    // Send the next byte
                    match TTargetSink::start_send(self.target.as_mut(), buf[num_written]) {
                        Ok(()) => { },
                        Err(_) => { return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Error while sending byte"))); },
                    }

                    // Add to the number of written bytes, go through the loop again to try to send more if we can
                    num_written += 1;
                    self.state = StreamWriterState::Writing(num_written);
                }

                Poll::Ready(Err(_)) => {
                    return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Error while waiting for ready")));
                }
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        loop {
            // If we started to wait for readiness, finish that first
            match self.state {
                StreamWriterState::Writing(_)      |
                StreamWriterState::WaitingForReady => {
                    // Poll for readiness
                    match TTargetSink::poll_ready(self.target.as_mut(), cx) {
                        Poll::Pending       => { self.state = StreamWriterState::WaitingForReady; return Poll::Pending; },
                        Poll::Ready(Ok(())) => { self.state = StreamWriterState::Idle; },
                        Poll::Ready(Err(_)) => { return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Error while waiting for ready"))); }
                    }
                },

                StreamWriterState::Idle => {
                    match TTargetSink::poll_flush(self.target.as_mut(), cx) {
                        Poll::Pending       => { return Poll::Pending; }
                        Poll::Ready(Ok(())) => { return Poll::Ready(Ok(())); }
                        Poll::Ready(Err(_)) => { return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Error while waiting for flush"))); }
                    }
                }
            }
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        match TTargetSink::poll_close(self.target.as_mut(), cx) {
            Poll::Pending       => Poll::Pending,
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(_)) => Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Error while waiting for sink to close")))
        }
    }
}

impl InternalSocketMessage {
    ///
    /// Returns a 'CreateInternalSocket' request which will send the messages generated by an input stream of bytes and receive the messages
    /// sent by an output stream of bytes
    ///
    pub fn create_socket_from_streams(input: impl 'static + Send + Stream<Item=u8>, output: impl 'static + Send + Sink<u8>) -> InternalSocketMessage {
        let input_stream    = StreamReader { source: Some(Box::pin(input.ready_chunks(256))), pending: Vec::with_capacity(256) };
        let output_stream   = StreamWriter { state: StreamWriterState::Idle, target: Box::pin(output) };

        InternalSocketMessage::CreateInternalSocket(Box::new(input_stream), Box::new(output_stream))
    }
}

///
/// Creates an internal socket program
///
pub fn start_internal_socket_program<TInputStream, TOutputMessage>(
    scene:                  &Scene, 
    program_id:             SubProgramId, 
    create_input_messages:  impl 'static + Send + Sync + Fn(BoxStream<'static, Vec<u8>>) -> TInputStream, 
    create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, TOutputMessage>) -> BoxStream<'static, Vec<u8>>
) -> Result<(), ConnectionError> 
where
    TInputStream:   'static + Send + Stream,
    TOutputMessage: 'static + Send,
{
    let create_output_messages      = Arc::new(create_output_messages);
    let mut subscribers             = EventSubscribers::new();

    // The internal socket program responds to InternalSocketMessages and sends subscriptions from the inner program
    scene.add_subprogram(program_id, move |input, context| async move {
        let mut input = input.messages_with_sources();

        while let Some((source, request)) = input.next().await {
            match request {
                InternalSocketMessage::CreateInternalSocket(async_reader, async_writer) => {
                    // Create the socket connection from the reader
                    let reader_stream = create_reader_stream(Box::into_pin(async_reader));
                    let reader_stream = create_input_messages(reader_stream.boxed());

                    let create_output_messages  = Arc::clone(&create_output_messages);
                    let socket_connection       = SocketConnection::new(&context, reader_stream, move |context, output_stream| {
                        // Create a stream that converts to bytes
                        let mut output_byte_stream = create_output_messages(output_stream);

                        // Future to write the bytes
                        let async_writer = Box::into_pin(async_writer);
                        let byte_writer  = async move {
                            // Write each block as it arrives from the output byte stream to the socket target
                            let mut async_writer = async_writer;
                            while let Some(bytes) = output_byte_stream.next().await {
                                // Loop until we've written all of the bytes
                                let mut write_pos = 0;

                                while write_pos < bytes.len() {
                                    match async_writer.write(&bytes[write_pos..(bytes.len())]).await {
                                        Ok(0)           => break,
                                        Err(_)          => break,
                                        Ok(num_written) => {
                                            write_pos += num_written;
                                            if write_pos >= bytes.len() {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        };

                        // Ask the scene to create a subprogram that writes the output (won't work if the main 'scene' program isn't running)
                        let output_program = SubProgramId::new();
                        let output_program = SceneControl::start_program(output_program, move |_: InputStream<()>, _| byte_writer, 0);

                        let mut control = context.send(()).unwrap();
                        control.send_immediate(output_program).ok();
                    });

                    // Send this connection to the subscribers
                    let maybe_failed_message = subscribers.send_round_robin(SocketMessage::Connection(socket_connection)).await;

                    if let Some(failed_message) = maybe_failed_message {
                        // No subscriber was available to receive the message successfully
                        todo!()
                    }
                },

                InternalSocketMessage::Subscribe => {
                    // Add to the subscribers
                    subscribers.subscribe(&context, source)
                }
            }
        }
    }, 0);

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::executor;
    use futures::channel::mpsc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn stream_reader_read() {
        let input_stream    = vec![0u8, 1u8, 2u8, 3u8, 4u8, 5u8, 6u8];
        let input_stream    = stream::iter(input_stream.into_iter());
        let stream_reader   = StreamReader { source: Some(Box::pin(input_stream.ready_chunks(256))), pending: vec![] };

        let read_bytes = executor::block_on(async {
            let mut stream_reader   = stream_reader;
            let mut result          = vec![];
            let mut buf             = [0u8, 0u8];

            while let Ok(num_read) = stream_reader.read(&mut buf).await {
                if num_read == 0 { break; }

                result.extend(buf[0..num_read].iter().copied());
            }

            result
        });

        assert!(read_bytes == vec![0u8, 1u8, 2u8, 3u8, 4u8, 5u8, 6u8], "{:?}", read_bytes);
    }

    #[test]
    fn stream_writer_write() {
        // Create an output stream
        let (write, read) = mpsc::channel::<u8>(1);
        let output_stream = StreamWriter { target: Box::pin(write), state: StreamWriterState::Idle };

        // Create a future that writes some bytes to the output stream
        let write_bytes = async move {
            let mut output_stream   = output_stream;
            let bytes               = vec![0u8, 1, 2, 3, 4, 5, 6];

            output_stream.write(&bytes).await.unwrap();
        };

        // Read the bytes from the reading stream
        let read_bytes = async move {
            let mut read        = read;
            let mut bytes_read  = vec![];

            while let Some(next_byte) = read.next().await {
                bytes_read.push(next_byte);
            }

            assert!(bytes_read == vec![0u8, 1, 2, 3, 4, 5, 6], "Read {:?}", bytes_read);
        };

        executor::block_on(future::join(write_bytes, read_bytes));
    }
}