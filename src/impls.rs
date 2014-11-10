use collections::slice::SlicePrelude;
use collections::string::String;
use collections::vec::Vec;
use core::clone::Clone;
use core::fmt::{Formatter,FormatError,Show};
use core::mem;
use core::result::Result;
use core::str::StrPrelude;

use raw::{Prim, RawIobuf};
use iobuf::Iobuf;

/// An `Iobuf` that cannot write into the buffer, but all read-only operations
/// are supported. It is possible to get a `RWIobuf` by performing a `deep_clone`
/// of the Iobuf, but this is extremely inefficient.
///
/// If your function only needs to do read-only operations on an Iobuf, consider
/// taking a generic `Iobuf` trait instead. That way, it can be used with either
/// a ROIobuf or a RWIobuf, generically.
pub struct ROIobuf<'a> {
  raw: RawIobuf<'a>,
}

impl<'a> Clone for ROIobuf<'a> {
  fn clone(&self) -> ROIobuf<'a> {
    ROIobuf {
      raw: self.raw.clone()
    }
  }
}

/// An `Iobuf` which can read and write into a buffer.
///
/// Iobufs may be `cloned` cheaply. When cloned, the data itself is shared and
/// refcounted, and a new copy of the limits and window is made. This can be
/// used to construct multiple views into the same buffer.
///
/// `poke` and `fill` write a value at a position relative to the start of
/// the window. Only `fill` advances the window by the amount written.
/// They are meant to be used with `try!`.
///
/// A suffix `_be` means the data will be read big-endian. A suffix `_le` means
/// the data will be read little-endian.
///
/// The `unsafe_` prefix means the function omits bounds checks. Misuse can
/// easily cause security issues. Be careful!
pub struct RWIobuf<'a> {
  raw: RawIobuf<'a>,
}

impl<'a> Clone for RWIobuf<'a> {
  fn clone(&self) -> RWIobuf<'a> {
    RWIobuf {
      raw: self.raw.clone()
    }
  }
}

impl<'a> ROIobuf<'a> {
  /// Constructs a trivially empty Iobuf, limits and window are 0, and there's
  /// an empty backing buffer. Unfortunately, that backing buffer is refcounted,
  /// so this still needs an allocation.
  ///
  /// ```
  /// use iobuf::{ROIobuf,Iobuf};
  ///
  /// let mut b = ROIobuf::empty();
  ///
  /// assert_eq!(b.cap(), 0);
  /// assert_eq!(b.len(), 0);
  /// ```
  #[inline]
  pub fn empty() -> ROIobuf<'static> {
    ROIobuf { raw: RawIobuf::empty() }
  }

  /// Constructs an Iobuf with the same contents as a string. The limits and
  /// window will be initially set to cover the whole string.
  ///
  /// ```
  /// use iobuf::{ROIobuf,Iobuf};
  ///
  /// let mut b = ROIobuf::from_str("hello");
  ///
  /// assert_eq!(b.cap(), 5);
  /// assert_eq!(b.len(), 5);
  /// unsafe { assert_eq!(b.as_window_slice(), b"hello"); }
  /// unsafe { assert_eq!(b.as_limit_slice(), b"hello"); }
  /// ```
  #[inline]
  pub fn from_str<'a>(s: &'a str) -> ROIobuf<'a> {
    ROIobuf { raw: RawIobuf::from_str(s) }
  }

  /// Directly converts a string into a read-only Iobuf. The Iobuf will take
  /// ownership of the string, therefore there will be no copying.
  ///
  /// ```
  /// use iobuf::{ROIobuf,Iobuf};
  ///
  /// let mut b = ROIobuf::from_string("hello".into_string());
  ///
  /// assert_eq!(b.cap(), 5);
  /// assert_eq!(b.len(), 5);
  /// unsafe { assert_eq!(b.as_window_slice(), b"hello"); }
  /// unsafe { assert_eq!(b.as_limit_slice(), b"hello"); }
  /// ```
  #[inline]
  pub fn from_string(s: String) -> ROIobuf<'static> {
    ROIobuf { raw: RawIobuf::from_string(s) }
  }

  /// Directly converts a byte vector into a read-only Iobuf. The Iobuf will
  /// take ownership of the vector, therefore there will be no copying.
  ///
  /// ```
  /// use iobuf::{ROIobuf,Iobuf};
  ///
  /// let mut v = vec!(1u8, 2, 3, 4, 5, 6);
  /// v.as_mut_slice()[1] = 20;
  ///
  /// let mut b = ROIobuf::from_vec(v);
  ///
  /// let expected = [ 1,20,3,4,5,6 ];
  /// unsafe { assert_eq!(b.as_window_slice(), expected.as_slice()); }
  /// ```
  #[inline]
  pub fn from_vec(v: Vec<u8>) -> ROIobuf<'static> {
    ROIobuf { raw: RawIobuf::from_vec(v) }
  }

  /// Construclts an Iobuf from a slice. The Iobuf will not copy the slice
  /// contents, and therefore their lifetimes will be linked.
  ///
  /// ```
  /// use iobuf::{ROIobuf,Iobuf};
  ///
  /// let s = [1,2,3,4];
  ///
  /// let mut b = ROIobuf::from_slice(s.as_slice());
  ///
  /// assert_eq!(b.advance(1), Ok(()));
  ///
  /// assert_eq!(s[1], 2); // we can still use the slice!
  /// assert_eq!(b.peek_be(1), Ok(0x0304u16)); // ...and the Iobuf!
  /// ```
  #[inline]
  pub fn from_slice<'a>(s: &'a [u8]) -> ROIobuf<'a> {
    ROIobuf { raw: RawIobuf::from_slice(s) }
  }
}

