#[repr(u8)]
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum Channel {
    Mono = 0,
    Stereo = 1,
}

impl From<u8> for Channel {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Mono,
            1 => Self::Stereo,
            _ => panic!("Invalid channel value"),
        }
    }
}

#[repr(u8)]
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum Format {
    TwosComplementPCM = 0,
    OffsetBinaryPCM = 1,
    YamahaADPCM = 2,
}

impl From<u8> for Format {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::TwosComplementPCM,
            1 => Self::OffsetBinaryPCM,
            2 => Self::YamahaADPCM,
            _ => panic!("Invalid format value"),
        }
    }
}

#[repr(u8)]
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum BaseBit {
    Bit4 = 0,
    Bit8 = 1,
    Bit12 = 2,
    Bit16 = 3,
}

impl From<u8> for BaseBit {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Bit4,
            1 => Self::Bit8,
            2 => Self::Bit12,
            3 => Self::Bit16,
            _ => panic!("Invalid base bit value"),
        }
    }
}
