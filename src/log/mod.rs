//! Debug log interface.

pub mod dso;
pub mod swo;

mod output;

pub use self::output::{Output, OutputMap, OutputStream};

use anyhow::Result;
use std::{
    ops::{Generator, GeneratorState},
    pin::Pin,
};
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};
use crate::color::Color;
use std::io::Read;

type ParserFn<'a> = fn(OutputMap) -> Pin<Box<dyn Generator<u8, Yield = (), Return = Result<!>> + 'a + Send>>;

/// Runs log capture thread.
pub fn capture<'a>(serial_endpoint: &str, baud_rate: u32,
    // input: impl AsyncRead + Send + 'static,
    outputs: OutputMap,
    parser: ParserFn<'static>,
    color: Color,
) {
    begin_log_output(color);
    let serial = mio_serial::new(serial_endpoint, baud_rate).open().unwrap();
    // for byte in serial.bytes() {
    //     sender.send(byte.unwrap()).await.unwrap();
    // }
    tokio::spawn(async move {
        let parser = parser(outputs);
        futures::pin_mut!(parser);
        // let input = BufReader::new(input);
        // futures::pin_mut!(input);
        let mut bytes = serial.bytes();
        loop {
            let byte = tokio::task::block_in_place(|| bytes.next().unwrap().unwrap());
            log::debug!("BYTE 0b{0:08b} 0x{0:02X} {1:?}", byte, char::from(byte));
            match parser.as_mut().resume(byte) {
                GeneratorState::Yielded(()) => (),
                GeneratorState::Complete(Err(err)) => panic!("log parser failure: {}", err),
            }
        }
    });
}

/// Displays a banner representing beginning of log output.
pub fn begin_log_output(color: Color) {
    eprintln!();
    eprintln!("{}", color.bold_fg(&format!("{:=^80}", " LOG OUTPUT "), ansi_term::Color::Cyan));
}