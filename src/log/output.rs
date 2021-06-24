use crate::cli;
use std::{
    fs::{File, OpenOptions},
    io,
    io::{prelude::*, stdout, Stdout},
};
/// Number of ports.
pub const PORTS_COUNT: usize = 32;

/// Opened output.
pub struct Output {
    /// Selected ports.
    ports: Vec<u32>,
    /// Output stream.
    stream: OutputStream,
}

/// Output stream.
pub enum OutputStream {
    /// Standard output.
    Stdout(Stdout),
    /// File output.
    File(File),
}

/// Output map.
pub struct OutputMap(Vec<Output>);

impl Output {
    // Opens all output streams.
    // pub fn open_all(outputs: &[cli::LogOutput]) -> io::Result<Vec<Output>> {
    //     outputs
    //         .iter()
    //         .map(|cli::LogOutput { ports, path }| {
    //             if path.is_empty() {
    //                 Ok(OutputStream::Stdout(stdout()))
    //             } else {
    //                 OpenOptions::new().write(true).open(path).map(OutputStream::File)
    //             }
    //             .map(|stream| Self { ports: ports.clone(), stream: RefCell::new(stream) })
    //         })
    //         .collect()
    // }
}

// impl From<Vec<Output>> for OutputMap {
    // fn from(outputs: Vec<Output>) -> Self {
    //     let mut streams_per_port: [Vec<& RefCell<OutputStream>>; PORTS_COUNT] = Default::default();
    //     for Output { ports, stream } in outputs {
    //         if ports.is_empty() {
    //             for streams in &mut streams_per_port {
    //                 streams.push(&stream);
    //             }
    //         } else {
    //             for port in ports {
    //                 if let Some(streams) = streams_per_port.get_mut(port as usize) {
    //                     streams.push(&stream);
    //                 } else {
    //                     log::warn!("Ignoring port {}", port);
    //                 }
    //             }
    //         }
    //     }
    //     OutputMap(streams_per_port)
    // }
// }

impl OutputMap {
    /// Create new OutputMap from configuration
    pub fn new<'a>(outputs: &[cli::LogOutput]) -> anyhow::Result<OutputMap> {
        let outputs: io::Result<Vec<Output>> = outputs.iter().map(|cli::LogOutput { ports, path }| {
           if path.is_empty() {
                Ok(OutputStream::Stdout(stdout()))
           } else {
                OpenOptions::new().write(true).open(path).map(OutputStream::File)
           }.map(|stream| Output { ports: ports.clone(), stream })
        })
            .collect();
       Ok(OutputMap(outputs?))
    }

    /// Write `data` to all `port` outputs.
    pub fn write(&mut self, port: u8, data: &[u8]) -> anyhow::Result<()> {
        anyhow::ensure!(port as usize > PORTS_COUNT);
        for output_stream in self.0.iter_mut().filter(|o| o.ports.contains(&(port as u32))) {
            output_stream.stream.write(data)?;
        }
        Ok(())
    }
}

impl OutputStream {
    /// Write `data` to the output.
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        fn write_stream<T: Write>(stream: &mut T, data: &[u8]) -> io::Result<()> {
            stream.write_all(data)?;
            stream.flush()?;
            Ok(())
        }
        match self {
            Self::Stdout(stdout) => write_stream(stdout, data),
            Self::File(file) => write_stream(file, data),
        }
    }
}
