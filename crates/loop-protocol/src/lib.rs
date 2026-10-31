#![no_std]
//! Bounded, acknowledged schedule transfer over CanopusConnect binary messages.
extern crate alloc;
use alloc::vec::Vec;

pub const MODULE_ID: &str = "loop";
pub const FRAME_MAX: usize = 480;
pub const HEADER: usize = 16;
pub const CHUNK_MAX: usize = FRAME_MAX - HEADER;
pub const SCHEDULE_MAX: usize = 40_960;
pub const HELLO: u8 = 1;
pub const BEGIN: u8 = 2;
pub const DATA: u8 = 3;
pub const COMMIT: u8 = 4;
pub const ABORT: u8 = 5;
const REPLY: u8 = 0x80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Ok = 0,
    Invalid = 1,
    TooLarge = 2,
    Sequence = 3,
    Checksum = 4,
    Schedule = 5,
    Io = 6,
    Busy = 7,
}

pub struct Frame<'a> {
    pub op: u8,
    pub status: Status,
    pub id: u32,
    pub value: u32,
    pub payload: &'a [u8],
    pub reply: bool,
}

impl<'a> Frame<'a> {
    pub fn decode(bytes: &'a [u8]) -> Option<Self> {
        if !(HEADER..=FRAME_MAX).contains(&bytes.len())
            || &bytes[..4] != b"LPC1"
            || bytes[6..8] != [0, 0]
        {
            return None;
        }
        let op = bytes[4] & !REPLY;
        if !(HELLO..=ABORT).contains(&op) {
            return None;
        }
        let status = match bytes[5] {
            0 => Status::Ok,
            1 => Status::Invalid,
            2 => Status::TooLarge,
            3 => Status::Sequence,
            4 => Status::Checksum,
            5 => Status::Schedule,
            6 => Status::Io,
            7 => Status::Busy,
            _ => return None,
        };
        Some(Self {
            op,
            status,
            id: u32::from_le_bytes(bytes[8..12].try_into().ok()?),
            value: u32::from_le_bytes(bytes[12..16].try_into().ok()?),
            payload: &bytes[HEADER..],
            reply: bytes[4] & REPLY != 0,
        })
    }
}

