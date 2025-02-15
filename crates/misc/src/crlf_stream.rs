pub struct CrlfStream<const N: usize> {
    buf: [u8; N],
    begin: usize,
    end: usize,
}

impl<const N: usize> Default for CrlfStream<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> CrlfStream<N> {
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

    pub fn pop(&mut self) -> Option<&[u8]> {
        for i in self.begin + 1..self.end {
            if self.buf[i - 1] == b'\r' && self.buf[i] == b'\n' {
                let begin = self.begin;
                self.begin = i + 1;
                return Some(&self.buf[begin..self.begin]);
            }
        }
        if self.end == N {
            self.buf.copy_within(self.begin..self.end, 0);
            self.end -= self.begin;
            self.begin = 0;
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
    use crate::crlf_stream::*;

    #[test]
    fn empty() {
        let mut buf = CrlfStream::<16>::new();
        assert_eq!(buf.buf_filled().len(), 0);
        assert_eq!(buf.buf_unused_mut().len(), 16);
        assert_eq!(buf.pop(), None);
    }

    #[test]
    fn commit_and_pop() {
        let mut buf = CrlfStream::<32>::new();

        buf.buf_unused_mut()[..10].copy_from_slice(b"abc\r\ndef\r\n");
        buf.commit(10);

        assert_eq!(buf.buf_filled(), b"abc\r\ndef\r\n");
        assert_eq!(buf.buf_unused_mut().len(), 22);

        assert_eq!(buf.pop(), Some(b"abc\r\n".as_slice()));
        assert_eq!(buf.buf_filled(), b"def\r\n");
        assert_eq!(buf.buf_unused_mut().len(), 22);

        buf.buf_unused_mut()[..5].copy_from_slice(b"ghi\r\n");
        buf.commit(5);

        assert_eq!(buf.buf_filled(), b"def\r\nghi\r\n");
        assert_eq!(buf.buf_unused_mut().len(), 17);

        assert_eq!(buf.pop(), Some(b"def\r\n".as_slice()));
        assert_eq!(buf.buf_filled(), b"ghi\r\n");
        assert_eq!(buf.buf_unused_mut().len(), 17);

        assert_eq!(buf.pop(), Some(b"ghi\r\n".as_slice()));
        assert_eq!(buf.buf_filled(), b"");
        assert_eq!(buf.buf_unused_mut().len(), 17);

        assert_eq!(buf.pop(), None);
    }

    #[test]
    fn shift_unused() {
        let mut buf = CrlfStream::<16>::new();

        buf.buf_unused_mut()[..11].copy_from_slice(b"abcdef\r\nghi");
        buf.commit(11);

        assert_eq!(buf.pop(), Some(b"abcdef\r\n".as_slice()));
        assert_eq!(buf.buf_filled(), b"ghi");
        assert_eq!(buf.buf_unused_mut().len(), 5);

        buf.buf_unused_mut()[..5].copy_from_slice(b"jklmn");
        buf.commit(5);

        assert_eq!(buf.buf_filled(), b"ghijklmn");
        assert_eq!(buf.buf_unused_mut().len(), 0);

        assert_eq!(buf.pop(), None);

        assert_eq!(buf.buf_filled(), b"ghijklmn");
        assert_eq!(buf.buf_unused_mut().len(), 8);

        buf.buf_unused_mut()[..5].copy_from_slice(b"\r\nopq");
        buf.commit(5);

        assert_eq!(buf.pop(), Some(b"ghijklmn\r\n".as_slice()));
        assert_eq!(buf.buf_filled(), b"opq");
        assert_eq!(buf.buf_unused_mut().len(), 3);

        assert_eq!(buf.pop(), None);
    }

    #[test]
    fn consume() {
        let mut buf = CrlfStream::<16>::new();

        buf.buf_unused_mut()[..8].copy_from_slice(b"abcdef\r\n");
        buf.commit(8);

        assert_eq!(buf.buf_filled(), b"abcdef\r\n");
        assert_eq!(buf.buf_unused_mut().len(), 8);

        buf.consume(3);

        assert_eq!(buf.buf_filled(), b"def\r\n");
        assert_eq!(buf.buf_unused_mut().len(), 8);

        assert_eq!(buf.pop(), Some(b"def\r\n".as_slice()));
    }
}
