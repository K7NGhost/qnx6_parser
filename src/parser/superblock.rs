use std::fmt;

pub const SUPERBLOCK_SIZE: usize = 1000;
const SUPERBLOCK_HEADER_SIZE: usize = 72;
const ROOT_NODE_SIZE: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuperBlock {
    pub magic: u32,
    pub checksum: u32,
    pub serial: u64,
    pub c_time: u32,
    pub a_time: u32,
    pub flags: u32,
    pub version1: u16,
    pub version2: u16,
    pub volumeid: String,
    pub block_size: u32,
    pub num_of_inodes: u32,
    pub free_inodes: u32,
    pub num_of_blocks: u32,
    pub free_blocks: u32,
    pub alloc_groups: u32,
    pub root_node_inode: RootNode,
    pub root_node_bitmap: RootNode,
    pub root_node_longfilename: RootNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootNode {
    pub raw: [u8; ROOT_NODE_SIZE],
}

impl SuperBlock {
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        if data.len() < SUPERBLOCK_HEADER_SIZE + ROOT_NODE_SIZE * 3 {
            return Err(format!(
                "superblock data is too small: expected at least {} bytes, got {}",
                SUPERBLOCK_HEADER_SIZE + ROOT_NODE_SIZE * 3,
                data.len()
            ));
        }

        let volumeid_raw = read_array::<16>(data, 32)?;

        Ok(Self {
            magic: read_u32_le(data, 0)?,
            checksum: read_u32_le(data, 4)?,
            serial: read_u64_le(data, 8)?,
            c_time: read_u32_le(data, 16)?,
            a_time: read_u32_le(data, 20)?,
            flags: read_u32_le(data, 24)?,
            version1: read_u16_le(data, 28)?,
            version2: read_u16_le(data, 30)?,
            volumeid: format_uuid(volumeid_raw),
            block_size: read_u32_le(data, 48)?,
            num_of_inodes: read_u32_le(data, 52)?,
            free_inodes: read_u32_le(data, 56)?,
            num_of_blocks: read_u32_le(data, 60)?,
            free_blocks: read_u32_le(data, 64)?,
            alloc_groups: read_u32_le(data, 68)?,
            root_node_inode: RootNode::parse(&data[72..152])?,
            root_node_bitmap: RootNode::parse(&data[152..232])?,
            root_node_longfilename: RootNode::parse(&data[232..312])?,
        })
    }
}

impl fmt::Display for SuperBlock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "<SuperBlock magic=0x{:X}, volumeid={}, serial={}, block_size={}, inodes={}, blocks={}>",
            self.magic,
            self.volumeid,
            self.serial,
            self.block_size,
            self.num_of_inodes,
            self.num_of_blocks
        )
    }
}

impl RootNode {
    fn parse(data: &[u8]) -> Result<Self, String> {
        Ok(Self {
            raw: data
                .try_into()
                .map_err(|_| format!("root node must be {ROOT_NODE_SIZE} bytes"))?,
        })
    }
}

fn read_u16_le(data: &[u8], offset: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(read_array(data, offset)?))
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(read_array(data, offset)?))
}

fn read_u64_le(data: &[u8], offset: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(read_array(data, offset)?))
}

fn read_array<const N: usize>(data: &[u8], offset: usize) -> Result<[u8; N], String> {
    data.get(offset..offset + N)
        .ok_or_else(|| format!("expected {N} bytes at superblock offset {offset:#x}"))?
        .try_into()
        .map_err(|_| format!("failed to read {N} bytes at superblock offset {offset:#x}"))
}

fn format_uuid(bytes: [u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_superblock_header() {
        let mut data = vec![0; SUPERBLOCK_SIZE];
        data[0..4].copy_from_slice(&0x6819_1122_u32.to_le_bytes());
        data[4..8].copy_from_slice(&0xAABB_CCDD_u32.to_le_bytes());
        data[8..16].copy_from_slice(&7_u64.to_le_bytes());
        data[16..20].copy_from_slice(&1_u32.to_le_bytes());
        data[20..24].copy_from_slice(&2_u32.to_le_bytes());
        data[24..28].copy_from_slice(&3_u32.to_le_bytes());
        data[28..30].copy_from_slice(&4_u16.to_le_bytes());
        data[30..32].copy_from_slice(&5_u16.to_le_bytes());
        data[32..48].copy_from_slice(&[
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ]);
        data[48..52].copy_from_slice(&4096_u32.to_le_bytes());
        data[52..56].copy_from_slice(&10_u32.to_le_bytes());
        data[56..60].copy_from_slice(&8_u32.to_le_bytes());
        data[60..64].copy_from_slice(&20_u32.to_le_bytes());
        data[64..68].copy_from_slice(&18_u32.to_le_bytes());
        data[68..72].copy_from_slice(&2_u32.to_le_bytes());

        let superblock = SuperBlock::parse(&data).expect("superblock should parse");

        assert_eq!(superblock.magic, 0x6819_1122);
        assert_eq!(superblock.volumeid, "00010203-0405-0607-0809-0a0b0c0d0e0f");
        assert_eq!(superblock.block_size, 4096);
        assert_eq!(superblock.num_of_inodes, 10);
        assert_eq!(superblock.num_of_blocks, 20);
    }
}
