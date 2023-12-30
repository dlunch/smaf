use nom_derive::NomBE;

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
pub enum StreamWaveFormat {
    TwosComplementPCM = 0,
    OffsetBinaryPCM = 1,
    YamahaADPCM = 2,
}

impl From<u8> for StreamWaveFormat {
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
pub enum PcmWaveFormat {
    TwosComplementPCM = 0,
    Adpcm = 1,
    TwinVQ = 2,
    MP3 = 3,
}

impl From<u8> for PcmWaveFormat {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::TwosComplementPCM,
            1 => Self::Adpcm,
            2 => Self::TwinVQ,
            3 => Self::MP3,
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

#[repr(u8)]
#[derive(NomBE, Eq, PartialEq, Copy, Clone, Debug)]
pub enum FormatType {
    HandyPhoneStandard = 0,
    MobileStandardCompress = 1,
    MobileStandardNoCompress = 2,
}
