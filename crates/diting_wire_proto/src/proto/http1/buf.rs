use std::{collections::VecDeque, io::IoSlice};

use bytes::{Buf, BufMut, Bytes, BytesMut};

/// A list of buffers that implements `Buf` by concatenating them together.
pub(crate) struct BufList<T> {
    bufs: VecDeque<T>,
    remaining: usize,
}

impl<T: Buf> BufList<T> {
    #[inline]
    pub(crate) fn new() -> BufList<T> {
        BufList {
            bufs: VecDeque::new(),
            remaining: 0,
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, buf: T) {
        debug_assert!(buf.has_remaining());
        self.remaining += buf.remaining();
        self.bufs.push_back(buf);
    }

    #[inline]
    pub(crate) fn bufs_cnt(&self) -> usize {
        self.bufs.len()
    }
}

impl<T: Buf> Buf for BufList<T> {
    #[inline]
    fn remaining(&self) -> usize {
        self.remaining
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        self.bufs.front().map(Buf::chunk).unwrap_or_default()
    }

    #[inline]
    fn advance(&mut self, mut cnt: usize) {
        assert!(cnt <= self.remaining, "`cnt` greater than remaining");
        self.remaining -= cnt;
        while cnt > 0 {
            {
                let front = &mut self.bufs[0];
                let rem = front.remaining();
                if rem > cnt {
                    front.advance(cnt);
                    return;
                } else {
                    front.advance(rem);
                    cnt -= rem;
                }
            }
            self.bufs.pop_front();
        }
    }

    #[inline]
    fn chunks_vectored<'t>(&'t self, dst: &mut [IoSlice<'t>]) -> usize {
        if dst.is_empty() {
            return 0;
        }
        let mut vecs = 0;
        for buf in &self.bufs {
            vecs += buf.chunks_vectored(&mut dst[vecs..]);
            if vecs == dst.len() {
                break;
            }
        }
        vecs
    }

    #[inline]
    fn copy_to_bytes(&mut self, len: usize) -> Bytes {
        // Our inner buffer may have an optimized version of copy_to_bytes, and if the whole
        // request can be fulfilled by the front buffer, we can take advantage.
        match self.bufs.front_mut() {
            Some(front) if front.remaining() == len => {
                let b = front.copy_to_bytes(len);
                self.bufs.pop_front();
                self.remaining -= len;
                b
            }
            Some(front) if front.remaining() > len => {
                self.remaining -= len;
                front.copy_to_bytes(len)
            }
            _ => {
                assert!(len <= self.remaining(), "`len` greater than remaining");
                let mut bm = BytesMut::with_capacity(len);
                bm.put(self.take(len));
                bm.freeze()
            }
        }
    }
}
