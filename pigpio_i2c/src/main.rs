extern crate i2c_parser;
extern crate tokio_core;
extern crate futures;

use i2c_parser::{I2cEngine, DecodeState, I2cMessage};
use std::{mem, io, env};
use std::io::Read;

type GpioBuffer = [u8; 12];

#[repr()]
#[derive(Debug)]
struct GpioReportRaw {
    seqno: u16,
    flags: u16,
    tick: u32,
    level: u32,
}

impl GpioReportRaw {
    fn from_buffer(buf: GpioBuffer) -> GpioReportRaw {
        unsafe {mem::transmute::<GpioBuffer, GpioReportRaw>(buf)}
    }
}

// -----------------------------------------------------------------------------

use futures::stream::*;
use futures::Poll;
use futures::Async;
use futures::Future;


struct GpioReportStream<R: Read> {
    input: R,
}

impl<R: Read> Stream for GpioReportStream<R> {
    type Item = GpioReportRaw;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        let mut buf: GpioBuffer = [0u8; 12];

        // TODO - Make this non-blocking, etc
        try!(self.input.read_exact(&mut buf));

        // Okay... what the fuck is this noise?
        // Result(Async(Option(Item))) - Seriously?
        //
        // Poll   -> Async || Error    => Result type
        // Async  -> Ready || NotReady => ::Ready
        // Option -> Some  || None     => Complete, message decoded?
        // Item   -> Buffer!
        Ok(Async::Ready(Some(GpioReportRaw::from_buffer(buf))))
    }
}

fn main() {

    let mut parse = I2cEngine::new();
    let mut stdin = io::stdin();
    let mut buf: GpioBuffer = [0u8; 12];

    // Todo: better argc handling
    let mut args = env::args();
    args.next(); // binary name
    let scl_mask: u32 = 1 << args.next().unwrap().parse::<u8>().unwrap();
    let sda_mask: u32 = 1 << args.next().unwrap().parse::<u8>().unwrap();

    println!("0x{:08X} 0x{:08X}", scl_mask, sda_mask);

    let mut raw_msg_stream = GpioReportStream {
        input: stdin,
    }.map(|msg_raw| {
        let scl = scl_mask == (msg_raw.level & scl_mask);
        let sda = sda_mask == (msg_raw.level & sda_mask);

        if let DecodeState::Complete(msg) = parse.update_i2c(scl, sda) {
            // TODO(AJM):
            // What I really want is to "yield" a format!("{}", msg), then
            // use this stream like an iterator, breaking when some kind of
            // error propigates through. Right now, I am just using some kind
            // of side effect magic to get the result that I want, but if I needed
            // to pass on as a stream, I don't know wtf to do.
            println!("{}", msg);
        }
    });

    loop {
        match raw_msg_stream.poll() {
            Ok(_) => {},
            Err(_) => {break;}
        }
    }
}
