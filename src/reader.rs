use super::{Nag, Outcome, RawHeader, Skip, San};
use std::cmp::min;
use std::io;
use std::io::Read;
use std::ptr;
use slice_deque::SliceDeque;

pub trait Visitor {
    type Result;

    fn begin_game(&mut self) { }

    fn begin_headers(&mut self) { }
    fn header(&mut self, _key: &[u8], _value: RawHeader<'_>) { }
    fn end_headers(&mut self) -> Skip { Skip(false) }

    fn san(&mut self, _san: San) { }
    fn nag(&mut self, _nag: Nag) { }
    fn comment(&mut self, _comment: &[u8]) { }
    fn begin_variation(&mut self) -> Skip { Skip(false) }
    fn end_variation(&mut self) { }
    fn outcome(&mut self, _outcome: Outcome) { }

    fn end_game(&mut self) -> Self::Result;
}

struct SkipVisitor;

impl Visitor for SkipVisitor {
    type Result = ();

    fn end_headers(&mut self) -> Skip { Skip(true) }
    fn begin_variation(&mut self) -> Skip { Skip(true) }
    fn end_game(&mut self) { }
}

const MIN_BUFFER_SIZE: usize = 8192;

trait ReadPgn {
    type Err;

    fn fill_buffer(&mut self) -> Result<bool, Self::Err>;
    fn buffer(&self) -> &[u8];
    fn consume(&mut self, bytes: usize);

    fn peek(&self) -> Option<u8> {
        self.buffer().get(0).cloned()
    }

    fn bump(&mut self) -> Option<u8> {
        let head = self.peek();
        if head.is_some() {
            self.consume(1);
        }
        head
    }

    fn remaining(&self) -> usize {
        self.buffer().len()
    }

    fn consume_all(&mut self) {
        let remaining = self.remaining();
        self.consume(remaining);
    }

    fn skip_bom(&mut self) -> Result<(), Self::Err> {
        if self.fill_buffer()? && self.buffer().starts_with(b"\xef\xbb\xbf") {
            self.consume(3);
        }
        Ok(())
    }

    fn skip_until(&mut self, needle: u8) -> Result<(), Self::Err> {
        while self.fill_buffer()? {
            if let Some(pos) = memchr::memchr(needle, self.buffer()) {
                self.consume(pos);
                return Ok(());
            } else {
                self.consume_all();
            }
        }

        Ok(())
    }

    fn skip_line(&mut self) -> Result<(), Self::Err> {
        self.skip_until(b'\n')?;
        self.bump();
        Ok(())
    }

    fn skip_whitespace(&mut self) -> Result<(), Self::Err> {
        while self.fill_buffer()? {
            while let Some(ch) = self.peek() {
                match ch {
                    b' ' | b'\t' | b'\r' | b'\n' => {
                        self.bump();
                    },
                    b'%' => {
                        self.bump();
                        self.skip_line()?;
                    },
                    _ => return Ok(()),
                }
            }
        }

        Ok(())
    }

