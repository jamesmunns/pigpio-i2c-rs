extern crate i2c_parser;
use i2c_parser::{I2cEngine, DecodeState};
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

    loop {
        match stdin.read_exact(&mut buf) {
            Ok(_) => {
                let msg_raw = GpioReportRaw::from_buffer(buf);
                let scl = scl_mask == (msg_raw.level & scl_mask);
                let sda = sda_mask == (msg_raw.level & sda_mask);
                match parse.update_i2c(scl, sda) {
                    DecodeState::Complete(msg) => {
                        println!("{}", msg);
                    }
                    _ => {}
                }
            },
            _ => panic!(),
        }
    }
}