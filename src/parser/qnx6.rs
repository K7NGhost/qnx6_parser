use std::{fs::File, path::Path};

use memmap2::Mmap;

use crate::parser::partitions::{self, Partition, PartitionTable, is_filesystem_partition};
use crate::parser::superblock::{SUPERBLOCK_SIZE, SuperBlock};

pub fn parse_qnx6(image_path: &Path) -> Result<PartitionTable, String> {
    let file = File::open(image_path).map_err(|error| {
        format!(
            "failed to open disk image {}: {error}",
            image_path.display()
        )
    })?;

    let image = unsafe {
        Mmap::map(&file).map_err(|error| {
            format!(
                "failed to memory-map disk image {}: {error}",
                image_path.display()
            )
        })?
    };

    let partition_table = get_all_partitions(&image)?;

    for partition in &partition_table.partitions {
        parse_partition(&image, partition)?;
    }

    Ok(partition_table)
}

fn get_all_partitions(image: &[u8]) -> Result<PartitionTable, String> {
    partitions::get_all_partitions(image)
}

fn parse_partition(image: &[u8], partition: &Partition) -> Result<(), String> {
    if !is_filesystem_partition(partition) {
        return Ok(());
    }

    println!("partition {}", partition.index);
    let start_sector = partition.first_lba;
    let offset_into_partition = 16;
    let target_sector = start_sector
        .checked_add(offset_into_partition)
        .ok_or_else(|| format!("partition {} superblock sector overflowed", partition.index))?;
    let superblock_offset = target_sector
        .checked_mul(512)
        .ok_or_else(|| format!("partition {} superblock offset overflowed", partition.index))?;
    let superblock_start = usize::try_from(superblock_offset).map_err(|_| {
        format!(
            "partition {} superblock offset is too large: {}",
            partition.index, superblock_offset
        )
    })?;
    let superblock_end = superblock_start
        .checked_add(SUPERBLOCK_SIZE)
        .ok_or_else(|| format!("partition {} superblock range overflowed", partition.index))?;

    println!(
        "  before: type={}, first_lba={}, sectors={}, byte_offset={}, byte_len={}",
        partition.partition_type,
        partition.first_lba,
        partition.sector_count,
        partition.byte_offset,
        partition.byte_len
    );

    let start = usize::try_from(partition.byte_offset).map_err(|_| {
        format!(
            "partition {} byte offset is too large: {}",
            partition.index, partition.byte_offset
        )
    })?;
    let len = usize::try_from(partition.byte_len).map_err(|_| {
        format!(
            "partition {} byte length is too large: {}",
            partition.index, partition.byte_len
        )
    })?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| format!("partition {} byte range overflowed", partition.index))?;
    let partition_bytes = image
        .get(start..end)
        .ok_or_else(|| format!("partition {} extends past end of image", partition.index))?;
    let superblock_bytes = image.get(superblock_start..superblock_end).ok_or_else(|| {
        format!(
            "partition {} superblock extends past end of image",
            partition.index
        )
    })?;
    let superblock = SuperBlock::parse(superblock_bytes)?;

    println!("  after: filesystem=true");
    println!("  action: parsing partition{}", partition.index);
    println!("  superblock_offset: {}", superblock_offset);
    println!("  superblock: {}", superblock);
    println!("  bytes_available: {}", partition_bytes.len());
    println!();

    Ok(())
}
