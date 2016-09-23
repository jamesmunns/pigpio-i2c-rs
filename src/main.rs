fn main() {
    println!("Hello, world!");
}

#[derive(Debug)]
struct I2cEngine {
    old_scl: bool,
    old_sda: bool,
    partial_data: u8,
    current_bit: u8,
    active: bool,
    bytes: Vec<I2cByte>,
}

#[derive(Debug)]
enum SclState {
    Rising,
    Falling,
    Steady,
}

#[derive(Debug)]
enum SdaState {
    Rising,
    Falling,
    Steady,
}

#[derive(Debug, PartialEq)]
pub enum DecodeState {
    Idle,
    Pending,
    Complete(Vec<I2cByte>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum I2cStatus {
    Ack,
    Nak
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct I2cByte {
    data: u8,
    status: I2cStatus,
}

impl I2cEngine {
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

    pub fn update_i2c(&mut self, new_scl: bool, new_sda: bool) -> DecodeState {
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

        self.old_scl = new_scl;
        self.old_sda = new_sda;

        match (scl_state, sda_state, self.active, new_scl, self.current_bit) {
            (SclState::Steady, SdaState::Rising, true, true, _) => {
                // Stop condition, with data
                let ret = self.bytes.to_owned();
                self.bytes.clear();
                self.partial_data = 0;
                self.current_bit = 0;
                self.active = false;
                return DecodeState::Complete(ret);
            },
            (SclState::Steady, SdaState::Falling, false, true, _) => {
                // Start condition
                self.active = true;
            },
            (SclState::Rising, _, true, _, 0...7) => {
                // Capture bit
                self.partial_data <<= 1;
                self.partial_data |= if new_sda {1} else {0};
                self.current_bit += 1;
            },
            (SclState::Rising, _, true, _, _) => {
                // Latch byte
                self.bytes.push(I2cByte{
                    data: self.partial_data,
                    status: if new_sda {I2cStatus::Nak} else {I2cStatus::Ack}
                });
                self.partial_data = 0;
                self.current_bit = 0;

            },
            _ => {},
        }

        match self.active {
            true => DecodeState::Pending,
            false => DecodeState::Idle
        }
    }
}

mod test {
    use super::{I2cEngine, I2cByte, DecodeState};

    fn start(machine: &mut I2cEngine)
    {
        assert_eq!(machine.update_i2c(true, true), DecodeState::Idle);
        assert_eq!(machine.update_i2c(true, false), DecodeState::Pending);
    }

    fn feed_one_bit(machine: &mut I2cEngine, bit: bool)
    {
        assert_eq!(machine.update_i2c(false, bit), DecodeState::Pending);
        assert_eq!(machine.update_i2c(true, bit), DecodeState::Pending);
        assert_eq!(machine.update_i2c(false, bit), DecodeState::Pending);
    }

    fn feed_one_byte(machine: &mut I2cEngine, byte: u8)
    {
        let mut byte = byte;

        // Data
        for _ in 0..8 {
            let state = if 0x80 == byte & 0x80 {true} else {false};
            byte <<= 1;
            feed_one_bit(machine, state)
        }

        // Ack/nak
        assert_eq!(machine.update_i2c(true, false), DecodeState::Pending);
        assert_eq!(machine.update_i2c(false, false), DecodeState::Pending);
    }

    fn stop(machine: &mut I2cEngine) -> Vec<I2cByte>
    {
        assert_eq!(machine.update_i2c(false, false), DecodeState::Pending);
        assert_eq!(machine.update_i2c(true, false), DecodeState::Pending);
        match machine.update_i2c(true, true) {
            DecodeState::Complete(i) => i,
            _ => {panic!(":(");}
        }
    }

    #[test]
    fn test_bytes() {
        let tests = vec!(
            vec!(),
            vec!(0x00u8),
            vec!(0x00u8, 0x00u8),
            vec!(0xF0u8),
            vec!(0x01u8, 0x02u8, 0x03u8, 0xA0u8, 0xB0u8, 0xC0u8),
        );

        let mut x = I2cEngine::new();

        for t in tests {
            start(&mut x);
            for b in &t {
                feed_one_byte(&mut x, *b);
            }

            // Todo, some kind of map function for unwrapping data from Vec<I2cByte>
            let bv = stop(&mut x);
            let mut out: Vec<u8> = Vec::new();
            for b in bv {
                out.push(b.data);
            }
            assert_eq!(out, t);
        }
    }
}