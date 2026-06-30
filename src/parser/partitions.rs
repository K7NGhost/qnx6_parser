use std::{fmt, io::Cursor};

const SECTOR_SIZE: usize = 512;
const GPT_PARTITION_TYPE_GUID_SIZE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionTableKind {
    Mbr,
    Gpt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionTable {
    pub kind: PartitionTableKind,
    pub partitions: Vec<Partition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub index: usize,
    pub partition_type: PartitionType,
    pub first_lba: u64,
    pub sector_count: u64,
    pub byte_offset: u64,
    pub byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionType {
    Mbr(u8),
    Gpt([u8; GPT_PARTITION_TYPE_GUID_SIZE]),
}

impl fmt::Display for PartitionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mbr(partition_type) => write!(formatter, "MBR type 0x{partition_type:02X}"),
            Self::Gpt(guid) => write!(
                formatter,
                "GPT type {:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                guid[3],
                guid[2],
                guid[1],
                guid[0],
                guid[5],
                guid[4],
                guid[7],
                guid[6],
                guid[8],
                guid[9],
                guid[10],
                guid[11],
                guid[12],
                guid[13],
                guid[14],
                guid[15],
            ),
        }
    }
}

pub fn get_all_partitions(image: &[u8]) -> Result<PartitionTable, String> {
    if let Ok(partition_table) = parse_gpt_partitions(image) {
        return Ok(partition_table);
    }

    parse_mbr_partitions(image)
}

pub fn is_filesystem_partition(partition: &Partition) -> bool {
    match partition.partition_type {
        PartitionType::Mbr(partition_type) => !is_extended_mbr_partition_type(partition_type),
        PartitionType::Gpt(partition_type_guid) => {
            partition_type_guid != [0; GPT_PARTITION_TYPE_GUID_SIZE]
        }
    }
}

