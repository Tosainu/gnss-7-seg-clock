#![no_std]

pub fn checksum(buf: &[u8]) -> (u8, u8) {
    let mut ck_a = 0_u8;
    let mut ck_b = 0_u8;
    for c in buf {
        ck_a = ck_a.overflowing_add(*c).0;
        ck_b = ck_b.overflowing_add(ck_a).0;
    }
    (ck_a, ck_b)
}

#[derive(Debug, PartialEq)]
pub struct UbxFrame<'a> {
    pub class: u8,
    pub id: u8,
    pub payload: &'a [u8],
}

pub struct UbxStream<const N: usize> {
    buf: [u8; N],
    begin: usize,
    end: usize,
}

impl<const N: usize> Default for UbxStream<N> {
    fn default() -> Self {
        Self::new()
    }
}

// UBX Frame:
// 0       1       2       3       4       5       6               6+N     7+N
// +-------+-------+-------+-------+-------+-------+-------+-------+-------+-------+
// | Preamble      | Class | ID    | Length (N)    | Payload       | CK_A  | CK_B  |
// +-------+-------+-------+-------+-------+-------+-------+-------+-------+-------+

const UBX_PREAMBLE1: u8 = 0xb5;
const UBX_PREAMBLE2: u8 = 0x62;

const UBX_FRAME_CLASS_OFFSET: usize = 2;
const UBX_FRAME_CLASS_SIZE: usize = 1;

const UBX_FRAME_ID_OFFSET: usize = UBX_FRAME_CLASS_OFFSET + UBX_FRAME_CLASS_SIZE;
const UBX_FRAME_ID_SIZE: usize = 1;

const UBX_FRAME_LENGTH_OFFSET: usize = UBX_FRAME_ID_OFFSET + UBX_FRAME_ID_SIZE;
const UBX_FRAME_LENGTH_SIZE: usize = 2;

const UBX_FRAME_PAYLOAD_OFFSET: usize = UBX_FRAME_LENGTH_OFFSET + UBX_FRAME_LENGTH_SIZE;

const UBX_FRAME_CHECKSUM_SIZE: usize = 2;

const UBX_FRAME_METATATA_SIZE: usize =
    2 + UBX_FRAME_CLASS_SIZE + UBX_FRAME_ID_SIZE + UBX_FRAME_LENGTH_SIZE + UBX_FRAME_CHECKSUM_SIZE;

impl<const N: usize> UbxStream<N> {
    pub fn new() -> Self {
        Self {
            buf: [0; N],
            begin: 0,
            end: 0,
        }
    }

    pub fn commit(&mut self, n: usize) {
        self.end = N.min(self.end + n);
    }

    pub fn consume(&mut self, n: usize) {
        self.begin = self.end.min(self.begin + n);
    }

    pub fn pop(&mut self) -> Option<UbxFrame<'_>> {
        if self.end == N {
            self.buf.copy_within(self.begin..self.end, 0);
            self.end -= self.begin;
            self.begin = 0;
        }

        for i in self.begin..self.end {
            match self.buf[i..self.end] {
                [UBX_PREAMBLE1, UBX_PREAMBLE2, ..] => (),
                [] | [UBX_PREAMBLE1] => break,
                _ => {
                    self.begin = i + 1; //discard this byte
                    continue;
                }
            }

            let frame = &self.buf[i..self.end];
            if frame.len() < UBX_FRAME_METATATA_SIZE {
                break;
            }

            let payload_size = u16::from_le_bytes([
                frame[UBX_FRAME_LENGTH_OFFSET],
                frame[UBX_FRAME_LENGTH_OFFSET + 1],
            ]) as usize;
            if frame.len() < UBX_FRAME_METATATA_SIZE + payload_size {
                break;
            }

            let frame = &frame[..UBX_FRAME_METATATA_SIZE + payload_size];

            let class = frame[UBX_FRAME_CLASS_OFFSET];
            let id = frame[UBX_FRAME_ID_OFFSET];
            let payload = &frame[UBX_FRAME_PAYLOAD_OFFSET..UBX_FRAME_PAYLOAD_OFFSET + payload_size];

            self.begin = i + UBX_FRAME_METATATA_SIZE + payload_size;

            let (ck_a, ck_b) =
                checksum(&frame[UBX_FRAME_CLASS_OFFSET..UBX_FRAME_PAYLOAD_OFFSET + payload_size]);
            if frame[frame.len() - 2] != ck_a || frame[frame.len() - 1] != ck_b {
                break;
            }

            return Some(UbxFrame { class, id, payload });
        }

