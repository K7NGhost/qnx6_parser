use std::{fs::File, path::Path};

use memmap2::Mmap;

use crate::parser::partitions::{self, Partition, PartitionTable, is_filesystem_partition};

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

    println!("  after: filesystem=true");
    println!("  action: parsing partition{}", partition.index);
    println!("  bytes_available: {}", partition_bytes.len());
    println!();

    Ok(())
}
