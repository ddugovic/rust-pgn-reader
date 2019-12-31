// This file is part of the pgn-reader library.
// Copyright (C) 2017-2018 Niklas Fiekas <niklas.fiekas@backscattering.de>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::str::{self, FromStr, Utf8Error};

/// Tell the reader to skip over a game or variation.
#[derive(Clone, Eq, PartialEq, Debug)]
#[must_use]
pub struct Skip(pub bool);

/// A clock comment such as [%clk 0:01:00].
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Clock(pub u8);

impl Clock {
    /// Tries to parse a Clock time from ASCII.
    ///
    /// # Examples
    ///
    /// ```
    /// use pgn_reader::Clock;
    ///
    /// assert_eq!(Clock::from_ascii(b" [%clk 0:01:00] "), Ok(Clock(60)));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`InvalidClock`] error if the input is not a clock time.
    ///
    ///
    /// [`InvalidClock`]: struct.InvalidClock.html
    pub fn from_ascii(s: &[u8]) -> Result<Clock, InvalidClock> {
        if &s[0..7] == b" [%clk " {
            btoi::btou(&s[12..13]).ok().map(Clock).ok_or(InvalidClock { _priv: () })
        } else {
            Err(InvalidClock { _priv: () })
        }
    }

    pub const ZERO: Clock = Clock(0);
}

impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

impl From<u8> for Clock {
    fn from(clk: u8) -> Clock {
        Clock(clk)
    }
}

/// Error when parsing an invalid Clock.
#[derive(Clone, Eq, PartialEq)]
pub struct InvalidClock {
    _priv: (),
}

impl fmt::Debug for InvalidClock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InvalidClock").finish()
    }
}

impl fmt::Display for InvalidClock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        "invalid clk".fmt(f)
    }
}

impl Error for InvalidClock {
    fn description(&self) -> &str {
        "invalid clk"
    }
}

impl FromStr for Clock {
    type Err = InvalidClock;

    fn from_str(s: &str) -> Result<Clock, InvalidClock> {
        Clock::from_ascii(s.as_bytes())
    }
}

/// A numeric annotation glyph like `?`, `!!` or `$42`.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Nag(pub u8);

impl Nag {
    /// Tries to parse a NAG from ASCII.
    ///
    /// # Examples
    ///
    /// ```
    /// use pgn_reader::Nag;
    ///
    /// assert_eq!(Nag::from_ascii(b"??"), Ok(Nag(4)));
    /// assert_eq!(Nag::from_ascii(b"$24"), Ok(Nag(24)));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`InvalidNag`] error if the input is neither a known glyph
    /// (`?!`, `!`, ...) nor a valid numeric annotation (`$0`, ..., `$255`).
    ///
    ///
    /// [`InvalidNag`]: struct.InvalidNag.html
    pub fn from_ascii(s: &[u8]) -> Result<Nag, InvalidNag> {
        if s == b"?!" {
            Ok(Nag::DUBIOUS_MOVE)
        } else if s == b"?" {
            Ok(Nag::MISTAKE)
        } else if s == b"??" {
            Ok(Nag::BLUNDER)
        } else if s == b"!" {
            Ok(Nag::GOOD_MOVE)
        } else if s == b"!!" {
            Ok(Nag::BRILLIANT_MOVE)
        } else if s == b"!?" {
            Ok(Nag::SPECULATIVE_MOVE)
        } else if s.len() > 1 && s[0] == b'$' {
            btoi::btou(&s[1..]).ok().map(Nag).ok_or(InvalidNag { _priv: () })
        } else {
            Err(InvalidNag { _priv: () })
        }
    }

    /// A good move (`!`).
    pub const GOOD_MOVE: Nag = Nag(1);

    /// A mistake (`?`).
    pub const MISTAKE: Nag = Nag(2);

    /// A brilliant move (`!!`).
    pub const BRILLIANT_MOVE: Nag = Nag(3);

    /// A blunder (`??`).
    pub const BLUNDER: Nag = Nag(4);

    /// A speculative move (`!?`).
    pub const SPECULATIVE_MOVE: Nag = Nag(5);

    /// A dubious move (`?!`).
    pub const DUBIOUS_MOVE: Nag = Nag(6);
}

impl fmt::Display for Nag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

impl From<u8> for Nag {
    fn from(nag: u8) -> Nag {
        Nag(nag)
    }
}

/// Error when parsing an invalid NAG.
#[derive(Clone, Eq, PartialEq)]
pub struct InvalidNag {
    _priv: (),
}

impl fmt::Debug for InvalidNag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InvalidNag").finish()
    }
}

impl fmt::Display for InvalidNag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        "invalid nag".fmt(f)
    }
}

impl Error for InvalidNag {
    fn description(&self) -> &str {
        "invalid nag"
    }
}

impl FromStr for Nag {
    type Err = InvalidNag;

    fn from_str(s: &str) -> Result<Nag, InvalidNag> {
        Nag::from_ascii(s.as_bytes())
    }
}

/// A header value.
///
/// Provides helper methods for decoding [backslash
/// escaped](http://www.saremba.de/chessgml/standards/pgn/pgn-complete.htm#c7)
/// values.
///
/// > A quote inside a string is represented by the backslash immediately
/// > followed by a quote. A backslash inside a string is represented by
/// > two adjacent backslashes.
#[derive(Clone, Eq, PartialEq)]
pub struct RawHeader<'a>(pub &'a[u8]);