        None
    }

    pub fn buf_filled(&self) -> &[u8] {
        &self.buf[self.begin..self.end]
    }

    pub fn buf_unused_mut(&mut self) -> &mut [u8] {
        &mut self.buf[self.end..]
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn test_checksum() {
        assert_eq!(
            checksum(&[0x05, 0x01, 0x02, 0x00, 0x06, 0x8a]),
            (0x98_u8, 0xc1_u8)
        );
        assert_eq!(
            checksum(&[0xab, 0xcd, 0x04, 0x00, 0xde, 0xad, 0xbe, 0xef]),
            (0xb4_u8, 0xf5_u8)
        );
    }

    #[test]
    fn test_empty() {
        let mut buf = UbxStream::<16>::new();
        assert_eq!(buf.buf_filled().len(), 0);
        assert_eq!(buf.buf_unused_mut().len(), 16);
        assert_eq!(buf.pop(), None);
    }

    const UBX_FRAME1: [u8; 10] = [
        0xb5, 0x62, // header
        0x05, 0x01, // id/class (=UBX-ACK-ACK)
        0x02, 0x00, // length
        0x06, 0x8a, // payload
        0x98, // ck_a
        0xc1, // ck_b
    ];

    const UBX_FRAME2: [u8; 12] = [
        0xb5, 0x62, // header
        0xab, 0xcd, // id/class (=UBX-ACK-ACK)
        0x04, 0x00, // length
        0xde, 0xad, 0xbe, 0xef, // payload
        0xb4, // ck_a
        0xf5, // ck_b
    ];

    #[test]
    fn test_commit_and_pop() {
        let mut buf = UbxStream::<32>::new();

        buf.buf_unused_mut()[..UBX_FRAME1.len()].copy_from_slice(&UBX_FRAME1);
        buf.commit(UBX_FRAME1.len());

        assert_eq!(buf.buf_filled(), UBX_FRAME1);
        assert_eq!(buf.buf_unused_mut().len(), 22);

        assert_eq!(
            buf.pop(),
            Some(UbxFrame {
                class: 0x05,
                id: 0x01,
                payload: &[0x06, 0x8a],
            })
        );
        assert_eq!(buf.buf_filled(), &[]);
        assert_eq!(buf.buf_unused_mut().len(), 22);

        assert_eq!(buf.pop(), None);
        assert_eq!(buf.buf_filled(), &[]);
        assert_eq!(buf.buf_unused_mut().len(), 22);

        buf.buf_unused_mut()[..UBX_FRAME2.len()].copy_from_slice(&UBX_FRAME2);
        buf.commit(UBX_FRAME2.len());

        buf.buf_unused_mut()[..UBX_FRAME1.len()].copy_from_slice(&UBX_FRAME1);
        buf.commit(UBX_FRAME1.len());

        assert_eq!(buf.buf_filled()[..UBX_FRAME2.len()], UBX_FRAME2);
        assert_eq!(buf.buf_filled()[UBX_FRAME2.len()..], UBX_FRAME1);
        assert_eq!(buf.buf_unused_mut().len(), 0);

        assert_eq!(
            buf.pop(),
            Some(UbxFrame {
                class: 0xab,
                id: 0xcd,
                payload: &[0xde, 0xad, 0xbe, 0xef],
            })
        );
        assert_eq!(buf.buf_filled(), UBX_FRAME1);
        assert_eq!(buf.buf_unused_mut().len(), 10);

        assert_eq!(
            buf.pop(),
            Some(UbxFrame {
                class: 0x05,
                id: 0x01,
                payload: &[0x06, 0x8a],
            })
        );
        assert_eq!(buf.buf_filled(), &[]);
        assert_eq!(buf.buf_unused_mut().len(), 10);

        assert_eq!(buf.pop(), None);
        assert_eq!(buf.buf_filled(), &[]);
        assert_eq!(buf.buf_unused_mut().len(), 10);
    }

    #[test]
    fn test_interrupted() {
        let mut buf = UbxStream::<32>::new();

        buf.buf_unused_mut()[..5].copy_from_slice(&UBX_FRAME1[..5]);
        buf.commit(5);

        assert_eq!(buf.buf_filled(), &UBX_FRAME1[..5]);
        assert_eq!(buf.buf_unused_mut().len(), 27);

        assert_eq!(buf.pop(), None);
        assert_eq!(buf.buf_filled(), &UBX_FRAME1[..5]);
        assert_eq!(buf.buf_unused_mut().len(), 27);

        buf.buf_unused_mut()[..UBX_FRAME1.len() - 5].copy_from_slice(&UBX_FRAME1[5..]);
        buf.commit(UBX_FRAME1.len() - 5);

        assert_eq!(
            buf.pop(),
            Some(UbxFrame {
                class: 0x05,
                id: 0x01,
                payload: &[0x06, 0x8a],
            })
        );
        assert_eq!(buf.buf_filled(), &[]);
        assert_eq!(buf.buf_unused_mut().len(), 22);

        buf.buf_unused_mut()[..1].copy_from_slice(&UBX_FRAME1[..1]);
        buf.commit(1);

        assert_eq!(buf.buf_filled(), &UBX_FRAME1[..1]);
        assert_eq!(buf.buf_unused_mut().len(), 21);

        assert_eq!(buf.pop(), None);
        assert_eq!(buf.buf_filled(), &UBX_FRAME1[..1]);
        assert_eq!(buf.buf_unused_mut().len(), 21);

        buf.buf_unused_mut()[..UBX_FRAME1.len() - 1].copy_from_slice(&UBX_FRAME1[1..]);
        buf.commit(UBX_FRAME1.len() - 1);

        assert_eq!(
            buf.pop(),
            Some(UbxFrame {
                class: 0x05,
                id: 0x01,
                payload: &[0x06, 0x8a],
            })
        );
        assert_eq!(buf.buf_filled(), &[]);
        assert_eq!(buf.buf_unused_mut().len(), 12);
    }

    #[test]
    fn test_garble() {
        let mut buf = UbxStream::<32>::new();

        buf.buf_unused_mut()[..4].copy_from_slice(b"abcd");
        buf.commit(4);

        assert_eq!(buf.buf_filled(), b"abcd");
        assert_eq!(buf.buf_unused_mut().len(), 28);

        assert_eq!(buf.pop(), None);
        assert_eq!(buf.buf_filled(), &[]);
        assert_eq!(buf.buf_unused_mut().len(), 28);

        buf.buf_unused_mut()[..4].copy_from_slice(b"abcd");
        buf.commit(4);

        buf.buf_unused_mut()[..UBX_FRAME1.len()].copy_from_slice(&UBX_FRAME1);
        buf.commit(UBX_FRAME1.len());

        assert_eq!(
            buf.pop(),
            Some(UbxFrame {
                class: 0x05,
                id: 0x01,
                payload: &[0x06, 0x8a],
            })
        );
        assert_eq!(buf.buf_filled(), &[]);
        assert_eq!(buf.buf_unused_mut().len(), 14);
    }

    #[test]
    fn test_consume() {
        let mut buf = UbxStream::<16>::new();

        buf.buf_unused_mut()[..8].copy_from_slice(b"abcdef\r\n");
        buf.commit(8);

        assert_eq!(buf.buf_filled(), b"abcdef\r\n");
        assert_eq!(buf.buf_unused_mut().len(), 8);

        buf.consume(3);

        assert_eq!(buf.buf_filled(), b"def\r\n");
        assert_eq!(buf.buf_unused_mut().len(), 8);

        buf.consume(1234);

        assert_eq!(buf.buf_filled(), &[]);
        assert_eq!(buf.buf_unused_mut().len(), 8);
    }

    #[test]
    fn test_avoid_no_room() {
        let mut buf = UbxStream::<16>::new();

        buf.buf_unused_mut()[..UBX_FRAME2.len()].copy_from_slice(&UBX_FRAME2);
        buf.commit(UBX_FRAME2.len());

        assert_eq!(buf.buf_filled(), UBX_FRAME2);
        assert_eq!(buf.buf_unused_mut().len(), 4);

        buf.buf_unused_mut()[..4].copy_from_slice(&UBX_FRAME1[..4]);
        buf.commit(4);

        assert_eq!(buf.buf_filled()[..UBX_FRAME2.len()], UBX_FRAME2);
        assert_eq!(
            &buf.buf_filled()[UBX_FRAME2.len()..UBX_FRAME2.len() + 4],
            &UBX_FRAME1[..4]
        );
        assert_eq!(buf.buf_unused_mut().len(), 0);

        assert_eq!(
            buf.pop(),
            Some(UbxFrame {
                class: 0xab,
                id: 0xcd,
                payload: &[0xde, 0xad, 0xbe, 0xef],
            })
        );
        assert_eq!(buf.buf_filled(), &UBX_FRAME1[..4]);
        assert_eq!(buf.buf_unused_mut().len(), 0);

        assert_eq!(buf.pop(), None);
        assert_eq!(buf.buf_filled(), &UBX_FRAME1[..4]);
        assert_ne!(buf.buf_unused_mut().len(), 0);
    }
}
