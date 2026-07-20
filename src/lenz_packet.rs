
/// Raw validated packet of Lenz protocol with unknown subtype.
pub struct LenzPacket {
    data: [u8; 2]
}

/// Packet abstraction covering all packet subtypes.
pub enum LenzCommand {
    Speed(LenzSpeedCommand),
    Function(LenzFunctionCommand),
    Accessory(LenzAccessoryCommand),
}

impl LenzPacket {

    /// Produces an undetermined packet using raw extracted protocol data (excluding the parity byte).
    pub(crate) fn new(data: [u8; 2]) -> Self {
        Self {
            data
        }
    }

    /// Resolves the packet type and returns a [`LenzCommand`] with specific packet.
    pub fn get_type(self) -> Option<LenzCommand> {
        match self.data[0] {
            0b0000_0001..=0b01111111 => { // locomotive packet
                match self.data[1] & 0b1100_0000 {
                    0b0100_0000 => { // speed command
                        Some(LenzCommand::Speed(LenzSpeedCommand { data: self.data }))
                    }
                    0b1000_0000 => { // function command
                        Some(LenzCommand::Function(LenzFunctionCommand { data: self.data }))
                    }
                    _ => None,
                }
            }
            0b1000_0000..=0b1011_1111 => { // accessory command
                Some(LenzCommand::Accessory(LenzAccessoryCommand { data: self.data }))
            }
            _ => None,
        }
    }
}

/// Packet subtype of Lenz protocol for locomotive speed, direction, and headlight function.
pub struct LenzSpeedCommand {
    data: [u8; 2],
}

/// Speed differentiating "emergency stop" (1) and speed.
/// The speed value is shifted to the range 0-14 with 0 being stop.
pub enum LenzSpeed {
    EmergencyStop,
    Speed(u8),
}

impl LenzSpeedCommand {
    
    /// Decimal address of the [`LenzSpeedCommand`] (0-127).
    pub fn address(&self) -> u8 {
        self.data[0] & 0b0111_1111
    }

    /// Boolean direction of the [`LenzSpeedCommand`].
    pub fn direction(&self) -> bool {
        (self.data[1] & 0b0010_0000) != 0
    }

    /// Boolean headlight enable of the [`LenzSpeedCommand`].
    pub fn f0(&self) -> bool {
        (self.data[1] & 0b0001_0000) != 0
    }

    /// Returns the packet's speed information. [`LenzSpeed`] differentiates speed and emergency stop. Speed is shifted to the range 0-14.
    pub fn speed(&self) -> LenzSpeed {
        let speed = self.data[1] & 0b0000_1111;
        match speed {
            0 => LenzSpeed::Speed(0),
            1 => LenzSpeed::EmergencyStop,
            _ => LenzSpeed::Speed(speed - 1),
        }
    }
}

/// Packet subtype of Lenz protocol for locomotive functions F1-F4.
pub struct LenzFunctionCommand {
    data: [u8; 2],
}

impl LenzFunctionCommand {

    /// Decimal address of the [`LenzFunctionCommand`] (0-127).
    pub fn address(&self) -> u8 {
        self.data[0] & 0b0111_1111
    }

    /// Boolean array for functions F1-F4. Index 0 = F1.
    pub fn states(&self) -> [bool; 4] {
        let mut states = [false; 4];
        for i in 0..4 {
            states[i] = (self.data[1] & (1u8 << i)) != 0;
        }
        states
    }
}

// TODO: add accessory support
pub struct LenzAccessoryCommand {
    data: [u8; 2],
}

impl LenzAccessoryCommand {

    /// Decimal address of the [`LenzAccessoryCommand`].
    pub fn address(&self) -> u8 {
        0
    }

    /// Provides the accessory port (0..7) and its corresponding state.
    pub fn output(&self) -> (u8, bool) {
        (0, false)
    }

}