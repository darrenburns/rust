// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use prelude::v1::*;
use io::prelude::*;

use cmp;
use io::{self, SeekFrom, Error, ErrorKind};
use iter::repeat;
use slice;

/// A `Cursor` is a type which wraps a non-I/O object to provide a `Seek`
/// implementation.
///
/// Cursors are typically used with memory buffer objects in order to allow
/// `Seek`, `Read`, and `Write` implementations. For example, common cursor types
/// include `Cursor<Vec<u8>>` and `Cursor<&[u8]>`.
///
/// Implementations of the I/O traits for `Cursor<T>` are currently not generic
/// over `T` itself. Instead, specific implementations are provided for various
/// in-memory buffer types like `Vec<u8>` and `&[u8]`.
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Clone, Debug)]
pub struct Cursor<T> {
    inner: T,
    pos: u64,
}

impl<T> Cursor<T> {
    /// Create a new cursor wrapping the provided underlying I/O object.
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new(inner: T) -> Cursor<T> {
        Cursor { pos: 0, inner: inner }
    }

    /// Consume this cursor, returning the underlying value.
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn into_inner(self) -> T { self.inner }

    /// Get a reference to the underlying value in this cursor.
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_ref(&self) -> &T { &self.inner }

    /// Get a mutable reference to the underlying value in this cursor.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying value as it may corrupt this cursor's position.
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_mut(&mut self) -> &mut T { &mut self.inner }

    /// Returns the current value of this cursor
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn position(&self) -> u64 { self.pos }

    /// Sets the value of this cursor
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn set_position(&mut self, pos: u64) { self.pos = pos; }
}