impl<'a> RawHeader<'a> {
    /// Returns the raw byte representation of the header value.
    pub fn as_bytes(&self) -> &[u8] {
        self.0
    }

    /// Decodes escaped quotes and backslashes into bytes. Allocates only when
    /// the value actually contains escape sequences.
    pub fn decode(&self) -> Cow<'a, [u8]> {
        let mut head = 0;
        let mut decoded: Vec<u8> = Vec::new();
        for escape in memchr::memchr_iter(b'\\', self.0) {
            match self.0.get(escape + 1).cloned() {
                Some(ch) if ch == b'\\' || ch == b'"' => {
                    decoded.extend_from_slice(&self.0[head..escape]);
                    head = escape + 1;
                }
                _ => (),
            }
        }
        if head == 0 {
            Cow::Borrowed(self.0)
        } else {
            decoded.extend_from_slice(&self.0[head..]);
            Cow::Owned(decoded)
        }
    }

    /// Tries to decode the header as UTF-8. This is guaranteed to succeed on
    /// valid PGNs.
    ///
    /// # Errors
    ///
    /// Errors if the header contains an invalid UTF-8 byte sequence.
    pub fn decode_utf8(&self) -> Result<Cow<'a, str>, Utf8Error> {
        Ok(match self.decode() {
            Cow::Borrowed(borrowed) => Cow::Borrowed(str::from_utf8(borrowed)?),
            Cow::Owned(owned) => Cow::Owned(String::from_utf8(owned).map_err(|e| e.utf8_error())?),
        })
    }

    /// Decodes the header as UTF-8, replacing any invalid byte sequences with
    /// the placeholder � U+FFFD.
    pub fn decode_utf8_lossy(&self) -> Cow<'a, str> {
        match self.decode() {
            Cow::Borrowed(borrowed) => String::from_utf8_lossy(borrowed),
            Cow::Owned(owned) => Cow::Owned(String::from_utf8_lossy(&owned).into_owned()),
        }
    }
}

impl<'a> fmt::Debug for RawHeader<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.decode_utf8_lossy())
    }
}

/// A comment, excluding the braces.
#[derive(Clone, Eq, PartialEq)]
pub struct RawComment<'a>(pub &'a [u8]);

impl<'a> RawComment<'a> {
    /// Returns the raw byte representation of the comment.
    pub fn as_bytes(&self) -> &[u8] {
        self.0
    }

    /// Decodes escaped quotes and backslashes into bytes. Allocates only when
    /// the value actually contains escape sequences.
    pub fn decode(&self) -> Cow<'a, [u8]> {
        let mut comment = 0;
        let mut decoded: Vec<u8> = Vec::new();
        for escape in memchr::memchr_iter(b'\\', self.0) {
            match self.0.get(escape + 1).cloned() {
                Some(ch) if ch == b'\\' || ch == b'"' => {
                    decoded.extend_from_slice(&self.0[comment..escape]);
                    comment = escape + 1;
                }
                _ => (),
            }
        }
        if comment == 0 {
            Cow::Borrowed(self.0)
        } else {
            decoded.extend_from_slice(&self.0[comment..]);
            Cow::Owned(decoded)
        }
    }

    /// Tries to decode the comment as UTF-8. This is guaranteed to succeed on
    /// valid PGNs.
    ///
    /// # Errors
    ///
    /// Errors if the comment contains an invalid UTF-8 byte sequence.
    pub fn decode_utf8(&self) -> Result<Cow<'a, str>, Utf8Error> {
        Ok(match self.decode() {
            Cow::Borrowed(borrowed) => Cow::Borrowed(str::from_utf8(borrowed)?),
            Cow::Owned(owned) => Cow::Owned(String::from_utf8(owned).map_err(|e| e.utf8_error())?),
        })
    }

    /// Decodes the comment as UTF-8, replacing any invalid byte sequences with
    /// the placeholder � U+FFFD.
    pub fn decode_utf8_lossy(&self) -> Cow<'a, str> {
        match self.decode() {
            Cow::Borrowed(borrowed) => String::from_utf8_lossy(borrowed),
            Cow::Owned(owned) => Cow::Owned(String::from_utf8_lossy(&owned).into_owned()),
        }
    }
}

impl<'a> fmt::Debug for RawComment<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(self.as_bytes()).as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock() {
        assert_eq!(Clock::from_ascii(b" [%clk 0:01:00] "), Ok(Clock(60)));
    }

    #[test]
    fn test_nag() {
        assert_eq!(Nag::from_ascii(b"$33"), Ok(Nag(33)));
    }

    #[test]
    fn test_raw_comment() {
        let comment = RawHeader(b"Hello world");
        assert_eq!(comment.decode().as_ref(), b"Hello world");

        let comment = RawHeader(b"Hello \\world\\");
        assert_eq!(comment.decode().as_ref(), b"Hello \\world\\");

        let comment = RawHeader(b"\\Hello \\\"world\\\\");
        assert_eq!(comment.decode().as_ref(), b"\\Hello \"world\\");
    }

    #[test]
    fn test_raw_header() {
        let header = RawHeader(b"Hello world");
        assert_eq!(header.decode().as_ref(), b"Hello world");

        let header = RawHeader(b"Hello \\world\\");
        assert_eq!(header.decode().as_ref(), b"Hello \\world\\");

        let header = RawHeader(b"\\Hello \\\"world\\\\");
        assert_eq!(header.decode().as_ref(), b"\\Hello \"world\\");
    }
}
