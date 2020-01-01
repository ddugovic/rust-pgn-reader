// Counts non-bullet games, moves and other tokens in PGNs.
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
    time: u16,
    increment: u16,
    turns: u16,
    wclock: Clock,
    bclock: Clock,
    wlast: u16,
    blast: u16,
    timeout: bool,
}

impl Stats {
    fn new() -> Stats {
        Stats::default()
    }
}

impl Visitor for Stats {
    type Result = ();

    fn begin_game(&mut self) {
        self.turns = 0;
        self.time = 0;
        self.increment = 0;
        self.wclock = Clock::default();
        self.bclock = Clock::default();
        self.wlast = 0;
        self.blast = 0;
        self.timeout = false;
    }

    fn header(&mut self, _key: &[u8], _value: RawHeader<'_>) {
        self.headers += 1;
        if _key == b"TimeControl" {
            let bytes: &[u8] = _value.as_bytes();
            match bytes.iter().position(|&x| x == b'+') {
                Some(i) => {
                    self.time = btou(&bytes[0..i]).ok().unwrap();
                    self.increment = btou(&bytes[(i+1)..]).ok().unwrap();
                },
                _ => {}
            }
        }
        if self.time + 40 * self.increment >= 1500 {
            if _key == b"Termination" && _value.as_bytes() == b"Time forfeit" {
                self.timeout = true;
                self.timeouts += 1;
            }
        }
    }

    fn end_headers(&mut self) -> Skip {
        Skip((self.time + 40 * self.increment) < 1500 || !self.timeout)
    }

    fn san(&mut self, _san: SanPlus) {
        self.sans += 1;
        self.turns += 1;
    }

    fn nag(&mut self, _nag: Nag) {
        self.nags += 1;
    }

    fn comment(&mut self, _comment: RawComment<'_>) {
        self.comments += 1;
        let clock = Clock::from_ascii(_comment.as_bytes());
        if clock.is_ok() {
            if self.turns % 2 == 1 {
                let t = self.wclock.0 + self.increment;
                self.wclock = clock.ok().unwrap_or(Clock::default());
                if t > self.wclock.0 {
                    self.wlast = t - self.wclock.0;
                }
            } else {
                let t = self.bclock.0 + self.increment;
                self.bclock = clock.ok().unwrap_or(Clock::default());
                if t > self.bclock.0 {
                    self.blast = t - self.bclock.0;
                }
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
        if self.timeout {
            let t: u16 = (self.time + 40 * self.increment) / 12;
            let w: char = if self.turns % 2 == 0 && (t < self.wclock.0 || t < self.wlast) {'*'} else {' '};
            let b: char = if self.turns % 2 == 1 && (t < self.bclock.0 || t < self.blast) {'*'} else {' '};
            println!("{:3}+{:2} (t={:3}): {}wtime={:5} wlast={:3}  {}btime={:5} blast={:3}  turns={:3}", self.time/60, self.increment, t, w, self.wclock.0, self.wlast, b, self.bclock.0, self.blast, self.turns);
        }
    }

    fn end_game(&mut self) {
        if self.time + 40 * self.increment >= 1500 {
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
