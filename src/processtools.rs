use std::io;
use std::io::{Read, Write, BufRead, BufReader};
use std::collections::VecDeque;
use std::collections::vec_deque::IntoIter as VecDequeIntoIter;
use std::process::{Child, Command, Stdio, ExitStatus};
use std::os::unix::process::ExitStatusExt;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

use error::Error;


pub enum Chunk {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
}

pub struct ProcessIterator {
    rx: Receiver<Option<Chunk>>,
}

pub struct LineBuffer {
    max_lines: usize,
    lines: VecDeque<String>,
}


fn reader_proc(child: &mut Child) -> Receiver<Option<Chunk>> {
    fn read<R, F>(readable: Option<R>, tx: Sender<Option<Chunk>>,
                  wrap: F)
    where
        R: Send + 'static + Read,
        F: Send + 'static + Fn(Vec<u8>) -> Chunk
    {
        if let Some(mut r) = readable {
            thread::spawn(move || {
                loop {
                    let mut chunk = [0u8; 16384];
                    match r.read(&mut chunk[..]) {
                        Ok(0) | Err(_)  => break,
                        Ok(len)  => {
                            let _ = tx.send(Some(wrap(chunk[..len].to_vec())));
                        }
                    }
                }
            });
        } else {
            let _ = tx.send(None);
        }
    };

    let (tx, rx) = channel();
    read(child.stdout.take(), tx.clone(), |l| Chunk::Stdout(l));
    read(child.stderr.take(), tx.clone(), |l| Chunk::Stderr(l));
    rx
}


impl Chunk {
    pub fn bytes(&self) -> &[u8] {
        match *self {
            Chunk::Stdout(ref data) => data,
            Chunk::Stderr(ref data) => data,
        }
    }

    pub fn echo(&self) -> Result<usize, io::Error> {
        match *self {
            Chunk::Stdout(ref data) => io::stdout().write(data),
            Chunk::Stderr(ref data) => io::stderr().write(data)
        }
    }
}

impl ProcessIterator {
    pub fn new(child: &mut Child) -> ProcessIterator {
        ProcessIterator {
            rx: reader_proc(child)
        }
    }
}

impl Iterator for ProcessIterator {
    type Item = Chunk;

    fn next(&mut self) -> Option<Chunk> {
        match self.rx.recv() {
            Ok(line) => line,
            _  => None,
        }
    }
}


pub type LineBufferIntoIter = VecDequeIntoIter<String>;

impl LineBuffer {
    pub fn new(max_lines: usize) -> LineBuffer {
        LineBuffer {
            max_lines: max_lines,
            lines: VecDeque::new(),
        }
    }

    pub fn append_chunk(&mut self, chunk: &Chunk) {
        let mut rdr = BufReader::new(chunk.bytes());

        // this technically is not entirley correct.  It assumes that we
        // will be fed complete lines which is currently not enforced
        for line_rv in rdr.lines() {
            if let Ok(line) = line_rv {
                self.lines.push_back(line);
                if self.lines.len() > self.max_lines {
                    self.lines.pop_front();
                }
            }
        }
    }
}

impl IntoIterator for LineBuffer {
    type Item = String;
    type IntoIter = LineBufferIntoIter;

    fn into_iter(self) -> LineBufferIntoIter {
        self.lines.into_iter()
    }
}


pub fn get_unix_exit_status(status: ExitStatus) -> Option<i32> {
    status.code().or_else(|| status.signal().map(|x| x + 127))
}
