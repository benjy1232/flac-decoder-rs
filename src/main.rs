/**
 * Copyright (c) Ben Serrano. All rights reserved.
 * Licensed under the MIT License. See LICENSE in the project root for license information
 */
use core::fmt;
use std::{
    fs,
    io::{self, BufReader, Error, Read},
};

const FLAC_HEADER: &[u8] = "fLaC".as_bytes();

trait Showable {
    fn show_details(&self);
}

#[derive(PartialEq)]
#[repr(u8)]
enum MetadataBlkType {
    Streaminfo = 0,
    Padding,
    Application,
    Seektable,
    VorbisComment,
    Cuesheet,
    Picture,
    Forbidden = 127,
}

impl fmt::Display for MetadataBlkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            MetadataBlkType::Streaminfo => "Streaminfo",
            MetadataBlkType::Padding => "Padding",
            MetadataBlkType::Application => "Application",
            MetadataBlkType::Seektable => "Seektable",
            MetadataBlkType::VorbisComment => "Vorbis Comment",
            MetadataBlkType::Cuesheet => "Cuesheet",
            MetadataBlkType::Picture => "Picture",
            MetadataBlkType::Forbidden => "Forbidden",
        };
        write!(f, "MetadataBlkType: {name}")
    }
}

struct MetadataBlkHdr {
    is_final_block: bool,
    blk_type: MetadataBlkType,
    length: u32,
}

impl Showable for MetadataBlkHdr {
    fn show_details(&self) {
        println!("MetadataBlkHdr:");
        println!("is_final_block: {0}", self.is_final_block);
        println!("blk_type: {0}", self.blk_type);
        println!("length: {0}", self.length);
    }
}

/// IETF Cellar Flac-14 Table 3
struct Streaminfo {
    min_blk_size: u16,
    max_blk_size: u16,
    min_frame_size: u32,     // 24 bits
    max_frame_size: u32,     // 24 bits
    sample_rate: u32,        // 20 bits
    num_channels: u8,        // 3 bits
    bits_per_sample: u8,     // 5 bits
    total_sample_count: u64, // 36 bits,
    md5_checksum: u128,
}

const STREAMINFO_SIZE: usize = 34usize;

