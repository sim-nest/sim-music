//! Standard MIDI controller-number constants for use as the `cc` field of
//! [`ChannelMessage::ControlChange`](crate::ChannelMessage::ControlChange).

use crate::U7;

/// Bank Select (MSB), controller 0.
pub const CC_BANK_SELECT_MSB: U7 = U7(0);
/// Modulation Wheel (MSB), controller 1.
pub const CC_MOD_WHEEL_MSB: U7 = U7(1);
/// Breath Controller (MSB), controller 2.
pub const CC_BREATH_MSB: U7 = U7(2);
/// Foot Controller (MSB), controller 4.
pub const CC_FOOT_MSB: U7 = U7(4);
/// Portamento Time (MSB), controller 5.
pub const CC_PORTAMENTO_TIME_MSB: U7 = U7(5);
/// Data Entry (MSB), controller 6.
pub const CC_DATA_ENTRY_MSB: U7 = U7(6);
/// Channel Volume (MSB), controller 7.
pub const CC_VOLUME_MSB: U7 = U7(7);
/// Balance (MSB), controller 8.
pub const CC_BALANCE_MSB: U7 = U7(8);
/// Pan (MSB), controller 10.
pub const CC_PAN_MSB: U7 = U7(10);
/// Expression Controller (MSB), controller 11.
pub const CC_EXPRESSION_MSB: U7 = U7(11);
/// Sustain (damper) Pedal, controller 64.
pub const CC_SUSTAIN_PEDAL: U7 = U7(64);
/// Portamento On/Off switch, controller 65.
pub const CC_PORTAMENTO_SWITCH: U7 = U7(65);
/// Sostenuto Pedal, controller 66.
pub const CC_SOSTENUTO: U7 = U7(66);
/// Soft Pedal, controller 67.
pub const CC_SOFT_PEDAL: U7 = U7(67);
/// All Sound Off (channel mode), controller 120.
pub const CC_ALL_SOUND_OFF: U7 = U7(120);
/// Reset All Controllers (channel mode), controller 121.
pub const CC_RESET_ALL_CONTROLLERS: U7 = U7(121);
/// Local Control On/Off (channel mode), controller 122.
pub const CC_LOCAL_CONTROL: U7 = U7(122);
/// All Notes Off (channel mode), controller 123.
pub const CC_ALL_NOTES_OFF: U7 = U7(123);