impl<'a> RWIobuf<'a> {
  /// Constructs a trivially empty Iobuf, limits and window are 0, and there's
  /// an empty backing buffer. Unfortunately, that backing buffer is refcounted,
  /// so this still needs an allocation.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::empty();
  ///
  /// assert_eq!(b.len(), 0);
  /// assert_eq!(b.cap(), 0);
  /// ```
  #[inline]
  pub fn empty() -> RWIobuf<'static> {
    RWIobuf { raw: RawIobuf::empty() }
  }

  /// Constructs a new Iobuf with a buffer of size `len`, undefined contents,
  /// and the limits and window set to the full size of the buffer.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// assert_eq!(b.len(), 10);
  /// assert_eq!(b.cap(), 10);
  /// ```
  #[inline(always)]
  pub fn new(len: uint) -> RWIobuf<'static> {
    RWIobuf { raw: RawIobuf::new(len) }
  }

  /// Directly converts a string into a writeable Iobuf. The Iobuf will take
  /// ownership of the string, therefore there will be no copying.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::from_string("hello".into_string());
  ///
  /// b.poke_be(1, b'4').unwrap();
  ///
  /// assert_eq!(b.len(), 5);
  /// assert_eq!(b.cap(), 5);
  /// unsafe { assert_eq!(b.as_window_slice(), b"h4llo"); }
  /// unsafe { assert_eq!(b.as_limit_slice(), b"h4llo"); }
  /// ```
  #[inline(always)]
  pub fn from_string(s: String) -> RWIobuf<'static> {
    RWIobuf { raw: RawIobuf::from_string(s) }
  }

  /// Directly converts a byte vector into a writeable Iobuf. The Iobuf will
  /// take ownership of the vector, therefore there will be no copying.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut v = vec!(1u8, 2, 3, 4, 5, 6, 10);
  /// v.as_mut_slice()[1] = 20;
  ///
  /// let mut b = RWIobuf::from_vec(v);
  ///
  /// let expected = [ 1,20,3,4,5,6,10 ];
  /// unsafe { assert_eq!(b.as_window_slice(), expected.as_slice()); }
  /// ```
  #[inline(always)]
  pub fn from_vec(v: Vec<u8>) -> RWIobuf<'static> {
    RWIobuf { raw: RawIobuf::from_vec(v) }
  }

  /// Construclts an Iobuf from a slice. The Iobuf will not copy the slice
  /// contents, and therefore their lifetimes will be linked.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut s = [1,2,3,4];
  ///
  /// {
  ///   let mut b = RWIobuf::from_slice(s.as_mut_slice());
  ///
  ///   assert_eq!(b.advance(1), Ok(()));
  ///   assert_eq!(b.peek_be(1), Ok(0x0304u16)); // ...and the Iobuf!
  ///   assert_eq!(b.poke_be(1, 100u8), Ok(()));
  /// }
  ///
  /// // We can still use the slice, but only because of the braces above.
  /// assert_eq!(s[2], 100);
  /// ```
  #[inline(always)]
  pub fn from_slice<'a>(s: &'a mut [u8]) -> RWIobuf<'a> {
    RWIobuf { raw: RawIobuf::from_slice(s) }
  }

  /// Reads the data in the window as a mutable slice. Note that since `&mut`
  /// in rust really means `&unique`, this function lies. There can exist
  /// multiple slices of the same data. Therefore, this function is unsafe.
  ///
  /// It may only be used safely if you ensure that the data in the iobuf never
  /// interacts with the slice, as they point to the same data. `peek`ing or
  /// `poke`ing the slice returned from this function is a big no-no.
  ///
  /// ```
  /// use iobuf::{RWIobuf, Iobuf};
  ///
  /// let mut s = [1,2,3];
  ///
  /// {
  ///   let mut b = RWIobuf::from_slice(&mut s);
  ///
  ///   assert_eq!(b.advance(1), Ok(()));
  ///   unsafe { b.as_window_slice_mut()[1] = 30; }
  /// }
  ///
  /// let expected = [ 1,2,30 ];
  /// assert_eq!(s.as_slice(), expected.as_slice());
  /// ```
  #[inline(always)]
  pub unsafe fn as_window_slice_mut<'b>(&'b self) -> &'b mut [u8] {
    self.raw.as_window_slice_mut()
  }

  /// Reads the data in the window as a mutable slice. Note that since `&mut`
  /// in rust really means `&unique`, this function lies. There can exist
  /// multiple slices of the same data. Therefore, this function is unsafe.
  ///
  /// It may only be used safely if you ensure that the data in the iobuf never
  /// interacts with the slice, as they point to the same data. `peek`ing or
  /// `poke`ing the slice returned from this function is a big no-no.
  ///
  /// ```
  /// use iobuf::{RWIobuf, Iobuf};
  ///
  /// let mut s = [1,2,3];
  ///
  /// {
  ///   let mut b = RWIobuf::from_slice(&mut s);
  ///
  ///   assert_eq!(b.advance(1), Ok(()));
  ///   unsafe { b.as_limit_slice_mut()[1] = 20; }
  /// }
  ///
  /// assert_eq!(s.as_slice(), [1,20,3].as_slice());
  /// ```
  #[inline(always)]
  pub unsafe fn as_limit_slice_mut<'b>(&'b self) -> &'b mut [u8] {
    self.raw.as_limit_slice_mut()
  }

  /// Gets a read-only copy of this Iobuf. This is a very cheap operation, as
  /// the backing buffers are shared. This can be useful for interfacing with
  /// code that only accepts read-only Iobufs.
  ///
  /// In general, ROIobuf should never be used as a function parameter. If
  /// read-only acceess is all that is required, take a generic `<T: Iobuf>`.
  ///
  /// ```
  /// use iobuf::{ROIobuf,RWIobuf,Iobuf};
  ///
  /// let mut s = [1,2,3,4];
  ///
  /// let rwb: RWIobuf = RWIobuf::from_slice(s.as_mut_slice());
  ///
  /// // write some data into rwb.
  ///
  /// let rb: ROIobuf = rwb.read_only();
  ///
  /// // now do read-only ops.
  /// assert_eq!(rb.len(), 4);
  /// ```
  #[inline(always)]
  pub fn read_only(&self) -> ROIobuf<'a> {
    ROIobuf { raw: self.raw.clone() }
  }

  /// Copies data from the window to the lower limit fo the iobuf and sets the
  /// window to range from the end of the copied data to the upper limit. This
  /// is typically called after a series of `Consume`s to save unread data and
  /// prepare for the next series of `Fill`s and `flip_lo`s.
  ///
  /// ```
  /// use std::iter::range;
  /// use std::result::{Result,Ok};
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// // A header, saying how many shorts will follow. Unfortunately, our buffer
  /// // isn't big enough for all the shorts! Assume the rest will be sent in a
  /// // later packet.
  /// let mut s = [ 0x02, 0x11, 0x22, 0x33 ];
  /// let mut b = RWIobuf::from_slice(s.as_mut_slice());
  ///
  /// #[deriving(Eq, PartialEq, Show)]
  /// enum ParseState {
  ///   NeedMore(u16), // sum so far
  ///   Done(u16),     // final sum
  /// };
  ///
  /// // Returns a pair of the sum of shorts seen so far, and `true` if we're
  /// // finally done parsing. The sum will be partial if parsing is incomplete.
  /// fn parse(b: &mut RWIobuf) -> Result<ParseState, ()> {
  ///   let len: u8 = try!(b.consume_be());
  ///   let mut sum = 0u16;
  ///
  ///   for _ in range(0u8, len) {
  ///     unsafe {
  ///       if b.len() < 2 {
  ///         b.compact();
  ///         return Ok(NeedMore(sum));
  ///       }
  ///       sum += b.unsafe_consume_be();
  ///     }
  ///   }
  ///
  ///   Ok(Done(sum))
  /// }
  ///
  /// assert_eq!(parse(&mut b), Ok(NeedMore(0x1122)));
  /// assert_eq!(b.len(), 3);
  /// assert_eq!(b.cap(), 4);
  /// assert_eq!(b.peek_be(0), Ok(0x11u8));
  /// ```
  #[inline(always)]
  pub fn compact(&mut self) { self.raw.compact() }

  /// Writes the bytes at a given offset from the beginning of the window, into
  /// the supplied buffer. Either the entire buffer is copied, or an error is
  /// returned because bytes outside of the window would be written.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let data = [ 1,2,3,4 ];
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// assert_eq!(b.poke(0, data.as_slice()), Ok(()));
  /// assert_eq!(b.poke(3, data.as_slice()), Ok(()));
  /// assert_eq!(b.resize(7), Ok(()));
  /// assert_eq!(b.poke(4, data.as_slice()), Err(())); // no partial write, just failure
  ///
  /// let expected = [ 1,2,3,1,2,3,4 ];
  /// unsafe { assert_eq!(b.as_window_slice(), expected.as_slice()); }
  /// ```
  #[inline(always)]
  pub fn poke(&self, pos: u32, src: &[u8]) -> Result<(), ()> { self.raw.poke(pos, src) }

  /// Writes a big-endian primitive at a given offset from the beginning of the
  /// window.
  ///
  /// An error is returned if bytes outside of the window would be accessed.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// assert_eq!(b.poke_be(0, 0x0304u16), Ok(()));
  /// assert_eq!(b.poke_be(1, 0x0505u16), Ok(()));
  /// assert_eq!(b.poke_be(3, 0x06070809u32), Ok(()));
  ///
  /// assert_eq!(b.resize(7), Ok(()));
  ///
  /// let expected = [ 3,5,5,6,7,8,9 ];
  /// unsafe { assert_eq!(b.as_window_slice(), expected.as_slice()); }
  /// ```
  #[inline(always)]
  pub fn poke_be<T: Prim>(&self, pos: u32, t: T) -> Result<(), ()> { self.raw.poke_be(pos, t) }

  /// Writes a little-endian primitive at a given offset from the beginning of
  /// the window.
  ///
  /// An error is returned if bytes outside of the window would be accessed.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// assert_eq!(b.poke_le(0, 0x0304u16), Ok(()));
  /// assert_eq!(b.poke_le(1, 0x0505u16), Ok(()));
  /// assert_eq!(b.poke_le(3, 0x06070809u32), Ok(()));
  ///
  /// assert_eq!(b.resize(7), Ok(()));
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 4, 5, 5, 9, 8, 7, 6 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub fn poke_le<T: Prim>(&self, pos: u32, t: T) -> Result<(), ()> { self.raw.poke_le(pos, t) }

  /// Writes bytes from the supplied buffer, starting from the front of the
  /// window. Either the entire buffer is copied, or an error is returned
  /// because bytes outside the window were requested.
  ///
  /// After the bytes have been written, the window will be moved to no longer
  /// include then.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let data = [ 1, 2, 3, 4 ];
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// assert_eq!(b.fill(data.as_slice()), Ok(()));
  /// assert_eq!(b.fill(data.as_slice()), Ok(()));
  /// assert_eq!(b.fill(data.as_slice()), Err(()));
  ///
  /// b.flip_lo();
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 1,2,3,4,1,2,3,4 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub fn fill(&mut self, src: &[u8]) -> Result<(), ()> { self.raw.fill(src) }

  /// Writes a big-endian primitive into the beginning of the window.
  ///
  /// After the primitive has been written, the window will be moved such that
  /// it is no longer included.
  ///
  /// An error is returned if bytes outside of the window were requested.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// assert_eq!(b.fill_be(0x12345678u32), Ok(()));
  /// assert_eq!(b.fill_be(0x11223344u32), Ok(()));
  /// assert_eq!(b.fill_be(0x54321123u32), Err(()));
  /// assert_eq!(b.fill_be(0x8877u16), Ok(()));
  ///
  /// b.flip_lo();
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 0x12, 0x34, 0x56, 0x78
  ///                                          , 0x11, 0x22, 0x33, 0x44
  ///                                          , 0x88, 0x77 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub fn fill_be<T: Prim>(&mut self, t: T) -> Result<(), ()> { self.raw.fill_be(t) }

  /// Writes a little-endian primitive into the beginning of the window.
  ///
  /// After the primitive has been written, the window will be moved such that
  /// it is no longer included.
  ///
  /// An error is returned if bytes outside of the window were requested.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// assert_eq!(b.fill_le(0x12345678u32), Ok(()));
  /// assert_eq!(b.fill_le(0x11223344u32), Ok(()));
  /// assert_eq!(b.fill_le(0x54321123u32), Err(()));
  /// assert_eq!(b.fill_le(0x8877u16), Ok(()));
  ///
  /// b.flip_lo();
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 0x78, 0x56, 0x34, 0x12
  ///                                          , 0x44, 0x33, 0x22, 0x11
  ///                                          , 0x77, 0x88 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub fn fill_le<T: Prim>(&mut self, t: T) -> Result<(), ()> { self.raw.fill_le(t) }

  /// Writes the bytes at a given offset from the beginning of the window, into
  /// the supplied buffer. It is undefined behavior to write outside the iobuf
  /// window.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let data = [ 1,2,3,4 ];
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// unsafe {
  ///   b.check_range_fail(1, 7);
  ///
  ///   b.unsafe_advance(1);
  ///   b.narrow();
  ///
  ///   b.unsafe_poke(0, data);
  ///   b.unsafe_poke(3, data);
  ///   b.unsafe_advance(7);
  /// }
  ///
  /// b.flip_lo();
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 1,2,3,1,2,3,4 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub unsafe fn unsafe_poke(&self, pos: u32, src: &[u8]) { self.raw.unsafe_poke(pos, src) }

  /// Writes a big-endian primitive at a given offset from the beginning of the
  /// window. It is undefined behavior to write outside the iobuf window.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// unsafe {
  ///   b.check_range_fail(0, 7);
  ///
  ///   b.unsafe_poke_be(0, 0x0304u16);
  ///   b.unsafe_poke_be(1, 0x0505u16);
  ///   b.unsafe_poke_be(3, 0x06070809u32);
  /// }
  ///
  /// assert_eq!(b.resize(7), Ok(()));
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 3, 5, 5, 6, 7, 8, 9 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub unsafe fn unsafe_poke_be<T: Prim>(&self, pos: u32, t: T) { self.raw.unsafe_poke_be(pos, t) }

  /// Writes a little-endian primitive at a given offset from the beginning of
  /// the window. It is undefined behavior to write outside the iobuf window.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// unsafe {
  ///   b.check_range_fail(0, 7);
  ///
  ///   b.unsafe_poke_le(0, 0x0304u16);
  ///   b.unsafe_poke_le(1, 0x0505u16);
  ///   b.unsafe_poke_le(3, 0x06070809u32);
  /// }
  ///
  /// assert_eq!(b.resize(7), Ok(()));
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 4, 5, 5, 9, 8, 7, 6 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub unsafe fn unsafe_poke_le<T: Prim>(&self, pos: u32, t: T) { self.raw.unsafe_poke_le(pos, t) }

  /// Writes bytes from the supplied buffer, starting from the front of the
  /// window. It is undefined behavior to write outside the iobuf window.
  ///
  /// After the bytes have been written, the window will be moved to no longer
  /// include then.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let data = [ 1, 2, 3, 4 ];
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// unsafe {
  ///   b.check_range_fail(0, 8);
  ///
  ///   b.unsafe_fill(data.as_slice());
  ///   b.unsafe_fill(data.as_slice());
  /// }
  ///
  /// b.flip_lo();
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 1,2,3,4,1,2,3,4 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub unsafe fn unsafe_fill(&mut self, src: &[u8]) { self.raw.unsafe_fill(src) }

  /// Writes a big-endian primitive into the beginning of the window. It is
  /// undefined behavior to write outside the iobuf window.
  ///
  /// After the primitive has been written, the window will be moved such that
  /// it is no longer included.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// unsafe {
  ///   b.check_range_fail(0, 10);
  ///
  ///   b.unsafe_fill_be(0x12345678u32);
  ///   b.unsafe_fill_be(0x11223344u32);
  ///   // b.unsafe_fill_be(0x54321123u32); DO NOT DO THIS. Undefined behavior.
  ///   b.unsafe_fill_be(0x8877u16);
  /// }
  ///
  /// b.flip_lo();
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 0x12, 0x34, 0x56, 0x78
  ///                                          , 0x11, 0x22, 0x33, 0x44
  ///                                          , 0x88, 0x77 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub unsafe fn unsafe_fill_be<T: Prim>(&mut self, t: T) { self.raw.unsafe_fill_be(t) }

  /// Writes a little-endian primitive into the beginning of the window. It is
  /// undefined behavior to write outside the iobuf window.
  ///
  /// After the primitive has been written, the window will be moved such that
  /// it is no longer included.
  ///
  /// ```
  /// use iobuf::{RWIobuf,Iobuf};
  ///
  /// let mut b = RWIobuf::new(10);
  ///
  /// unsafe {
  ///   b.check_range_fail(0, 10);
  ///
  ///   b.unsafe_fill_le(0x12345678u32);
  ///   b.unsafe_fill_le(0x11223344u32);
  ///   // b.unsafe_fill_le(0x54321123u32); DO NOT DO THIS. Undefined behavior.
  ///   b.unsafe_fill_le(0x8877u16);
  /// }
  ///
  /// b.flip_lo();
  ///
  /// unsafe { assert_eq!(b.as_window_slice(), [ 0x78, 0x56, 0x34, 0x12
  ///                                          , 0x44, 0x33, 0x22, 0x11
  ///                                          , 0x77, 0x88 ].as_slice()); }
  /// ```
  #[inline(always)]
  pub unsafe fn unsafe_fill_le<T: Prim>(&mut self, t: T) { self.raw.unsafe_fill_le(t) }
}

