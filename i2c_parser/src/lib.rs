//! # I2C Parsing State Machine Library
//!
//! Rust implementation of [pigpio's I2C Sniffer](https://github.com/joan2937/pigpio/tree/master/EXAMPLES/C/I2C_SNIFFER).

use std::fmt;
extern crate tokio_core;

/// Structure for parsing I2C Messages from raw SDA and SCL inputs
#[derive(Debug)]
pub struct I2cEngine {
    old_scl: bool,
    old_sda: bool,
    partial_data: u8,
    current_bit: u8,
    active: bool,
    bytes: Vec<I2cByte>,
}

/// Structure containing a complete I2C message comprised of `I2cByte`s
#[derive(Debug, PartialEq)]
pub struct I2cMessage {
    pub message: Vec<I2cByte>,
}

/// A single byte of I2C Data, including ACK or NAK state
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct I2cByte {
    pub data: u8,
    pub status: I2cStatus,
}

/// Current behavior of the SCL line
#[derive(Debug)]
enum SclState {
    Rising,
    Falling,
    Steady,
}

/// Current behavior of the SDA line
#[derive(Debug)]
enum SdaState {
    Rising,
    Falling,
    Steady,
}

impl I2cMessage {
    /// Obtain only the bytes from an I2C Message, discarding ACK and NAKs
    pub fn get_payload(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for b in &self.message {
            out.push(b.data);
        }
        out
    }
}

impl fmt::Display for I2cMessage {
    /// Implementation of the display trait for use with `println!()`, etc.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut out = String::new();
        out.push_str(&("["));
        for byte in &self.message {
            out.push_str(&(format!("{:02X}", byte.data)));
            out.push_str(&(format!("{}", match byte.status {
                I2cStatus::Ack => "+",
                I2cStatus::Nak => "-",
            })));
        }
        out.push_str(&(format!("]")));
        write!(f, "{}", out)
    }
}

/// Representation of the current engine state
///
/// * Idle: A message has not yet been started
/// * Pending: An I2C START condition has been received, waiting for a STOP
/// * Complete: A STOP condition has just occurred, and contains all bytes received between START and STOP
#[derive(Debug, PartialEq)]
pub enum DecodeState {
    Idle,
    Pending,
    Complete(I2cMessage),
}

/// Representation of ACK/NAK bit after every 8 bits of data
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum I2cStatus {
    Ack,
    Nak
}

impl I2cEngine {
    /// Create a new I2CEngine in the idle and empty state
    pub fn new() -> I2cEngine {
        I2cEngine {
            old_scl: true,
            old_sda: true,
            partial_data: 0u8,
            current_bit: 0u8,
            active: false,
            bytes: Vec::new(),
        }
    }

    /// Process one sample of SDA and SCL data from an I2C bus.
    ///
    /// Returns the current state, as well as a message if a STOP condition was
    ///   just received
    pub fn update_i2c(&mut self, new_scl: bool, new_sda: bool) -> DecodeState {
        // Determine current SCL and SDA behavior
        let scl_state = match (self.old_scl, new_scl) {
            (false, false) => SclState::Steady,
            (false, true)  => SclState::Rising,
            (true, false)  => SclState::Falling,
            (true, true)   => SclState::Steady,
        };

        let sda_state = match (self.old_sda, new_sda) {
            (false, false) => SdaState::Steady,
            (false, true)  => SdaState::Rising,
            (true, false)  => SdaState::Falling,
            (true, true)   => SdaState::Steady,
        };

        // Save off state for next update
        self.old_scl = new_scl;
        self.old_sda = new_sda;

        // Process state transition, based on current data
        match (scl_state, sda_state, self.active, new_scl, self.current_bit) {
            (SclState::Steady, SdaState::Rising, true, true, _) => {
                // Stop condition, after previously receiving a Start Condition
                let ret = I2cMessage{message:self.bytes.to_owned()};
                self.bytes.clear();
                self.partial_data = 0;
                self.current_bit = 0;
                self.active = false;
                return DecodeState::Complete(ret);
            },
            (SclState::Steady, SdaState::Falling, false, true, _) => {
                // Start condition from idle state
                self.active = true;
            },
            (SclState::Rising, _, true, _, 0...7) => {
                // Capture bit of whole byte
                self.partial_data <<= 1;
                self.partial_data |= if new_sda {1} else {0};
                self.current_bit += 1;
            },
            (SclState::Rising, _, true, _, _) => {
                // 8 bits received, observe ACK/NAK and record byte
                self.bytes.push(I2cByte{
                    data: self.partial_data,
                    status: if new_sda {I2cStatus::Nak} else {I2cStatus::Ack}
                });
                self.partial_data = 0;
                self.current_bit = 0;
            },
            _ => {},
        }

        // A message was not recieved, return the current state
        match self.active {
            true => DecodeState::Pending,
            false => DecodeState::Idle
        }
    }
}

#[cfg(test)]
mod test {
    use super::{I2cEngine, DecodeState, I2cMessage};

    /// Helper function to send a START condition
    fn start(machine: &mut I2cEngine)
    {
        assert_eq!(machine.update_i2c(true, true), DecodeState::Idle);
        assert_eq!(machine.update_i2c(true, false), DecodeState::Pending);
    }

    /// Helper function to send one bit of data
    fn feed_one_bit(machine: &mut I2cEngine, bit: bool)
    {
        assert_eq!(machine.update_i2c(false, bit), DecodeState::Pending);
        assert_eq!(machine.update_i2c(true, bit), DecodeState::Pending);
        assert_eq!(machine.update_i2c(false, bit), DecodeState::Pending);
    }

    /// Helper function to send 8 bits of data and an ACK
    fn feed_one_byte(machine: &mut I2cEngine, byte: u8)
    {
        let mut byte = byte;

        // Data
        for _ in 0..8 {
            let state = 0x80 == (byte & 0x80);
            byte <<= 1;
            feed_one_bit(machine, state)
        }

        // Always Ack
        assert_eq!(machine.update_i2c(true, false), DecodeState::Pending);
        assert_eq!(machine.update_i2c(false, false), DecodeState::Pending);
    }

    /// Helper function to send a STOP condition
    fn stop(machine: &mut I2cEngine) -> I2cMessage
    {
        assert_eq!(machine.update_i2c(false, false), DecodeState::Pending);
        assert_eq!(machine.update_i2c(true, false), DecodeState::Pending);
        match machine.update_i2c(true, true) {
            DecodeState::Complete(i) => i,
            _ => {panic!("Unexpected incomplete message!");}
        }
    }

    /// Test various sequences of bytes to be processed by the engine. Assert
    ///   that message is reassembled correctly
    #[test]
    fn test_bytes() {
        let tests = vec!(
            vec!(),
            vec!(0x00),
            vec!(0x00, 0x00),
            vec!(0xF0),
            vec!(0x01, 0x02, 0x03, 0xA0, 0xB0, 0xC0),
        );

        let mut x = I2cEngine::new();

        for t in tests {
            start(&mut x);
            for b in &t {
                feed_one_byte(&mut x, *b);
            }

            assert_eq!(stop(&mut x).get_payload(), t);
        }
    }
}
