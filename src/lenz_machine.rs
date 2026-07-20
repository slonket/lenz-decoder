use crate::lenz_packet::{LenzPacket};

// Result Shorthand and Errors
type Result<T> = core::result::Result<T, LenzMachineError>;
use LenzMachineError::*;

pub enum LenzMachineError {
    PacketSizeExceeded,
}

// Lenz measured timings
const LENZ_0_MIN: u16 = 110 - 5;
const LENZ_0_MAX: u16 = 110 + 5;
const LENZ_1_MIN: u16 = 55 - 5;
const LENZ_1_MAX: u16 = 55 + 5;

const LENZ_N_PREAMBLE: u8 = 13;

/// An abstraction of a pulse duration binned into a legal Lenz value.
#[derive(Default, PartialEq)]
enum LenzPulse {
    HalfZero,
    HalfOne,
    IdlePrev,
    #[default]
    Invalid,
}

impl From<u16> for LenzPulse {
    fn from(value: u16) -> Self {
        match value {
            LENZ_0_MIN..=LENZ_0_MAX => LenzPulse::HalfZero,
            LENZ_1_MIN..=LENZ_1_MAX => LenzPulse::HalfOne,
            _ => LenzPulse::Invalid
        }
    }
}

/// Abstracted bit type for the Lenz state machine.
#[derive(PartialEq)]
enum LenzBit {
    Zero,
    One,
}

impl From<LenzBit> for u8 {
    fn from(lenz_bit: LenzBit) -> Self {
        match lenz_bit {
            LenzBit::Zero => 0x00,
            LenzBit::One => 0x01,
        }
    }
}

/// A wrapper type for u8 that allows "building" the data one bit at a time.
/// Individual bits (bitmask u8) can be "pushed" to the byte. When 8 bits have been pushed,
/// the byte is complete and will be returned by the `push()` function.
struct BuildableU8 {
    data: u8,
    index: u8,
}

impl BuildableU8 {

    const fn new() -> Self {
        Self {
            data: 0,
            index: 0,
        }
    }

    fn push(&mut self, bit: u8) -> Option<u8> {

        self.data = (self.data << 1) | (bit & 0x01);
        self.index += 1;

        if self.index >= 8 {
            self.index = 0;
            return Some(self.data);
        }

        None
    }
}

/// A raw array of bytes that [`LenzMachine`] can build into a [`LenzPacket`].
#[derive(Clone, Copy)]
struct BuildableLenzPacket {
    data: [u8; 3],
    index: usize,
}

impl BuildableLenzPacket {
    const fn new() -> Self {
        Self {
            data: [0; 3],
            index: 0,
        }
    }

    // reset the buildable packet
    // the data doesn't need to be reset since it's index-locked
    fn reset(&mut self) {
        self.index = 0;
    }

    // push a completed byte into data
    fn push(&mut self, byte: u8) -> Result<()> {
        if self.index == 3 {
            return Err(PacketSizeExceeded);
        }
        self.data[self.index] = byte;
        self.index += 1;
        Ok(())
    }

    // checks the packet is 3 bytes long with correct checksum (XOR)
    // returns only the 2 payload bytes if valid as LenzPacket
    fn validate(&self) -> Option<LenzPacket> {

        // check length
        if self.index != 3 {
            return None;
        }

        // checksum validation
        if self.data[0] ^ self.data[1] != self.data[2] {
            return None;
        }

        // return valid packet
        Some(LenzPacket::new([self.data[0], self.data[1]]))
    }
}

enum LenzState {
    Preamble(u8),
    Start,
    Data(BuildableU8),
    Intermission,
}

/// A state machine for decoding Lenz packets from pulses.
/// The machine requires a consistent stream of "pulses" in microseconds. The stream of pulses
/// should represent the contiguous high and low durations of a toggling datasource. The
/// actual polarity of the pulses is not required - the delimitation of the pulses is sufficient
/// for determining Lenz data.
pub struct LenzMachine {
    state: LenzState,
    prev_pulse: LenzPulse,
    packet: BuildableLenzPacket,
}

impl LenzMachine {

    /// Create a new Lenz machine.
    pub const fn new() -> Self {
        Self {
            state: LenzState::Preamble(0),
            prev_pulse: LenzPulse::Invalid,
            packet: BuildableLenzPacket::new(),
        }
    }