impl<'a> Iobuf for ROIobuf<'a> {
  #[inline(always)]
  fn deep_clone(&self) -> RWIobuf<'static> { RWIobuf { raw: self.raw.deep_clone() } }

  #[inline(always)]
  fn len(&self) -> u32 { self.raw.len() }

  #[inline(always)]
  fn cap(&self) -> u32 { self.raw.cap() }

  #[inline(always)]
  fn is_empty(&self) -> bool { self.raw.is_empty() }

  #[inline(always)]
  unsafe fn as_window_slice<'b>(&'b self) -> &'b [u8] { self.raw.as_window_slice() }

  #[inline(always)]
  unsafe fn as_limit_slice<'b>(&'b self) -> &'b [u8] { self.raw.as_limit_slice() }

  #[inline(always)]
  fn sub_window(&mut self, pos: u32, len: u32) -> Result<(), ()> { self.raw.sub_window(pos, len) }

  #[inline(always)]
  fn sub_window_from(&mut self, pos: u32) -> Result<(), ()> { self.raw.sub_window_from(pos) }

  #[inline(always)]
  fn sub_window_to(&mut self, len: u32) -> Result<(), ()> { self.raw.sub_window_to(len) }

  #[inline(always)]
  unsafe fn unsafe_sub_window(&mut self, pos: u32, len: u32) { self.raw.unsafe_sub_window(pos, len) }

  #[inline(always)]
  unsafe fn unsafe_sub_window_from(&mut self, pos: u32) { self.raw.unsafe_sub_window_from(pos) }

  #[inline(always)]
  unsafe fn unsafe_sub_window_to(&mut self, len: u32) { self.raw.unsafe_sub_window_to(len) }

  #[inline(always)]
  fn sub(&mut self, pos: u32, len: u32) -> Result<(), ()> { self.raw.sub(pos, len) }

  #[inline(always)]
  fn sub_from(&mut self, pos: u32) -> Result<(), ()> { self.raw.sub_from(pos) }

  #[inline(always)]
  fn sub_to(&mut self, len: u32) -> Result<(), ()> { self.raw.sub_to(len) }

  #[inline(always)]
  unsafe fn unsafe_sub(&mut self, pos: u32, len: u32) { self.raw.unsafe_sub(pos, len) }

  #[inline(always)]
  unsafe fn unsafe_sub_from(&mut self, pos: u32) { self.raw.unsafe_sub_from(pos) }

  #[inline(always)]
  unsafe fn unsafe_sub_to(&mut self, len: u32) { self.raw.unsafe_sub_to(len) }

  #[inline(always)]
  fn set_limits_and_window(&mut self, limits: (u32, u32), window: (u32, u32)) -> Result<(), ()> { self.raw.set_limits_and_window(limits, window) }

  #[inline(always)]
  fn narrow(&mut self) { self.raw.narrow() }

  #[inline(always)]
  fn advance(&mut self, len: u32) -> Result<(), ()> { self.raw.advance(len) }

  #[inline(always)]
  unsafe fn unsafe_advance(&mut self, len: u32) { self.raw.unsafe_advance(len) }

  #[inline(always)]
  fn extend(&mut self, len: u32) -> Result<(), ()> { self.raw.extend(len) }

  #[inline(always)]
  unsafe fn unsafe_extend(&mut self, len: u32) { self.raw.unsafe_extend(len) }

  #[inline(always)]
  fn is_extended_by_ro<'a>(&self, other: &ROIobuf<'a>) -> bool { self.raw.is_extended_by(&other.raw) }

  #[inline(always)]
  fn is_extended_by_rw<'a>(&self, other: &RWIobuf<'a>) -> bool { self.raw.is_extended_by(&other.raw) }

  #[inline(always)]
  fn resize(&mut self, len: u32) -> Result<(), ()> { self.raw.resize(len) }

  #[inline(always)]
  unsafe fn unsafe_resize(&mut self, len: u32) { self.raw.unsafe_resize(len) }

  #[inline(always)]
  fn rewind(&mut self) { self.raw.rewind() }

  #[inline(always)]
  fn reset(&mut self) { self.raw.reset() }

  #[inline(always)]
  fn flip_lo(&mut self) { self.raw.flip_lo() }

  #[inline(always)]
  fn flip_hi(&mut self) { self.raw.flip_hi() }

  #[inline(always)]
  fn peek(&self, pos: u32, dst: &mut [u8]) -> Result<(), ()> { self.raw.peek(pos, dst) }
  #[inline(always)]
  fn peek_be<T: Prim>(&self, pos: u32) -> Result<T, ()> { self.raw.peek_be(pos) }
  #[inline(always)]
  fn peek_le<T: Prim>(&self, pos: u32) -> Result<T, ()> { self.raw.peek_le(pos) }

  #[inline(always)]
  fn consume(&mut self, dst: &mut [u8]) -> Result<(), ()> { self.raw.consume(dst) }
  #[inline(always)]
  fn consume_be<T: Prim>(&mut self) -> Result<T, ()> { self.raw.consume_be::<T>() }
  #[inline(always)]
  fn consume_le<T: Prim>(&mut self) -> Result<T, ()> { self.raw.consume_le::<T>() }

  #[inline(always)]
  fn check_range(&self, pos: u32, len: u32) -> Result<(), ()> { self.raw.check_range_u32(pos, len) }

  #[inline(always)]
  fn check_range_uint(&self, pos: u32, len: uint) -> Result<(), ()> { self.raw.check_range_uint(pos, len) }

  #[inline(always)]
  fn check_range_fail(&self, pos: u32, len: u32) { self.raw.check_range_u32_fail(pos, len) }

  #[inline(always)]
  fn check_range_uint_fail(&self, pos: u32, len: uint) { self.raw.check_range_uint_fail(pos, len) }

  #[inline(always)]
  unsafe fn unsafe_peek(&self, pos: u32, dst: &mut [u8]) { self.raw.unsafe_peek(pos, dst) }
  #[inline(always)]
  unsafe fn unsafe_peek_be<T: Prim>(&self, pos: u32) -> T { self.raw.unsafe_peek_be(pos) }
  #[inline(always)]
  unsafe fn unsafe_peek_le<T: Prim>(&self, pos: u32) -> T { self.raw.unsafe_peek_le(pos) }

  #[inline(always)]
  unsafe fn unsafe_consume(&mut self, dst: &mut [u8]) { self.raw.unsafe_consume(dst) }
  #[inline(always)]
  unsafe fn unsafe_consume_be<T: Prim>(&mut self) -> T { self.raw.unsafe_consume_be::<T>() }
  #[inline(always)]
  unsafe fn unsafe_consume_le<T: Prim>(&mut self) -> T { self.raw.unsafe_consume_le::<T>() }
}