    fn skip_ket(&mut self) -> Result<(), Self::Err> {
        while self.fill_buffer()? {
            while let Some(ch) = self.peek() {
                match ch {
                    b' ' | b'\t' | b'\r' | b']' => {
                        self.bump();
                    },
                    b'%' => {
                        self.bump();
                        self.skip_line();
                        return Ok(());
                    },
                    b'\n' => {
                        self.bump();
                        return Ok(());
                    },
                    _ => {
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    fn read_headers<V: Visitor>(&mut self, visitor: &mut V) -> Result<(), Self::Err> {
        while self.fill_buffer()? {
            if let Some(ch) = self.peek() {
                match ch {
                    b'[' => {
                        self.bump();

                        let left_quote = match memchr::memchr2(b'"', b'\n', self.buffer()) {
                            Some(left_quote) if self.buffer()[left_quote] == b'"' => left_quote,
                            Some(eol) => {
                                visitor.header(&self.buffer()[..eol], RawHeader(b""));
                                self.consume(eol + 1);
                                continue;
                            },
                            None => {
                                self.skip_line()?;
                                continue;
                            }
                        };

                        let space = if left_quote > 0 && self.buffer()[left_quote - 1] == b' ' {
                            left_quote - 1
                        } else {
                            left_quote
                        };

                        let value_start = left_quote + 1;
                        let mut right_quote = value_start;
                        let consumed = loop {
                            match memchr::memchr3(b'\\', b'"', b'\n', &self.buffer()[right_quote..]) {
                                Some(delta) if self.buffer()[right_quote + delta] == b'"' => {
                                    right_quote += delta;
                                    break right_quote + 1;
                                }
                                Some(delta) if self.buffer()[right_quote + delta] == b'\n' => {
                                    right_quote += delta;
                                    break right_quote;
                                }
                                Some(delta) => {
                                    // Skip escaped character.
                                    right_quote = min(right_quote + delta + 2, self.remaining());
                                },
                                None => {
                                    right_quote = self.remaining();
                                    break right_quote;
                                }
                            }
                        };

                        visitor.header(&self.buffer()[..space], RawHeader(&self.buffer()[value_start..right_quote]));
                        self.consume(consumed);
                        self.skip_ket()?;
                    },
                    b'%' => self.skip_line()?,
                    _ => return Ok(()),
                }
            }
        }

        Ok(())
    }

    fn skip_movetext(&mut self) -> Result<(), Self::Err> {
        while self.fill_buffer()? {
            if let Some(ch) = self.bump() {
                match ch {
                    b'{' => {
                        self.skip_until(b'}')?;
                        self.bump();
                    },
                    b';' => {
                        self.skip_until(b'\n')?;
                    },
                    b'\n' => {
                        match self.peek() {
                            Some(b'%') => self.skip_until(b'\n')?,
                            Some(b'\n') | Some(b'[') => break,
                            Some(b'\r') => {
                                self.bump();
                                if let Some(b'\n') = self.peek() {
                                    break;
                                }
                            }
                            _ => continue,
                        }
                    },
                    _ => {
                        let consumed = memchr::memchr3(b'\n', b'{', b';', self.buffer()).unwrap_or_else(|| self.remaining());
                        self.consume(consumed);
                    }
                }
            }
        }

        Ok(())
    }

    fn read_game<V: Visitor>(&mut self, visitor: &mut V) -> Result<Option<V::Result>, Self::Err> {
        self.skip_bom()?;
        self.skip_whitespace()?;

        if !self.fill_buffer()? {
            return Ok(None);
        }

        visitor.begin_game();
        visitor.begin_headers();
        self.read_headers(visitor)?;
        if let Skip(false) = visitor.end_headers() {
            //self.skip_until(b'\n')?;
            self.skip_movetext()?;
        } else {
            self.skip_movetext()?;
        }

        self.skip_whitespace()?;
        Ok(Some(visitor.end_game()))
    }

    fn skip_game(&mut self) -> Result<bool, Self::Err> {
        self.read_game(&mut SkipVisitor).map(|r| r.is_some())
    }
}

pub struct PgnReader<R> {
    inner: R,
    buffer: SliceDeque<u8>,
}

impl<R: Read> PgnReader<R> {
    pub fn new(inner: R) -> PgnReader<R> {
        PgnReader {
            inner,
            buffer: SliceDeque::with_capacity(MIN_BUFFER_SIZE * 2),
        }
    }

    pub fn read_game<V: Visitor>(&mut self, visitor: &mut V) -> io::Result<Option<V::Result>> {
        ReadPgn::read_game(self, visitor)
    }

    pub fn skip_game<V: Visitor>(&mut self) -> io::Result<bool> {
        ReadPgn::skip_game(self)
    }
}

impl<R> PgnReader<R> {
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> ReadPgn for PgnReader<R> {
    type Err = io::Error;

    fn fill_buffer(&mut self) -> io::Result<bool> {
        while self.buffer.len() < MIN_BUFFER_SIZE {
            unsafe {
                let size = {
                    let remainder = self.buffer.tail_head_slice();
                    ptr::write_bytes(remainder.as_mut_ptr(), 0, remainder.len()); // TODO
                    self.inner.read(remainder)?
                };

                if size == 0 {
                    break;
                }

                self.buffer.move_tail(size as isize);
            }
        }

        Ok(!self.buffer.is_empty())
    }

    fn buffer(&self) -> &[u8] {
        self.buffer.as_slice()
    }

    fn consume(&mut self, bytes: usize) {
        // TODO: Safety argument.
        unsafe { self.buffer.move_head(bytes as isize); }
    }

    fn consume_all(&mut self) {
        self.buffer.clear();
    }

    fn bump(&mut self) -> Option<u8> {
        self.buffer.pop_front()
    }

    fn peek(&self) -> Option<u8> {
        self.buffer.front().cloned()
    }
}

pub struct SliceReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> SliceReader<'a> {
    pub fn new(bytes: &'a [u8]) -> SliceReader<'a> {
        SliceReader {
            bytes,
            pos: 0,
        }
    }

    pub fn read_game<V: Visitor>(&mut self, visitor: &mut V) -> Option<V::Result> {
        ReadPgn::read_game(self, visitor).unwrap_or_else(|_| unreachable!())
    }
}

enum Never { }

impl<'a> ReadPgn for SliceReader<'a> {
    type Err = Never;

    fn fill_buffer(&mut self) -> Result<bool, Self::Err> {
        Ok(self.pos < self.bytes.len())
    }

    fn buffer(&self) -> &[u8] {
        &self.bytes[self.pos..]
    }

    fn consume(&mut self, bytes: usize) {
        self.pos += bytes;
        debug_assert!(self.pos <= self.bytes.len());
    }
}