    /// Every invocation of this function should provide the state machine a new, contiguous
    /// pulse sample. The packets formed by this machine are invalid if the data is not
    /// contiguous.
    pub fn advance(&mut self, pulse: u16) -> Option<LenzPacket> {

        let lenz_pulse = LenzPulse::from(pulse);

        // when a non-Lenz halfbit is encountered, the Lenz pulse chain has been interrupted
        // the state machine needs to be reset for a new packet
        if lenz_pulse == LenzPulse::Invalid {
            self.reset();
            return None;
        }

        // bit resolver - developer note: this was taken from the C implementation.
        // This is a rolling comparison for pairs of half-bits. The comparison is aligned
        // to two half-bits. It is possible for the state machine to misaligned; this is
        // detected by the bit resolver and corrected for.
        let lenz_bit: LenzBit = match self.prev_pulse {
            LenzPulse::IdlePrev => {
                self.prev_pulse = lenz_pulse;
                return None;
            }
            _ => {
                if lenz_pulse != self.prev_pulse {
                    // a mismatch means disorientation across the half-bit pairs.
                    // store the current half-bit to "re-orientated" for the expected compliment
                    self.prev_pulse = lenz_pulse;

                    // re-orientating during the start condition is acceptable.
                    // the "one" bits preceding this state are not data, and do not need to be
                    // recorded. The starting "zero" bit is the first indicator that can be
                    // used for orientation. As such, we don't need to reset in this state.
                    // Resetting in preamble is necessary as a zero half-bit would mean
                    // a preamble that is too short.
                    match self.state {
                        LenzState::Start => {}
                        _ => {
                            self.reset();
                        }
                    }
                    return None;
                } else {
                    // clear the previously held type
                    self.prev_pulse = LenzPulse::IdlePrev;
                    match lenz_pulse {
                        LenzPulse::HalfZero => {
                            LenzBit::Zero
                        }
                        LenzPulse::HalfOne => {
                            LenzBit::One
                        }
                        _ => {
                            // the invalid half-bit type is unreachable as it is filtered
                            // by the first comparison in this function.
                            unreachable!()
                        }
                    }
                }
            }
        };

        // the state machine - it is guaranteed that a Lenz bit has been found by this point
        match &mut self.state {
            LenzState::Preamble(count) => {
                match lenz_bit {
                    LenzBit::Zero => {
                        *count = 0;
                    }
                    LenzBit::One => {
                        // the check below should never allow count to exceed the 255, but
                        // we don't want any chance of runtime problems with wrapping. In
                        // this application, wrapping on the preamble count is okay - that
                        // just means the preamble has reset.
                        *count = count.wrapping_add(1);
                        // because count increments one at a time, this comparison shouldn't
                        // need to be >=. But, it feels safer to capture any condition where
                        // the value exceeds the minimum, even if this shouldn't happen.
                        if *count >= LENZ_N_PREAMBLE {
                            self.state = LenzState::Start;
                        }
                    }
                }
            }
            LenzState::Start => {
                // any number of additional ones can occur before the starting 0;
                // as such, we only care about the zero condition to progress the machine.
                match lenz_bit {
                    LenzBit::One => {}
                    LenzBit::Zero => {
                        self.state = LenzState::Data(BuildableU8::new());
                    }
                }
            }
            LenzState::Data(data) => {
                match data.push(lenz_bit.into()) {
                    Some(byte) => {

                        match self.packet.push(byte) {
                            Ok(_) => {}
                            Err(_) => {
                                // in the case where the maximum packet size is exceeded,
                                // the packet is considered erroneous. Simply reset.
                                self.reset();
                            }
                        }

                        self.state = LenzState::Intermission;
                    }
                    None => {}
                }
            }
            LenzState::Intermission => {
                match lenz_bit {
                    LenzBit::Zero => {
                        // still more data to come!
                        self.state = LenzState::Data(BuildableU8::new());
                    }
                    LenzBit::One => {
                        // packet complete - return a LenzPacket
                        let ret_packet = self.packet.validate();
                        self.reset();
                        return ret_packet;
                    }
                }
            }
        }

        // no packet to return this cycle
        None
    }

    /// Resets the [`LenzMachine`]. Used when illegal conditions are met.
    #[inline]
    fn reset(&mut self) {
        self.packet.reset();
        self.state = LenzState::Preamble(0);
        self.prev_pulse = LenzPulse::IdlePrev;
    }
}