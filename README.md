# lenz-decoder

### A no-std Rust crate for decoding Lenz (pre-DCC) protocol.

## State Machine Usage

A state machine can be created using `LenzMachine::new();`; this can be initialised as `static`.

The state machine must be fed a series of pulse durations representing track data using `advance(pulse: u16)`, where pulse is a unit of track data. The machine is "polarity insensitive"; data must be a contiguous series of high and low pulse durations. The unit for these pulses is microseconds.

All pulse durations have a tolerance of ±5 when being resolved. If your transduced data has some shift from nominal values, this should be corrected for outside of the state machine. The tolerance does not account for transducer issues.

## Example Usage

Typical usage involves retrieving pulses from some buffer. These can be pulse durations captured using a dual-edge sensitive timer in capture mode. Pulses are retrieved and then passed through one (or more) state machines which may produce a packet. See below:

```Rust
static mut LENZ_MACHINE: LenzMachine = LenzMachine::new();
const ADDRESS: u8 = 1;

loop {

    // retrieve pulses from some buffer
    if let Ok(pulse) = pulse_cons.get() {

        // processing Lenz protocol
        if let Some(packet) = LENZ_MACHINE.advance(pulse) {
            match packet.get_type() {
                Some(LenzCommand::Speed(s)) if s.address() == ADDRESS => {
                    // use loco data
                }
                Some(LenzCommand::Function(f)) if f.address() == ADDRESS => {
                    // use function data
                }
                _ => {}
            }
        }
    }
}
```

## Disorientation

In the case that your calling code loses track of the contiguous pulses, you can parse any invalid value for the state machines to reset. The easiest and most consistent is `0`; this will always be invalid. Parsing `0` is a good way to handle timer overcapture errors.