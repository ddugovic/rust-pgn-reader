// Counts standard games, moves and other tokens in PGNs.
// Usage: cargo run --release --example stats -- [PGN]...

use std::env;
use std::io;
use std::fs::File;

use btoi::btou;
use pgn_reader::{BufferedReader, RawComment, RawHeader, Visitor, Skip, SanPlus, Clock, Nag, Outcome};

#[derive(Debug, Default)]
struct Stats {
    headers: usize,
    games: usize,
    sans: usize,
    nags: usize,
    comments: usize,
    variations: usize,
    timeouts: usize,
    decisions: usize,
    outcomes: usize,
    standard: bool,
    time: u16,
    increment: u16,
    clock1: Clock,
    clock2: Clock,
}

impl Stats {
    fn new() -> Stats {
        Stats::default()
    }
}

impl Visitor for Stats {
    type Result = ();

    fn begin_game(&mut self) {
        self.standard = true;
        self.time = 0;
        self.increment = 0;
    }

    fn header(&mut self, _key: &[u8], _value: RawHeader<'_>) {
        self.headers += 1;
        if _key == b"Variant" {
            self.standard = _value.as_bytes() == b"Standard";
        }
        if self.standard {
            if _key == b"TimeControl" {
                let bytes: &[u8] = _value.as_bytes();
                if bytes.len() > 1 {
                    if bytes[1] == b'+' {
                        self.time = btou(&bytes[0..1]).ok().unwrap();
                        self.increment = btou(&bytes[2..]).ok().unwrap();
                    } else if bytes[2] == b'+' {
                        self.time = btou(&bytes[0..2]).ok().unwrap();
                        self.increment = btou(&bytes[3..]).ok().unwrap();
                    } else if bytes[3] == b'+' {
                        self.time = btou(&bytes[0..3]).ok().unwrap();
                        self.increment = btou(&bytes[4..]).ok().unwrap();
                    }
                    self.clock1 = Clock(self.time);
                    self.clock2 = Clock(self.time);
                }
            }
            if self.time + 40 * self.increment >= 180 {
                if _key == b"Termination" && _value.as_bytes() == b"Time forfeit" {
                    self.timeouts += 1;
                }
            }
        }
    }

    fn end_headers(&mut self) -> Skip {
        Skip((self.time + 40 * self.increment) < 180 || !self.standard)
    }

    fn san(&mut self, _san: SanPlus) {
        self.sans += 1;
    }

    fn nag(&mut self, _nag: Nag) {
        self.nags += 1;
    }

    fn comment(&mut self, _comment: RawComment<'_>) {
        self.comments += 1;
        let clock = Clock::from_ascii(_comment.as_bytes());
        if clock.is_ok() {
            if self.sans % 2 == 0 {
                self.clock1 = clock.ok().unwrap_or(Clock::default());
            } else {
                self.clock2 = clock.ok().unwrap_or(Clock::default());
            }
        }
    }

    fn end_variation(&mut self) {
        self.variations += 1;
    }

    fn outcome(&mut self, _outcome: Option<Outcome>) {
        self.outcomes += 1;
        self.decisions += match _outcome {
            None => 0,
            Some(x) => match x.winner() {
                None => 0,
                Some(_y) => 1
            }
        };
    }

    fn end_game(&mut self) {
        if self.time + 40 * self.increment >= 180 && self.standard {
            self.games += 1;
        }
    }
}

fn main() -> Result<(), io::Error> {
    for arg in env::args().skip(1) {
        let file = File::open(&arg).expect("fopen");

        let uncompressed: Box<dyn io::Read> = if arg.ends_with(".bz2") {
            Box::new(bzip2::read::BzDecoder::new(file))
        } else if arg.ends_with(".xz") {
            Box::new(xz2::read::XzDecoder::new(file))
        } else if arg.ends_with(".gz") {
            Box::new(flate2::read::GzDecoder::new(file))
        } else if arg.ends_with(".lz4") {
            Box::new(lz4::Decoder::new(file)?)
        } else {
            Box::new(file)
        };

        let mut reader = BufferedReader::new(uncompressed);

        let mut stats = Stats::new();
        reader.read_all(&mut stats)?;
        println!("{}: {:?}", arg, stats);
    }

    Ok(())
}
