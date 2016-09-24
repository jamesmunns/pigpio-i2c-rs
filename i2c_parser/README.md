# I2C Parser

Rust implementation of [pigpio's I2C Sniffer](https://github.com/joan2937/pigpio/tree/master/EXAMPLES/C/I2C_SNIFFER).

Raw I2C SDA/SCL bits -> Hex. Message starts noted with `[`, bytes in `:02X` format, byte ACKs `+`, byte NAKs `-`, message end noted with `]`.