impl Streaminfo {
    fn new(raw_data: &[u8]) -> Result<Self, Error> {
        eprintln!("Raw data len: {0}", raw_data.len());
        if raw_data.len() < STREAMINFO_SIZE {
            return Err(Error::new(
                io::ErrorKind::InvalidInput,
                "Not the expected length for a Streaminfo object",
            ));
        }

        let mut offset = 0;
        let mut copy_data_increase_offset = |data_slice: &mut [u8], offset_len: usize| {
            data_slice[..].copy_from_slice(&raw_data[offset..offset + offset_len]);
            offset += offset_len;
        };
        let mut get_blk_size = || {
            let mut raw_blk_size = [0u8; 2];
            copy_data_increase_offset(&mut raw_blk_size, 2);
            return u16::from_be_bytes(raw_blk_size);
        };

        let min_blk_size = get_blk_size();
        let max_blk_size = get_blk_size();

        let mut get_frame_size = || {
            let mut raw_frame_size = [0u8; 4];
            copy_data_increase_offset(&mut raw_frame_size[1..], 3);
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
        let num_channels = u8::try_from(get_sr_nc_bps_tsc_u64(3)).expect("Value not a u8");
        let bits_per_sample = u8::try_from(get_sr_nc_bps_tsc_u64(5)).expect("Value not a u8");
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

impl Showable for Streaminfo {
    fn show_details(&self) {
        println!("Streaminfo:");
        println!("min_blk_size: {0}", self.min_blk_size);
        println!("max_blk_size: {0}", self.max_blk_size);
        println!("min_frame_size: {0}", self.min_frame_size); // 24 bits
        println!("max_frame_size: {0}", self.max_frame_size); // 24 bits
        println!("sample_rate: {0}", self.sample_rate); // 20 bits
        println!("num_channels: {0}", self.num_channels); // 3 bits
        println!("bits_per_sample: {0}", self.bits_per_sample); // 5 bits
        println!("total_sample_count: {0}", self.total_sample_count); // 36 bits,
        println!("md5_checksum: {0:#032x}", self.md5_checksum);
    }
}

fn create_metadata_blk_hdr(metadata_blk: &[u8; 4]) -> Result<MetadataBlkHdr, Error> {
    const METADATA_BLK_HDR_MASK: u8 = 0b1000_0000;
    let is_final_block = (metadata_blk[0] & METADATA_BLK_HDR_MASK) != 0;
    let blk_type = match metadata_blk[0] & !METADATA_BLK_HDR_MASK {
        0 => MetadataBlkType::Streaminfo,
        1 => MetadataBlkType::Padding,
        2 => MetadataBlkType::Application,
        3 => MetadataBlkType::Seektable,
        4 => MetadataBlkType::VorbisComment,
        5 => MetadataBlkType::Cuesheet,
        6 => MetadataBlkType::Picture,
        127 => MetadataBlkType::Forbidden,
        e => {
            return Err(Error::new(
                io::ErrorKind::InvalidData,
                format!("Found unexpected Metadata Block Type {e}"),
            ))
        }
    };

    let length = u32::from_be_bytes(metadata_blk.clone()) ^ u32::from(metadata_blk[0]) << 24;

    Ok(MetadataBlkHdr {
        is_final_block,
        blk_type,
        length,
    })
}

fn is_valid_flac_hdr(flac_hdr: &[u8; 4]) -> bool {
    flac_hdr == FLAC_HEADER
}

fn read_flac_hdr(flac_file: &mut BufReader<fs::File>) -> Result<(), Error> {
    let mut flac_hdr = [0u8; 4];
    match flac_file.read(&mut flac_hdr) {
        Ok(bytes_read) => {
            if bytes_read != 4 {
                return Err(Error::new(
                    io::ErrorKind::InvalidData,
                    "File not large enough for header",
                ));
            }
        }
        Err(e) => return Err(e),
    };

    if !is_valid_flac_hdr(&flac_hdr) {
        return Err(Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Invalid Header, Found: {0:?}, expected {1:?}",
                FLAC_HEADER, flac_hdr
            ),
        ));
    }

    let mut is_final_block = false;
    let mut stream_info: Streaminfo;
    while !is_final_block {
        let mut raw_metadata_blk_hdr = [0u8; 4];
        match flac_file.read(&mut raw_metadata_blk_hdr) {
            Ok(bytes_read) => {
                if bytes_read != 4 {
                    return Err(Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid Metadata Block",
                    ));
                }
            }
            Err(e) => return Err(e),
        }

        let metadata_blk_hdr = match create_metadata_blk_hdr(&raw_metadata_blk_hdr) {
            Ok(mdblh) => mdblh,
            Err(e) => return Err(e),
        };
        metadata_blk_hdr.show_details();

        let mut raw_metadata = Vec::with_capacity(metadata_blk_hdr.length as usize);
        for _ in 0..(metadata_blk_hdr.length) {
            raw_metadata.push(0u8);
        }

        let _ = flac_file.read(&mut raw_metadata);

        if metadata_blk_hdr.blk_type == MetadataBlkType::Streaminfo {
            stream_info = match Streaminfo::new(raw_metadata.as_slice()) {
                Ok(si) => si,
                Err(e) => return Err(e),
            };
            stream_info.show_details();
        }
        is_final_block = metadata_blk_hdr.is_final_block;
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    let file_name = "./flac-specification/example_1.flac";
    let flac_file = match fs::File::open(file_name) {
        Ok(flac_file) => flac_file,
        Err(e) => return Err(e),
    };

    let mut flac_file = BufReader::new(flac_file);
    let hdr_result = read_flac_hdr(&mut flac_file);

    if hdr_result.is_err() {
        return hdr_result;
    }
    return Ok(());
}