impl<'a> Iobuf for RWIobuf<'a> {
  #[inline(always)]
  fn deep_clone(&self) -> RWIobuf<'static> { RWIobuf { raw: self.raw.deep_clone() } }

  #[inline(always)]
  fn len(&self) -> u32 { self.raw.len() }

  #[inline(always)]
  fn cap(&self) -> u32 { self.raw.cap() }

  #[inline(always)]
  fn is_empty(&self) -> bool { self.raw.is_empty() }

  #[inline(always)]
  unsafe fn as_window_slice<'b>(&'b self) -> &'b [u8] { self.raw.as_window_slice() }

  #[inline(always)]
  unsafe fn as_limit_slice<'b>(&'b self) -> &'b [u8] { self.raw.as_limit_slice() }

  #[inline(always)]
  fn sub_window(&mut self, pos: u32, len: u32) -> Result<(), ()> { self.raw.sub_window(pos, len) }

  #[inline(always)]
  fn sub_window_from(&mut self, pos: u32) -> Result<(), ()> { self.raw.sub_window_from(pos) }

  #[inline(always)]
  fn sub_window_to(&mut self, len: u32) -> Result<(), ()> { self.raw.sub_window_to(len) }

  #[inline(always)]
  unsafe fn unsafe_sub_window(&mut self, pos: u32, len: u32) { self.raw.unsafe_sub_window(pos, len) }

  #[inline(always)]
  unsafe fn unsafe_sub_window_from(&mut self, pos: u32) { self.raw.unsafe_sub_window_from(pos) }

  #[inline(always)]
  unsafe fn unsafe_sub_window_to(&mut self, len: u32) { self.raw.unsafe_sub_window_to(len) }

  #[inline(always)]
  fn sub(&mut self, pos: u32, len: u32) -> Result<(), ()> { self.raw.sub(pos, len) }

  #[inline(always)]
  fn sub_from(&mut self, pos: u32) -> Result<(), ()> { self.raw.sub_from(pos) }

  #[inline(always)]
  fn sub_to(&mut self, len: u32) -> Result<(), ()> { self.raw.sub_to(len) }

  #[inline(always)]
  unsafe fn unsafe_sub(&mut self, pos: u32, len: u32) { self.raw.unsafe_sub(pos, len) }

  #[inline(always)]
  unsafe fn unsafe_sub_from(&mut self, pos: u32) { self.raw.unsafe_sub_from(pos) }

  #[inline(always)]
  unsafe fn unsafe_sub_to(&mut self, len: u32) { self.raw.unsafe_sub_to(len) }

  #[inline(always)]
  fn set_limits_and_window(&mut self, limits: (u32, u32), window: (u32, u32)) -> Result<(), ()> { self.raw.set_limits_and_window(limits, window) }

  #[inline(always)]
  fn narrow(&mut self) { self.raw.narrow() }

  #[inline(always)]
  fn advance(&mut self, len: u32) -> Result<(), ()> { self.raw.advance(len) }

  #[inline(always)]
  unsafe fn unsafe_advance(&mut self, len: u32) { self.raw.unsafe_advance(len) }

  #[inline(always)]
  fn extend(&mut self, len: u32) -> Result<(), ()> { self.raw.extend(len) }

  #[inline(always)]
  unsafe fn unsafe_extend(&mut self, len: u32) { self.raw.unsafe_extend(len) }

  #[inline(always)]
  fn is_extended_by_ro<'a>(&self, other: &ROIobuf<'a>) -> bool { self.raw.is_extended_by(&other.raw) }

  #[inline(always)]
  fn is_extended_by_rw<'a>(&self, other: &RWIobuf<'a>) -> bool { self.raw.is_extended_by(&other.raw) }

  #[inline(always)]
  fn resize(&mut self, len: u32) -> Result<(), ()> { self.raw.resize(len) }

  #[inline(always)]
  unsafe fn unsafe_resize(&mut self, len: u32) { self.raw.unsafe_resize(len) }

  #[inline(always)]
  fn rewind(&mut self) { self.raw.rewind() }

  #[inline(always)]
  fn reset(&mut self) { self.raw.reset() }

  #[inline(always)]
  fn flip_lo(&mut self) { self.raw.flip_lo() }

  #[inline(always)]
  fn flip_hi(&mut self) { self.raw.flip_hi() }

  #[inline(always)]
  fn peek(&self, pos: u32, dst: &mut [u8]) -> Result<(), ()> { self.raw.peek(pos, dst) }
  #[inline(always)]
  fn peek_be<T: Prim>(&self, pos: u32) -> Result<T, ()> { self.raw.peek_be(pos) }
  #[inline(always)]
  fn peek_le<T: Prim>(&self, pos: u32) -> Result<T, ()> { self.raw.peek_le(pos) }

  #[inline(always)]
  fn consume(&mut self, dst: &mut [u8]) -> Result<(), ()> { self.raw.consume(dst) }
  #[inline(always)]
  fn consume_be<T: Prim>(&mut self) -> Result<T, ()> { self.raw.consume_be::<T>() }
  #[inline(always)]
  fn consume_le<T: Prim>(&mut self) -> Result<T, ()> { self.raw.consume_le::<T>() }

  #[inline(always)]
  fn check_range(&self, pos: u32, len: u32) -> Result<(), ()> { self.raw.check_range_u32(pos, len) }

  #[inline(always)]
  fn check_range_uint(&self, pos: u32, len: uint) -> Result<(), ()> { self.raw.check_range_uint(pos, len) }

  #[inline(always)]
  fn check_range_fail(&self, pos: u32, len: u32) { self.raw.check_range_u32_fail(pos, len) }

  #[inline(always)]
  fn check_range_uint_fail(&self, pos: u32, len: uint) { self.raw.check_range_uint_fail(pos, len) }

  #[inline(always)]
  unsafe fn unsafe_peek(&self, pos: u32, dst: &mut [u8]) { self.raw.unsafe_peek(pos, dst) }
  #[inline(always)]
  unsafe fn unsafe_peek_be<T: Prim>(&self, pos: u32) -> T { self.raw.unsafe_peek_be(pos) }
  #[inline(always)]
  unsafe fn unsafe_peek_le<T: Prim>(&self, pos: u32) -> T { self.raw.unsafe_peek_le(pos) }

  #[inline(always)]
  unsafe fn unsafe_consume(&mut self, dst: &mut [u8]) { self.raw.unsafe_consume(dst) }
  #[inline(always)]
  unsafe fn unsafe_consume_be<T: Prim>(&mut self) -> T { self.raw.unsafe_consume_be::<T>() }
  #[inline(always)]
  unsafe fn unsafe_consume_le<T: Prim>(&mut self) -> T { self.raw.unsafe_consume_le::<T>() }
}