pub fn request(op: u8, id: u32, value: u32, payload: &[u8]) -> Vec<u8> {
    assert!(payload.len() <= CHUNK_MAX);
    let mut bytes = Vec::with_capacity(HEADER + payload.len());
    bytes.extend_from_slice(b"LPC1");
    bytes.extend_from_slice(&[op, 0, 0, 0]);
    bytes.extend_from_slice(&id.to_le_bytes());
    bytes.extend_from_slice(&value.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

struct Transfer {
    id: u32,
    size: usize,
    crc: u32,
    bytes: Vec<u8>,
}
#[derive(Default)]
pub struct Receiver {
    transfer: Option<Transfer>,
    committed: Option<(u32, u32)>,
}

impl Receiver {
    pub fn reset(&mut self) {
        self.transfer = None;
    }
    /// `commit` must validate and atomically persist bytes before returning a course count.
    pub fn handle(
        &mut self,
        bytes: &[u8],
        mut commit: impl FnMut(&[u8]) -> Result<u32, Status>,
    ) -> Option<Vec<u8>> {
        let frame = Frame::decode(bytes)?;
        if frame.reply || frame.status != Status::Ok {
            return None;
        }
        let (status, value) = self.apply(&frame, &mut commit);
        let mut response = request(frame.op | REPLY, frame.id, value, &[]);
        response[5] = status as u8;
        Some(response)
    }
    fn apply(
        &mut self,
        f: &Frame<'_>,
        commit: &mut impl FnMut(&[u8]) -> Result<u32, Status>,
    ) -> (Status, u32) {
        let result = match f.op {
            HELLO if f.payload.is_empty() && f.value == 0 => Ok(SCHEDULE_MAX as u32),
            BEGIN if f.payload.len() == 4 && f.id != 0 => {
                let size = f.value as usize;
                if size == 0 || size > SCHEDULE_MAX {
                    return (Status::TooLarge, 0);
                }
                let crc = u32::from_le_bytes(f.payload.try_into().unwrap());
                if let Some(t) = &self.transfer {
                    if t.id != f.id {
                        return (Status::Busy, 0);
                    }
                    if t.size != size || t.crc != crc {
                        return (Status::Sequence, 0);
                    }
                    return (Status::Ok, t.bytes.len() as u32);
                }
                // Reusing a completed transaction id must never replace its committed payload.
                if self.committed.is_some_and(|(id, _)| id == f.id) {
                    return (Status::Sequence, 0);
                }
                self.transfer = Some(Transfer {
                    id: f.id,
                    size,
                    crc,
                    bytes: Vec::with_capacity(size),
                });
                Ok(0)
            }
            DATA if !f.payload.is_empty() => {
                let Some(t) = self.transfer.as_mut().filter(|t| t.id == f.id) else {
                    return (Status::Sequence, 0);
                };
                let offset = f.value as usize;
                let Some(end) = offset.checked_add(f.payload.len()) else {
                    return (Status::Sequence, 0);
                };
                if end > t.size || offset > t.bytes.len() {
                    return (Status::Sequence, t.bytes.len() as u32);
                }
                if offset < t.bytes.len() {
                    if end > t.bytes.len() || &t.bytes[offset..end] != f.payload {
                        return (Status::Sequence, t.bytes.len() as u32);
                    }
                } else {
                    t.bytes.extend_from_slice(f.payload);
                }
                Ok(t.bytes.len() as u32)
            }
            COMMIT if f.payload.is_empty() && f.value == 0 => {
                if let Some((id, count)) = self.committed.filter(|(id, _)| *id == f.id) {
                    let _ = id;
                    return (Status::Ok, count);
                }
                let Some(t) = self.transfer.as_ref().filter(|t| t.id == f.id) else {
                    return (Status::Sequence, 0);
                };
                if t.bytes.len() != t.size {
                    return (Status::Sequence, t.bytes.len() as u32);
                }
                if crc32(&t.bytes) != t.crc {
                    self.reset();
                    return (Status::Checksum, 0);
                }
                match commit(&t.bytes) {
                    Ok(count) => {
                        self.committed = Some((f.id, count));
                        self.reset();
                        Ok(count)
                    }
                    Err(error) => {
                        self.reset();
                        Err(error)
                    }
                }
            }
            ABORT if f.payload.is_empty() && f.value == 0 => {
                if self.transfer.as_ref().is_some_and(|t| t.id == f.id) {
                    self.reset();
                }
                Ok(0)
            }
            _ => Err(Status::Invalid),
        };
        match result {
            Ok(value) => (Status::Ok, value),
            Err(error) => (error, 0),
        }
    }
}

pub enum Progress {
    Ignore,
    Send(Vec<u8>),
    Complete(u32),
}
pub struct Sender {
    id: u32,
    bytes: Vec<u8>,
    current: Vec<u8>,
    offset: usize,
    sent_end: usize,
}
impl Sender {
    pub fn new(id: u32, bytes: Vec<u8>) -> Result<Self, Status> {
        if id == 0 || bytes.is_empty() || bytes.len() > SCHEDULE_MAX {
            return Err(Status::TooLarge);
        }
        Ok(Self {
            id,
            bytes,
            current: request(HELLO, id, 0, &[]),
            offset: 0,
            sent_end: 0,
        })
    }
    pub fn current(&self) -> &[u8] {
        &self.current
    }
    pub fn on_reply(&mut self, bytes: &[u8]) -> Result<Progress, Status> {
        let Some(f) = Frame::decode(bytes) else {
            return Ok(Progress::Ignore);
        };
        let expected = self.current[4];
        if !f.reply || f.id != self.id || f.op != expected {
            return Ok(Progress::Ignore);
        }
        if f.status != Status::Ok {
            return Err(f.status);
        }
        if !f.payload.is_empty() {
            return Err(Status::Invalid);
        }
        let next = match f.op {
            HELLO => {
                if self.bytes.len() > f.value as usize {
                    return Err(Status::TooLarge);
                }
                request(
                    BEGIN,
                    self.id,
                    self.bytes.len() as u32,
                    &crc32(&self.bytes).to_le_bytes(),
                )
            }
            BEGIN => {
                if f.value as usize > self.bytes.len() {
                    return Err(Status::Sequence);
                }
                self.offset = f.value as usize;
                self.next_data()
            }
            DATA => {
                if f.value as usize <= self.offset {
                    return Ok(Progress::Ignore);
                }
                if f.value as usize != self.sent_end {
                    return Err(Status::Sequence);
                }
                self.offset = self.sent_end;
                self.next_data()
            }
            COMMIT => return Ok(Progress::Complete(f.value)),
            _ => return Err(Status::Invalid),
        };
        self.current = next.clone();
        Ok(Progress::Send(next))
    }
    fn next_data(&mut self) -> Vec<u8> {
        if self.offset == self.bytes.len() {
            return request(COMMIT, self.id, 0, &[]);
        }
        self.sent_end = (self.offset + CHUNK_MAX).min(self.bytes.len());
        request(
            DATA,
            self.id,
            self.offset as u32,
            &self.bytes[self.offset..self.sent_end],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    #[test]
    fn transfer_retries_and_ignores_other_sessions_without_double_commit() {
        let bytes = vec![0x5a; 2000];
        let mut sender = Sender::new(17, bytes.clone()).unwrap();
        let mut receiver = Receiver::default();
        let mut commits = 0;
        loop {
            let frame = sender.current().to_vec();
            assert!(frame.len() <= FRAME_MAX);
            let reply = receiver
                .handle(&frame, |data| {
                    assert_eq!(data, bytes);
                    commits += 1;
                    Ok(42)
                })
                .unwrap();
            let retry = receiver
                .handle(&frame, |_| {
                    commits += 1;
                    Ok(42)
                })
                .unwrap();
            assert_eq!(reply, retry);
            let mut other = reply.clone();
            other[8] ^= 1;
            assert!(matches!(sender.on_reply(&other), Ok(Progress::Ignore)));
            match sender.on_reply(&reply).unwrap() {
                Progress::Complete(count) => {
                    assert_eq!(count, 42);
                    break;
                }
                Progress::Send(_) => {
                    assert!(matches!(sender.on_reply(&reply), Ok(Progress::Ignore)))
                }
                Progress::Ignore => panic!("lost progress"),
            }
        }
        assert_eq!(commits, 1);
    }
    #[test]
    fn rejects_corruption_out_of_order_oversize_and_failed_commit() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        let mut r = Receiver::default();
        let mut send = |f: Vec<u8>| r.handle(&f, |_| Err(Status::Io)).unwrap();
        let status = |b: Vec<u8>| Frame::decode(&b).unwrap().status;
        assert_eq!(
            status(send(request(BEGIN, 1, SCHEDULE_MAX as u32 + 1, &[0; 4]))),
            Status::TooLarge
        );
        send(request(BEGIN, 1, 3, &crc32(b"abc").to_le_bytes()));
        assert_eq!(status(send(request(DATA, 1, 1, b"bc"))), Status::Sequence);
        assert_eq!(status(send(request(BEGIN, 2, 3, &[0; 4]))), Status::Busy);
        send(request(DATA, 1, 0, b"abd"));
        assert_eq!(status(send(request(COMMIT, 1, 0, &[]))), Status::Checksum);
        send(request(BEGIN, 2, 3, &crc32(b"abc").to_le_bytes()));
        send(request(DATA, 2, 0, b"abc"));
        assert_eq!(status(send(request(COMMIT, 2, 0, &[]))), Status::Io);
        send(request(BEGIN, 3, 3, &[0; 4]));
        send(request(ABORT, 3, 0, &[]));
        assert_eq!(status(send(request(DATA, 3, 0, b"abc"))), Status::Sequence);
    }
}
