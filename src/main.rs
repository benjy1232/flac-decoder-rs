/**
 * Copyright (c) Ben Serrano. All rights reserved.
 * Licensed under the MIT License. See LICENSE in the project root for license information
 */
pub mod metadata;
pub mod showable;

use std::{
    fs,
    io::{self, BufReader, Error, Read},
};

use showable::Showable;

const FLAC_HEADER: &[u8] = "fLaC".as_bytes();

impl showable::Showable for metadata::Header {
    fn show_details(&self) {
        println!("MetadataBlkHdr:");
        println!("is_final_block: {0}", self.is_final_block);
        println!("blk_type: {0}", self.blk_type);
        println!("length: {0}", self.length);
    }
}

impl showable::Showable for metadata::Streaminfo {
    fn show_details(&self) {
        println!("Streaminfo:");
        println!("min_blk_size: {0}", self.min_blk_size);
        println!("max_blk_size: {0}", self.max_blk_size);
        println!("min_frame_size: {0}", self.min_frame_size);
        println!("max_frame_size: {0}", self.max_frame_size);
        println!("sample_rate: {0}", self.sample_rate);
        println!("num_channels: {0}", self.num_channels);
        println!("bits_per_sample: {0}", self.bits_per_sample);
        println!("total_sample_count: {0}", self.total_sample_count);
        println!("md5_checksum: {0:#032x}", self.md5_checksum);
        println!();
    }
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
    let mut stream_info: metadata::Streaminfo;
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

        let metadata_blk_hdr = match metadata::Header::new(&raw_metadata_blk_hdr) {
            Ok(mdblh) => mdblh,
            Err(e) => return Err(e),
        };
        metadata_blk_hdr.show_details();

        let mut raw_metadata = Vec::with_capacity(metadata_blk_hdr.length as usize);
        for _ in 0..(metadata_blk_hdr.length) {
            raw_metadata.push(0u8);
        }

        let _ = flac_file.read(&mut raw_metadata);

        if metadata_blk_hdr.blk_type == metadata::Type::Streaminfo {
            stream_info = match metadata::Streaminfo::new(raw_metadata.as_slice()) {
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
