use std;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use message_types::{SrdMessage, srd_msg_id::SRD_INITIATE_MSG_ID, SRD_SIGNATURE};
use Result;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SrdInitiate {
    pub signature: u32,
    pub packet_type: u8,
    pub seq_num: u8,
    pub flags: u16,
    pub key_size: u16,
    pub reserved: u16,
}

impl SrdMessage for SrdInitiate {
    fn read_from(buffer: &mut std::io::Cursor<Vec<u8>>) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(SrdInitiate {
            signature: buffer.read_u32::<LittleEndian>()?,
            packet_type: buffer.read_u8()?,
            seq_num: buffer.read_u8()?,
            flags: buffer.read_u16::<LittleEndian>()?,
            key_size: buffer.read_u16::<LittleEndian>()?,
            reserved: buffer.read_u16::<LittleEndian>()?,
        })
    }

    fn write_to(&self, buffer: &mut Vec<u8>) -> Result<()> {
        buffer.write_u32::<LittleEndian>(self.signature)?;
        buffer.write_u8(self.packet_type)?;
        buffer.write_u8(self.seq_num)?;
        buffer.write_u16::<LittleEndian>(self.flags)?;
        buffer.write_u16::<LittleEndian>(self.key_size)?;
        buffer.write_u16::<LittleEndian>(self.reserved)?;
        Ok(())
    }

    fn get_id(&self) -> u8 {
        SRD_INITIATE_MSG_ID
    }
}

impl SrdInitiate {
    pub fn new(key_size: u16) -> SrdInitiate {
        SrdInitiate {
            signature: SRD_SIGNATURE,
            packet_type: SRD_INITIATE_MSG_ID,
            seq_num: 0,
            flags: 0,
            key_size,
            reserved: 0,
        }
    }
}

#[cfg(test)]
mod test {
    use std;
    use message_types::{SrdInitiate, SrdMessage, srd_msg_id::SRD_INITIATE_MSG_ID, SRD_SIGNATURE};

    #[test]
    fn initiate_encoding() {
        let msg = SrdInitiate::new(2);
        assert_eq!(msg.get_id(), SRD_INITIATE_MSG_ID);

        let mut buffer: Vec<u8> = Vec::new();
        match msg.write_to(&mut buffer) {
            Ok(_) => (),
            Err(_) => assert!(false),
        };

        let mut cursor = std::io::Cursor::new(buffer);
        match SrdInitiate::read_from(&mut cursor) {
            Ok(x) => {
                assert_eq!(x.signature, SRD_SIGNATURE);
                assert_eq!(x, msg);
            }
            Err(_) => assert!(false),
        };
    }
}