macro_rules! seek {
    () => {
        fn seek(&mut self, style: SeekFrom) -> io::Result<u64> {
            let pos = match style {
                SeekFrom::Start(n) => { self.pos = n; return Ok(n) }
                SeekFrom::End(n) => self.inner.len() as i64 + n,
                SeekFrom::Current(n) => self.pos as i64 + n,
            };

            if pos < 0 {
                Err(Error::new(ErrorKind::InvalidInput,
                               "invalid seek to a negative position"))
            } else {
                self.pos = pos as u64;
                Ok(self.pos)
            }
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> io::Seek for Cursor<&'a [u8]> { seek!(); }
#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> io::Seek for Cursor<&'a mut [u8]> { seek!(); }
#[stable(feature = "rust1", since = "1.0.0")]
impl io::Seek for Cursor<Vec<u8>> { seek!(); }

macro_rules! read {
    () => {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = try!(Read::read(&mut try!(self.fill_buf()), buf));
            self.pos += n as u64;
            Ok(n)
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> Read for Cursor<&'a [u8]> { read!(); }
#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> Read for Cursor<&'a mut [u8]> { read!(); }
#[stable(feature = "rust1", since = "1.0.0")]
impl Read for Cursor<Vec<u8>> { read!(); }

macro_rules! buffer {
    () => {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            let amt = cmp::min(self.pos, self.inner.len() as u64);
            Ok(&self.inner[(amt as usize)..])
        }
        fn consume(&mut self, amt: usize) { self.pos += amt as u64; }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> BufRead for Cursor<&'a [u8]> { buffer!(); }
#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> BufRead for Cursor<&'a mut [u8]> { buffer!(); }
#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> BufRead for Cursor<Vec<u8>> { buffer!(); }

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> Write for Cursor<&'a mut [u8]> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let pos = cmp::min(self.pos, self.inner.len() as u64);
        let amt = try!((&mut self.inner[(pos as usize)..]).write(data));
        self.pos += amt as u64;
        Ok(amt)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Write for Cursor<Vec<u8>> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Make sure the internal buffer is as least as big as where we
        // currently are
        let pos = self.position();
        let amt = pos.saturating_sub(self.inner.len() as u64);
        self.inner.extend(repeat(0).take(amt as usize));

        // Figure out what bytes will be used to overwrite what's currently
        // there (left), and what will be appended on the end (right)
        let space = self.inner.len() - pos as usize;
        let (left, right) = buf.split_at(cmp::min(space, buf.len()));
        slice::bytes::copy_memory(left, &mut self.inner[(pos as usize)..]);
        self.inner.push_all(right);

        // Bump us forward
        self.set_position(pos + buf.len() as u64);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}


#[cfg(test)]
mod tests {
    use core::prelude::*;

    use io::prelude::*;
    use io::{Cursor, SeekFrom};
    use vec::Vec;

    #[test]
    fn test_vec_writer() {
        let mut writer = Vec::new();
        assert_eq!(writer.write(&[0]).unwrap(), 1);
        assert_eq!(writer.write(&[1, 2, 3]).unwrap(), 3);
        assert_eq!(writer.write(&[4, 5, 6, 7]).unwrap(), 4);
        let b: &[_] = &[0, 1, 2, 3, 4, 5, 6, 7];
        assert_eq!(writer, b);
    }

    #[test]
    fn test_mem_writer() {
        let mut writer = Cursor::new(Vec::new());
        assert_eq!(writer.write(&[0]).unwrap(), 1);
        assert_eq!(writer.write(&[1, 2, 3]).unwrap(), 3);
        assert_eq!(writer.write(&[4, 5, 6, 7]).unwrap(), 4);
        let b: &[_] = &[0, 1, 2, 3, 4, 5, 6, 7];
        assert_eq!(&writer.get_ref()[..], b);
    }

    #[test]
    fn test_buf_writer() {
        let mut buf = [0 as u8; 9];
        {
            let mut writer = Cursor::new(&mut buf[..]);
            assert_eq!(writer.position(), 0);
            assert_eq!(writer.write(&[0]).unwrap(), 1);
            assert_eq!(writer.position(), 1);
            assert_eq!(writer.write(&[1, 2, 3]).unwrap(), 3);
            assert_eq!(writer.write(&[4, 5, 6, 7]).unwrap(), 4);
            assert_eq!(writer.position(), 8);
            assert_eq!(writer.write(&[]).unwrap(), 0);
            assert_eq!(writer.position(), 8);

            assert_eq!(writer.write(&[8, 9]).unwrap(), 1);
            assert_eq!(writer.write(&[10]).unwrap(), 0);
        }
        let b: &[_] = &[0, 1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(buf, b);
    }

    #[test]
    fn test_buf_writer_seek() {
        let mut buf = [0 as u8; 8];
        {
            let mut writer = Cursor::new(&mut buf[..]);
            assert_eq!(writer.position(), 0);
            assert_eq!(writer.write(&[1]).unwrap(), 1);
            assert_eq!(writer.position(), 1);

            assert_eq!(writer.seek(SeekFrom::Start(2)).unwrap(), 2);
            assert_eq!(writer.position(), 2);
            assert_eq!(writer.write(&[2]).unwrap(), 1);
            assert_eq!(writer.position(), 3);

            assert_eq!(writer.seek(SeekFrom::Current(-2)).unwrap(), 1);
            assert_eq!(writer.position(), 1);
            assert_eq!(writer.write(&[3]).unwrap(), 1);
            assert_eq!(writer.position(), 2);

            assert_eq!(writer.seek(SeekFrom::End(-1)).unwrap(), 7);
            assert_eq!(writer.position(), 7);
            assert_eq!(writer.write(&[4]).unwrap(), 1);
            assert_eq!(writer.position(), 8);

        }
        let b: &[_] = &[1, 3, 2, 0, 0, 0, 0, 4];
        assert_eq!(buf, b);
    }

    #[test]
    fn test_buf_writer_error() {
        let mut buf = [0 as u8; 2];
        let mut writer = Cursor::new(&mut buf[..]);
        assert_eq!(writer.write(&[0]).unwrap(), 1);
        assert_eq!(writer.write(&[0, 0]).unwrap(), 1);
        assert_eq!(writer.write(&[0, 0]).unwrap(), 0);
    }

    #[test]
    fn test_mem_reader() {
        let mut reader = Cursor::new(vec!(0, 1, 2, 3, 4, 5, 6, 7));
        let mut buf = [];
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
        assert_eq!(reader.position(), 0);
        let mut buf = [0];
        assert_eq!(reader.read(&mut buf).unwrap(), 1);
        assert_eq!(reader.position(), 1);
        let b: &[_] = &[0];
        assert_eq!(buf, b);
        let mut buf = [0; 4];
        assert_eq!(reader.read(&mut buf).unwrap(), 4);
        assert_eq!(reader.position(), 5);
        let b: &[_] = &[1, 2, 3, 4];
        assert_eq!(buf, b);
        assert_eq!(reader.read(&mut buf).unwrap(), 3);
        let b: &[_] = &[5, 6, 7];
        assert_eq!(&buf[..3], b);
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn read_to_end() {
        let mut reader = Cursor::new(vec!(0, 1, 2, 3, 4, 5, 6, 7));
        let mut v = Vec::new();
        reader.read_to_end(&mut v).unwrap();
        assert_eq!(v, [0, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_slice_reader() {
        let in_buf = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let mut reader = &mut &in_buf[..];
        let mut buf = [];
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
        let mut buf = [0];
        assert_eq!(reader.read(&mut buf).unwrap(), 1);
        assert_eq!(reader.len(), 7);
        let b: &[_] = &[0];
        assert_eq!(&buf[..], b);
        let mut buf = [0; 4];
        assert_eq!(reader.read(&mut buf).unwrap(), 4);
        assert_eq!(reader.len(), 3);
        let b: &[_] = &[1, 2, 3, 4];
        assert_eq!(&buf[..], b);
        assert_eq!(reader.read(&mut buf).unwrap(), 3);
        let b: &[_] = &[5, 6, 7];
        assert_eq!(&buf[..3], b);
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn test_buf_reader() {
        let in_buf = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let mut reader = Cursor::new(&in_buf[..]);
        let mut buf = [];
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
        assert_eq!(reader.position(), 0);
        let mut buf = [0];
        assert_eq!(reader.read(&mut buf).unwrap(), 1);
        assert_eq!(reader.position(), 1);
        let b: &[_] = &[0];
        assert_eq!(buf, b);
        let mut buf = [0; 4];
        assert_eq!(reader.read(&mut buf).unwrap(), 4);
        assert_eq!(reader.position(), 5);
        let b: &[_] = &[1, 2, 3, 4];
        assert_eq!(buf, b);
        assert_eq!(reader.read(&mut buf).unwrap(), 3);
        let b: &[_] = &[5, 6, 7];
        assert_eq!(&buf[..3], b);
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn test_read_char() {
        let b = &b"Vi\xE1\xBB\x87t"[..];
        let mut c = Cursor::new(b).chars();
        assert_eq!(c.next().unwrap().unwrap(), 'V');
        assert_eq!(c.next().unwrap().unwrap(), 'i');
        assert_eq!(c.next().unwrap().unwrap(), 'ệ');
        assert_eq!(c.next().unwrap().unwrap(), 't');
        assert!(c.next().is_none());
    }

    #[test]
    fn test_read_bad_char() {
        let b = &b"\x80"[..];
        let mut c = Cursor::new(b).chars();
        assert!(c.next().unwrap().is_err());
    }

    #[test]
    fn seek_past_end() {
        let buf = [0xff];
        let mut r = Cursor::new(&buf[..]);
        assert_eq!(r.seek(SeekFrom::Start(10)).unwrap(), 10);
        assert_eq!(r.read(&mut [0]).unwrap(), 0);

        let mut r = Cursor::new(vec!(10));
        assert_eq!(r.seek(SeekFrom::Start(10)).unwrap(), 10);
        assert_eq!(r.read(&mut [0]).unwrap(), 0);

        let mut buf = [0];
        let mut r = Cursor::new(&mut buf[..]);
        assert_eq!(r.seek(SeekFrom::Start(10)).unwrap(), 10);
        assert_eq!(r.write(&[3]).unwrap(), 0);
    }

    #[test]
    fn seek_before_0() {
        let buf = [0xff];
        let mut r = Cursor::new(&buf[..]);
        assert!(r.seek(SeekFrom::End(-2)).is_err());

        let mut r = Cursor::new(vec!(10));
        assert!(r.seek(SeekFrom::End(-2)).is_err());

        let mut buf = [0];
        let mut r = Cursor::new(&mut buf[..]);
        assert!(r.seek(SeekFrom::End(-2)).is_err());
    }

    #[test]
    fn test_seekable_mem_writer() {
        let mut writer = Cursor::new(Vec::<u8>::new());
        assert_eq!(writer.position(), 0);
        assert_eq!(writer.write(&[0]).unwrap(), 1);
        assert_eq!(writer.position(), 1);
        assert_eq!(writer.write(&[1, 2, 3]).unwrap(), 3);
        assert_eq!(writer.write(&[4, 5, 6, 7]).unwrap(), 4);
        assert_eq!(writer.position(), 8);
        let b: &[_] = &[0, 1, 2, 3, 4, 5, 6, 7];
        assert_eq!(&writer.get_ref()[..], b);

        assert_eq!(writer.seek(SeekFrom::Start(0)).unwrap(), 0);
        assert_eq!(writer.position(), 0);
        assert_eq!(writer.write(&[3, 4]).unwrap(), 2);
        let b: &[_] = &[3, 4, 2, 3, 4, 5, 6, 7];
        assert_eq!(&writer.get_ref()[..], b);

        assert_eq!(writer.seek(SeekFrom::Current(1)).unwrap(), 3);
        assert_eq!(writer.write(&[0, 1]).unwrap(), 2);
        let b: &[_] = &[3, 4, 2, 0, 1, 5, 6, 7];
        assert_eq!(&writer.get_ref()[..], b);

        assert_eq!(writer.seek(SeekFrom::End(-1)).unwrap(), 7);
        assert_eq!(writer.write(&[1, 2]).unwrap(), 2);
        let b: &[_] = &[3, 4, 2, 0, 1, 5, 6, 1, 2];
        assert_eq!(&writer.get_ref()[..], b);

        assert_eq!(writer.seek(SeekFrom::End(1)).unwrap(), 10);
        assert_eq!(writer.write(&[1]).unwrap(), 1);
        let b: &[_] = &[3, 4, 2, 0, 1, 5, 6, 1, 2, 0, 1];
        assert_eq!(&writer.get_ref()[..], b);
    }

    #[test]
    fn vec_seek_past_end() {
        let mut r = Cursor::new(Vec::new());
        assert_eq!(r.seek(SeekFrom::Start(10)).unwrap(), 10);
        assert_eq!(r.write(&[3]).unwrap(), 1);
    }

    #[test]
    fn vec_seek_before_0() {
        let mut r = Cursor::new(Vec::new());
        assert!(r.seek(SeekFrom::End(-2)).is_err());
    }
}