fn parse_mbr_partitions(image: &[u8]) -> Result<PartitionTable, String> {
    let mut cursor = Cursor::new(image);
    let mbr = mbrman::MBR::read_from(&mut cursor, SECTOR_SIZE as u32)
        .map_err(|error| format!("failed to parse MBR partition table: {error}"))?;

    let partitions = mbr
        .iter()
        .filter(|(_, partition)| partition.is_used())
        .map(|(index, partition)| {
            build_partition(
                index - 1,
                PartitionType::Mbr(partition.sys),
                partition.starting_lba as u64,
                partition.sectors as u64,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PartitionTable {
        kind: PartitionTableKind::Mbr,
        partitions,
    })
}

fn parse_gpt_partitions(image: &[u8]) -> Result<PartitionTable, String> {
    let cursor = Cursor::new(image.to_vec());
    let disk = gpt::GptConfig::new()
        .writable(false)
        .logical_block_size(gpt::disk::LogicalBlockSize::Lb512)
        .open_from_device(cursor)
        .map_err(|error| format!("failed to parse GPT partition table: {error}"))?;

    let partitions = disk
        .partitions()
        .iter()
        .filter(|(_, partition)| partition.is_used())
        .map(|(index, partition)| {
            let partition_type_guid = gpt_type_guid_to_partition_bytes(partition);
            let sector_count = partition
                .sectors_len()
                .map_err(|error| format!("failed to calculate GPT partition length: {error}"))?;

            build_partition(
                (*index as usize).saturating_sub(1),
                PartitionType::Gpt(partition_type_guid),
                partition.first_lba,
                sector_count,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PartitionTable {
        kind: PartitionTableKind::Gpt,
        partitions,
    })
}

fn build_partition(
    index: usize,
    partition_type: PartitionType,
    first_lba: u64,
    sector_count: u64,
) -> Result<Partition, String> {
    let byte_offset = first_lba
        .checked_mul(SECTOR_SIZE as u64)
        .ok_or_else(|| format!("partition {index} byte offset overflowed"))?;
    let byte_len = sector_count
        .checked_mul(SECTOR_SIZE as u64)
        .ok_or_else(|| format!("partition {index} byte length overflowed"))?;

    Ok(Partition {
        index,
        partition_type,
        first_lba,
        sector_count,
        byte_offset,
        byte_len,
    })
}

fn is_extended_mbr_partition_type(partition_type: u8) -> bool {
    matches!(partition_type, 0x05 | 0x0F | 0x85)
}

fn gpt_type_guid_to_partition_bytes(partition: &gpt::partition::Partition) -> [u8; 16] {
    let (field_1, field_2, field_3, field_4) = partition.part_type_guid.guid.as_fields();
    let mut bytes = [0; GPT_PARTITION_TYPE_GUID_SIZE];

    bytes[0..4].copy_from_slice(&field_1.to_le_bytes());
    bytes[4..6].copy_from_slice(&field_2.to_le_bytes());
    bytes[6..8].copy_from_slice(&field_3.to_le_bytes());
    bytes[8..16].copy_from_slice(field_4);

    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const MBR_SIGNATURE_OFFSET: usize = 510;
    const MBR_SIGNATURE: [u8; 2] = [0x55, 0xAA];
    const MBR_PARTITION_TABLE_OFFSET: usize = 446;
    const MBR_PARTITION_ENTRY_SIZE: usize = 16;

    #[test]
    fn detects_mbr_from_signature() {
        let mut image = vec![0; SECTOR_SIZE * 2];
        image[MBR_SIGNATURE_OFFSET..MBR_SIGNATURE_OFFSET + MBR_SIGNATURE.len()]
            .copy_from_slice(&MBR_SIGNATURE);
        write_mbr_entry(&mut image, MBR_PARTITION_TABLE_OFFSET, 0x4F, 2048, 4096);

        let partition_table = get_all_partitions(&image).expect("MBR should parse");

        assert_eq!(partition_table.kind, PartitionTableKind::Mbr);
        assert_eq!(
            partition_table.partitions,
            vec![Partition {
                index: 0,
                partition_type: PartitionType::Mbr(0x4F),
                first_lba: 2048,
                sector_count: 4096,
                byte_offset: 2048 * SECTOR_SIZE as u64,
                byte_len: 4096 * SECTOR_SIZE as u64,
            }]
        );
    }

    #[test]
    fn detects_gpt_from_header() {
        let total_bytes = 1024 * 1024;
        let mut image = Cursor::new(vec![0; total_bytes]);
        let protective_mbr =
            gpt::mbr::ProtectiveMBR::with_lb_size(((total_bytes / SECTOR_SIZE) - 1) as u32);
        protective_mbr
            .overwrite_lba0(&mut image)
            .expect("protective MBR should be writable");

        let mut disk = gpt::GptConfig::new()
            .writable(true)
            .logical_block_size(gpt::disk::LogicalBlockSize::Lb512)
            .create_from_device(image, None)
            .expect("GPT should be creatable");
        disk.add_partition(
            "qnx6",
            66 * SECTOR_SIZE as u64,
            gpt::partition_types::BASIC,
            0,
            None,
        )
        .expect("GPT partition should be addable");
        let image = disk.write().expect("GPT should be writable").into_inner();

        let partition_table = get_all_partitions(&image).expect("GPT should parse");

        assert_eq!(partition_table.kind, PartitionTableKind::Gpt);
        assert_eq!(partition_table.partitions.len(), 1);
        assert_eq!(partition_table.partitions[0].index, 0);
        assert_eq!(partition_table.partitions[0].sector_count, 66);
        assert_eq!(
            partition_table.partitions[0].byte_len,
            66 * SECTOR_SIZE as u64
        );
        assert!(matches!(
            partition_table.partitions[0].partition_type,
            PartitionType::Gpt(_)
        ));
    }

    #[test]
    fn detects_ebr_logical_partitions() {
        let mut image = vec![0; SECTOR_SIZE * 100];
        image[MBR_SIGNATURE_OFFSET..MBR_SIGNATURE_OFFSET + MBR_SIGNATURE.len()]
            .copy_from_slice(&MBR_SIGNATURE);

        write_mbr_entry(&mut image, MBR_PARTITION_TABLE_OFFSET, 0x0F, 10, 80);

        let first_ebr_offset = SECTOR_SIZE * 10;
        image[first_ebr_offset + MBR_SIGNATURE_OFFSET
            ..first_ebr_offset + MBR_SIGNATURE_OFFSET + MBR_SIGNATURE.len()]
            .copy_from_slice(&MBR_SIGNATURE);
        write_mbr_entry(
            &mut image,
            first_ebr_offset + MBR_PARTITION_TABLE_OFFSET,
            0x4F,
            1,
            20,
        );
        write_mbr_entry(
            &mut image,
            first_ebr_offset + MBR_PARTITION_TABLE_OFFSET + MBR_PARTITION_ENTRY_SIZE,
            0x05,
            50,
            30,
        );

        let second_ebr_offset = SECTOR_SIZE * 60;
        image[second_ebr_offset + MBR_SIGNATURE_OFFSET
            ..second_ebr_offset + MBR_SIGNATURE_OFFSET + MBR_SIGNATURE.len()]
            .copy_from_slice(&MBR_SIGNATURE);
        write_mbr_entry(
            &mut image,
            second_ebr_offset + MBR_PARTITION_TABLE_OFFSET,
            0x4F,
            2,
            30,
        );

        let partition_table = get_all_partitions(&image).expect("EBR chain should parse");

        assert_eq!(partition_table.kind, PartitionTableKind::Mbr);
        assert_eq!(
            partition_table.partitions,
            vec![
                Partition {
                    index: 0,
                    partition_type: PartitionType::Mbr(0x0F),
                    first_lba: 10,
                    sector_count: 80,
                    byte_offset: 10 * SECTOR_SIZE as u64,
                    byte_len: 80 * SECTOR_SIZE as u64,
                },
                Partition {
                    index: 4,
                    partition_type: PartitionType::Mbr(0x4F),
                    first_lba: 11,
                    sector_count: 20,
                    byte_offset: 11 * SECTOR_SIZE as u64,
                    byte_len: 20 * SECTOR_SIZE as u64,
                },
                Partition {
                    index: 5,
                    partition_type: PartitionType::Mbr(0x4F),
                    first_lba: 62,
                    sector_count: 30,
                    byte_offset: 62 * SECTOR_SIZE as u64,
                    byte_len: 30 * SECTOR_SIZE as u64,
                },
            ]
        );
    }

    #[test]
    fn rejects_unknown_partition_table() {
        let image = vec![0; SECTOR_SIZE * 2];

        assert!(get_all_partitions(&image).is_err());
    }

    pub fn write_mbr_entry(
        image: &mut [u8],
        offset: usize,
        partition_type: u8,
        first_lba: u32,
        sector_count: u32,
    ) {
        image[offset + 4] = partition_type;
        image[offset + 8..offset + 12].copy_from_slice(&first_lba.to_le_bytes());
        image[offset + 12..offset + 16].copy_from_slice(&sector_count.to_le_bytes());
    }
}