impl<'a> Show for ROIobuf<'a> {
  fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
    self.raw.show(f, "read-only")
  }
}

impl<'a> Show for RWIobuf<'a> {
  fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
    self.raw.show(f, "read-write")
  }
}

/// A ring buffer implemented with `Iobuf`s.
pub struct IORingbuf {
  /// The contents of the window is space for input to be put into. Therefore,
  /// initially full.
  i_buf: RWIobuf<'static>,
  /// The contents of the window are things needing to be output. Therefore,
  /// initally empty.
  o_buf: RWIobuf<'static>,
}

impl IORingbuf {
  /// Creates a new ring buffer, with room for `cap` bytes.
  pub fn new(cap: uint) -> IORingbuf {
    let left_size = cap / 2;
    let mut ret =
      IORingbuf {
        i_buf: RWIobuf::new(left_size),
        o_buf: RWIobuf::new(cap - left_size),
      };
    ret.o_buf.flip_lo(); // start with an empty o_buf.
    ret
  }

  /// Returns an Iobuf, whose window may be filled with new data. This acts as
  /// the "push" operations for the ringbuf.
  ///
  /// It is easy to get garbage data if using a clone of the returned Iobuf.
  /// This is not memory-unsafe, but should be avoided.
  #[inline(always)]
  pub fn push_buf(&mut self) -> &mut RWIobuf<'static> {
    &mut self.i_buf
  }

  /// Returns an Iobuf, whose window may be have data `consume`d out of it. This
  /// acts as the "pop" operation for the ringbuf.
  ///
  /// After emptying out the returned Iobuf, it is not necessarily true that
  /// the ringbuf is empty. To truly empty out the ringbuf, you must pop
  /// Iobufs in a loop until `is_empty` returns `true`.
  ///
  /// It is easy to get garbage data if using a clone of the returned Iobuf.
  /// This is not memory-unsafe, but should be avoided.
  #[inline]
  pub fn pop_buf(&mut self) -> &mut ROIobuf<'static> {
    if self.o_buf.is_empty() {
      self.i_buf.flip_lo();
      self.o_buf.reset();
      mem::swap(&mut self.i_buf, &mut self.o_buf);
    }
    // Clients should only be doing read-only operations into the iobuf, so
    // return a ROIobuf.
    unsafe { mem::transmute(&mut self.o_buf) }
  }

  /// `true` if there is no data to pop in the Iobuf.
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.i_buf.cap() == self.i_buf.len() && self.o_buf.is_empty()
  }

  /// `true` if there is no room for new data in the Iobuf.
  #[inline(always)]
  pub fn is_full(&self) -> bool {
    self.i_buf.is_empty()
  }
}