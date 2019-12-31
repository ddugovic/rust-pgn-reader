// Counts classical games, moves and other tokens in PGNs.
// Usage: cargo run --release --example stats -- [PGN]...

use std::env;
use std::io;
use std::fs::File;

use btoi::btou;
use pgn_reader::{BufferedReader, RawComment, RawHeader, Visitor, Skip, SanPlus, Clock, Nag, Outcome};

#[derive(Debug, Default)]
struct Stats {
    classical: bool,
    time: u16,
    increment: u8,
    games: usize,
    headers: usize,
    sans: usize,
    nags: usize,
    comments: usize,
    clocks: usize,
    variations: usize,
    timeouts: usize,
    decisions: usize,
    outcomes: usize,
}

impl Stats {
    fn new() -> Stats {
        Stats::default()
    }
}

impl Visitor for Stats {
    type Result = ();

    fn begin_game(&mut self) {
        self.classical = false;
        self.time = 0;
        self.increment = 0;
    }

    fn header(&mut self, _key: &[u8], _value: RawHeader<'_>) {
        if _key == b"Event" {
            self.classical = _value.as_bytes() == b"Rated Classical game";
        }
        if self.classical {
            self.headers += 1;
            if _key == b"TimeControl" {
                let bytes: &[u8] = _value.as_bytes();
                if bytes[3] == b'+' {
                    self.time = 60 * btou::<u16>(&bytes[0..3]).ok().unwrap();
                    self.increment = btou(&bytes[4..]).ok().unwrap();
                } else {
                    self.time = 60 * btou::<u16>(&bytes[0..2]).ok().unwrap();
                    self.increment = btou(&bytes[3..]).ok().unwrap();
                }
            }
            if _key == b"Termination" && _value.as_bytes() == b"Time forfeit" {
                self.timeouts += 1;
            }
        }
    }

    fn end_headers(&mut self) -> Skip {
        Skip(!self.classical)
    }

    fn san(&mut self, _san: SanPlus) {
        self.sans += 1;
    }

    fn nag(&mut self, _nag: Nag) {
        self.nags += 1;
    }

    fn comment(&mut self, _comment: RawComment<'_>) {
        self.comments += 1;
        if Clock::from_ascii(_comment.as_bytes()).is_ok() {
            self.clocks += 1;
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
        if self.classical {
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
