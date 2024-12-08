use core::fmt;
use std::io::{self, Error};

#[derive(PartialEq)]
#[repr(u8)]
pub enum Type {
    Streaminfo = 0,
    Padding,
    Application,
    Seektable,
    VorbisComment,
    Cuesheet,
    Picture,
    Forbidden = 127,
}

pub struct Header {
    pub is_final_block: bool,
    pub blk_type: Type,
    pub length: u32,
}

/// IETF Cellar Flac-14 Table 3
pub struct Streaminfo {
    pub min_blk_size: u32,
    pub max_blk_size: u32,
    pub min_frame_size: u32,     // 24 bits
    pub max_frame_size: u32,     // 24 bits
    pub sample_rate: u32,        // 20 bits
    pub num_channels: u32,       // 3 bits
    pub bits_per_sample: u32,    // 5 bits
    pub total_sample_count: u64, // 36 bits,
    pub md5_checksum: u128,
}

const STREAMINFO_BLK_BIT_SIZE: usize = 16usize;
const STREAMINFO_FRAME_BIT_SIZE: usize = 24usize;
const STREAMINFO_SAMPLE_RATE_BIT_SIZE: usize = 20usize;
const STREAMINFO_NUM_CHANNELS_BIT_SIZE: usize = 3usize;
const STREAMINFO_BITS_PER_SAMPLE_BIT_SIZE: usize = 5usize;
const STREAMINFO_TOTAL_SAMPLE_COUNT_BIT_SIZE: usize = 36usize;
const STREAMINFO_MD5_CHECKSUM_BIT_SIZE: usize = 128usize;
const STREAMINFO_SIZE_BITS: usize = STREAMINFO_BLK_BIT_SIZE * 2
    + STREAMINFO_FRAME_BIT_SIZE * 2
    + STREAMINFO_SAMPLE_RATE_BIT_SIZE
    + STREAMINFO_NUM_CHANNELS_BIT_SIZE
    + STREAMINFO_BITS_PER_SAMPLE_BIT_SIZE
    + STREAMINFO_TOTAL_SAMPLE_COUNT_BIT_SIZE
    + STREAMINFO_MD5_CHECKSUM_BIT_SIZE;

const STREAMINFO_SIZE: usize = STREAMINFO_SIZE_BITS / 8;

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Type::Streaminfo => "Streaminfo",
            Type::Padding => "Padding",
            Type::Application => "Application",
            Type::Seektable => "Seektable",
            Type::VorbisComment => "Vorbis Comment",
            Type::Cuesheet => "Cuesheet",
            Type::Picture => "Picture",
            Type::Forbidden => "Forbidden",
        };
        write!(f, "MetadataBlkType: {name}")
    }
}

impl Header {
    pub fn new(raw_header: &[u8; 4]) -> Result<Self, Error> {
        const METADATA_BLK_HDR_MASK: u8 = 0b1000_0000;
        let is_final_block = (raw_header[0] & METADATA_BLK_HDR_MASK) != 0;
        let blk_type = match raw_header[0] & !METADATA_BLK_HDR_MASK {
            0 => Type::Streaminfo,
            1 => Type::Padding,
            2 => Type::Application,
            3 => Type::Seektable,
            4 => Type::VorbisComment,
            5 => Type::Cuesheet,
            6 => Type::Picture,
            127 => Type::Forbidden,
            e => {
                return Err(Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Found unexpected Metadata Block Type {e}"),
                ))
            }
        };

        let length = u32::from_be_bytes(raw_header.clone()) ^ u32::from(raw_header[0]) << 24;

        Ok(Header {
            is_final_block,
            blk_type,
            length,
        })
    }
}

impl Streaminfo {
    pub fn new(raw_streaminfo: &[u8]) -> Result<Self, Error> {
        eprintln!("Raw data len: {0}", raw_streaminfo.len());
        if raw_streaminfo.len() < STREAMINFO_SIZE {
            return Err(Error::new(
                io::ErrorKind::InvalidInput,
                "Not the expected length for a Streaminfo object",
            ));
        }

        let mut offset = 0;
        let mut copy_data_increase_offset = |data_slice: &mut [u8], offset_len: usize| {
            data_slice[..].copy_from_slice(&raw_streaminfo[offset..offset + offset_len]);
            offset += offset_len;
        };
        let mut get_blk_size = || {
            let mut raw_blk_size = [0u8; 4];
            copy_data_increase_offset(&mut raw_blk_size[2..], STREAMINFO_BLK_BIT_SIZE);
            return u32::from_be_bytes(raw_blk_size);
        };

        let min_blk_size = get_blk_size();
        let max_blk_size = get_blk_size();

        let mut get_frame_size = || {
            let mut raw_frame_size = [0u8; 4];
            copy_data_increase_offset(&mut raw_frame_size[1..], STREAMINFO_FRAME_BIT_SIZE);
            return u32::from_be_bytes(raw_frame_size);
        };

        let min_frame_size = get_frame_size();
        let max_frame_size = get_frame_size();

        let mut raw_sr_nc_bps_tsc = [0u8; 8];
        copy_data_increase_offset(&mut raw_sr_nc_bps_tsc, 8);

        let raw_sr_nc_bps_tsc = u64::from_be_bytes(raw_sr_nc_bps_tsc);
        let mut ignore_mask = 0u64;
        let mut remaining_len = 64;
        let mut get_sr_nc_bps_tsc_u64 = |num_bits_to_read: u32| {
            if num_bits_to_read > remaining_len {
                return 0;
            }

            ignore_mask <<= num_bits_to_read;
            remaining_len -= num_bits_to_read;
            let ret = (raw_sr_nc_bps_tsc >> remaining_len) ^ ignore_mask;
            ignore_mask = ignore_mask | ret;
            ret
        };

        let sample_rate = u32::try_from(get_sr_nc_bps_tsc_u64(20)).expect("Value not a u32");
        let num_channels = u32::try_from(get_sr_nc_bps_tsc_u64(3)).expect("Value not a u8");
        let bits_per_sample = u32::try_from(get_sr_nc_bps_tsc_u64(5)).expect("Value not a u8");
        let total_sample_count = get_sr_nc_bps_tsc_u64(36);

        let mut raw_md5_checksum = [0u8; 16];
        copy_data_increase_offset(&mut raw_md5_checksum, 16);
        let md5_checksum = u128::from_be_bytes(raw_md5_checksum);

        return Ok(Streaminfo {
            min_blk_size,
            max_blk_size,
            min_frame_size,
            max_frame_size,
            sample_rate,
            num_channels,
            bits_per_sample,
            total_sample_count,
            md5_checksum,
        });
    }
}
