// decode yamaha adpcm-b
// adapted to rust from https://github.com/superctr/adpcm/blob/master/ymb_codec.c (Unlicense)

use alloc::vec::Vec;

static STEP_TABLE: [u8; 8] = [57, 57, 57, 57, 77, 102, 128, 153];

struct DecodeContext {
    history: i16,
    step_size: u16,
}

fn ymb_step(step: u8, context: &mut DecodeContext) -> i16 {
    let sign = step & 8;
    let delta = step & 7;
    let diff = ((1 + ((delta as u32) << 1)) * (context.step_size as u32)) >> 3;
    let mut newval = context.history as i32;
    let nstep = (((STEP_TABLE[delta as usize] as u32) * (context.step_size as u32)) >> 6) as u16;
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

    for i in data {
        let mut nibble = 0;
        loop {
            let mut step = i << nibble;
            step >>= 4;

            nibble ^= 4;
            result.push(ymb_step(step, &mut context));

            if nibble == 0 {
                break;
            }
        }
    }

    result
}
