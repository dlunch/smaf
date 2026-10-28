// Decode Yamaha ADPCM used by SMAF.
// adapted to rust from https://github.com/superctr/adpcm/blob/master/ymb_codec.c (Unlicense)

use alloc::vec::Vec;

static STEP_TABLE: [u16; 8] = [230, 230, 230, 230, 307, 409, 512, 614];

struct DecodeContext {
    history: i16,
    step_size: u16,
}

fn decode_nibble(step: u8, context: &mut DecodeContext) -> i16 {
    let sign = step & 8;
    let delta = step & 7;
    let diff = ((1 + ((delta as u32) << 1)) * (context.step_size as u32)) >> 3;
    let mut newval = context.history as i32;
    let nstep = (((STEP_TABLE[delta as usize] as u32) * (context.step_size as u32)) >> 8) as u16;
    if sign > 0 {
        newval -= diff as i32;
    } else {
        newval += diff as i32;
    }
    context.step_size = u16::clamp(nstep, 127, 24576);
    newval = i32::clamp(newval, -32768, 32767);
    context.history = newval as i16;

    newval as i16
}

pub fn decode_adpcm(data: &[u8]) -> Vec<i16> {
    let mut result = Vec::new();
    let mut context = DecodeContext { history: 0, step_size: 127 };

    for byte in data {
        result.push(decode_nibble(byte & 0x0f, &mut context));
        result.push(decode_nibble(byte >> 4, &mut context));
    }

    result
}
