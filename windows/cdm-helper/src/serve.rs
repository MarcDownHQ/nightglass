//! The helper's loop: requests from stdin, the CDM's timers, and every answer and event to stdout. The CDM is only
//! ever called from this thread.

use std::collections::HashMap;
use std::io::{BufReader, Stdout, Write};
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nightglass_drm::protocol::{self, Envelope, Failure, FailureKind, Key, Message, MessageKind, Reply, Request, Sample, Scheme};

use crate::ffi::{Pattern, SubsampleEntry, scheme, status};
use crate::host::{Callbacks, Deferred, Encrypted, Host, Instance, KeyStatus, Module};

pub fn run(module: Module) -> ExitCode {
    let events = Arc::new(Events { out: Mutex::new(std::io::stdout()), state: Mutex::default() });
    let host = Box::leak(Host::new(Box::new(Relay(events.clone()))));
    let instance = match module.create(host) {
        Ok(instance) => instance,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    events.send(&Message::Ready { protocol: protocol::PROTOCOL, cdm_version: module.version().to_string() }, &[]);

    let (requests, incoming) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stdin = BufReader::new(std::io::stdin().lock());
        while let Ok(Some(frame)) = protocol::read_frame::<Envelope>(&mut stdin) {
            if requests.send(frame).is_err() {
                return;
            }
        }
    });

    let mut promises = 0u32;
    loop {
        events.run_due(&instance);
        let wait = events.next_timer().map_or(Duration::from_secs(3600), |at| at.saturating_duration_since(Instant::now()));
        let (envelope, payload) = match incoming.recv_timeout(wait) {
            Ok(frame) => frame,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return ExitCode::SUCCESS,
        };
        events.run_due(&instance);
        let id = envelope.id;
        let mut promise = || {
            promises = promises.wrapping_add(1).max(1);
            lock(&events.state).promises.insert(promises, id);
            promises
        };
        match envelope.request {
            Request::Initialize { distinctive_identifier, persistent_state } => {
                lock(&events.state).initialize = Some(id);
                instance.initialize(distinctive_identifier, persistent_state);
            }
            Request::SetServerCertificate => instance.set_server_certificate(promise(), &payload),
            Request::CreateSession => instance.create_session(promise(), &payload),
            Request::UpdateSession { session } => instance.update_session(promise(), &session, &payload),
            Request::CloseSession { session } => instance.close_session(promise(), &session),
            Request::Decrypt { samples } => match decrypt(&instance, &events, &samples, &payload) {
                Ok(clear) => events.send(&Message::Reply { id, reply: Reply::Decrypted }, &clear),
                Err(failure) => events.send(&Message::Failed { id, failure }, &[]),
            },
        }
    }
}

struct Events {
    out: Mutex<Stdout>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    timers: Vec<(Instant, usize)>,
    promises: HashMap<u32, u64>,
    initialize: Option<u64>,
    deferred: Vec<Deferred>,
}

impl Events {
    fn send(&self, message: &Message, payload: &[u8]) {
        let mut out = lock(&self.out);
        if protocol::write_frame(&mut *out, message, payload).and_then(|_| out.flush()).is_err() {
            std::process::exit(0);
        }
    }

    fn reply(&self, promise: u32, reply: Reply) {
        if let Some(id) = lock(&self.state).promises.remove(&promise) {
            self.send(&Message::Reply { id, reply }, &[]);
        }
    }

    fn next_timer(&self) -> Option<Instant> {
        lock(&self.state).timers.iter().map(|(at, _)| *at).min()
    }

    /// Timers that are due and answers the CDM asked for, until none is left: each may ask for more.
    fn run_due(&self, instance: &Instance) {
        loop {
            let now = Instant::now();
            let (due, deferred) = {
                let mut state = lock(&self.state);
                let (due, later): (Vec<_>, Vec<_>) = state.timers.drain(..).partition(|(at, _)| *at <= now);
                state.timers = later;
                (due, std::mem::take(&mut state.deferred))
            };
            if due.is_empty() && deferred.is_empty() {
                return;
            }
            for call in deferred {
                instance.answer(call);
            }
            for (_, context) in due {
                instance.timer_expired(context);
            }
        }
    }
}

struct Relay(Arc<Events>);

impl Callbacks for Relay {
    fn set_timer(&self, delay_ms: i64, context: usize) {
        let at = Instant::now() + Duration::from_millis(delay_ms.max(0) as u64);
        lock(&self.0.state).timers.push((at, context));
    }

    fn initialized(&self, success: bool) {
        let Some(id) = lock(&self.0.state).initialize.take() else { return };
        if success {
            self.0.send(&Message::Reply { id, reply: Reply::Initialized }, &[]);
        } else {
            let failure = Failure { kind: FailureKind::Initialization, message: "the CDM did not initialise".into(), system_code: None, sample: None, kid: None };
            self.0.send(&Message::Failed { id, failure }, &[]);
        }
    }

    fn new_session(&self, promise: u32, session: String) {
        self.0.reply(promise, Reply::Session { session });
    }

    fn resolved(&self, promise: u32) {
        self.0.reply(promise, Reply::Done);
    }

    fn rejected(&self, promise: u32, exception: u32, system_code: u32, message: String) {
        let Some(id) = lock(&self.0.state).promises.remove(&promise) else { return };
        let exception = match exception {
            0 => "TypeError",
            1 => "NotSupportedError",
            2 => "InvalidStateError",
            3 => "QuotaExceededError",
            _ => "error",
        };
        let message = if message.is_empty() { exception.to_string() } else { format!("{exception}: {message}") };
        let failure = Failure { kind: FailureKind::Rejected, message, system_code: Some(system_code), sample: None, kid: None };
        self.0.send(&Message::Failed { id, failure }, &[]);
    }

    fn message(&self, session: String, kind: u32, message: Vec<u8>) {
        self.0.send(&Message::SessionMessage { session, kind: MessageKind::from_cdm(kind) }, &message);
    }

    fn keys(&self, session: String, keys: Vec<KeyStatus>) {
        let keys = keys.into_iter().map(|k| Key { kid: k.kid, status: k.status, system_code: k.system_code }).collect();
        self.0.send(&Message::Keys { session, keys }, &[]);
    }

    fn expiration(&self, session: String, time: f64) {
        self.0.send(&Message::Expiration { session, time }, &[]);
    }

    fn closed(&self, session: String) {
        self.0.send(&Message::Closed { session }, &[]);
    }

    fn deferred(&self, call: Deferred) {
        lock(&self.0.state).deferred.push(call);
    }
}

fn decrypt(instance: &Instance, events: &Events, samples: &[Sample], payload: &[u8]) -> Result<Vec<u8>, Failure> {
    let malformed = |message: String| Failure { kind: FailureKind::Malformed, message, system_code: None, sample: None, kid: None };
    let total: u64 = samples.iter().map(|s| u64::from(s.size)).sum();
    if total != payload.len() as u64 {
        return Err(malformed(format!("{} bytes for samples of {total}", payload.len())));
    }
    let mut out = Vec::with_capacity(payload.len());
    let mut offset = 0;
    for (index, sample) in samples.iter().enumerate() {
        let data = &payload[offset..offset + sample.size as usize];
        offset += sample.size as usize;
        let Some((subsamples, protected)) = for_cdm(&sample.subsamples, data.len()) else {
            return Err(Failure { sample: Some(index), ..malformed("subsamples larger than their sample".into()) });
        };
        if protected == 0 {
            out.extend_from_slice(data);
            continue;
        }
        events.run_due(instance);
        let (crypt, skip) = sample.pattern.unwrap_or((0, 0));
        let input = Encrypted {
            data: &data[..protected],
            scheme: match sample.scheme {
                Scheme::Cenc => scheme::CENC,
                Scheme::Cbcs => scheme::CBCS,
            },
            kid: &sample.kid,
            iv: &sample.iv,
            subsamples: &subsamples,
            pattern: Pattern { crypt_byte_block: crypt.into(), skip_byte_block: skip.into() },
        };
        match instance.decrypt(&input) {
            Ok(clear) if clear.len() == protected => {
                out.extend_from_slice(&clear);
                out.extend_from_slice(&data[protected..]);
            }
            Ok(clear) => return Err(Failure { sample: Some(index), ..malformed(format!("the CDM gave back {} bytes for {protected}", clear.len())) }),
            Err(code) => {
                let kind = if code == status::NO_KEY { FailureKind::NoKey } else { FailureKind::DecryptError };
                return Err(Failure { kind, message: format!("Decrypt answered status {code}"), system_code: Some(code), sample: Some(index), kid: Some(sample.kid.clone()) });
            }
        }
    }
    Ok(out)
}

/// The subsamples as Widevine takes them, and how many leading bytes they cover. It answers `kNoKey` to an entry
/// with no protected bytes: those join the next entry's clear bytes, and a clear run at the end stays out.
fn for_cdm(subsamples: &[(u32, u32)], size: usize) -> Option<(Vec<SubsampleEntry>, usize)> {
    if subsamples.is_empty() {
        return Some((Vec::new(), size));
    }
    let (mut entries, mut clear, mut total, mut covered) = (Vec::new(), 0u64, 0u64, 0u64);
    for &(clear_bytes, cipher_bytes) in subsamples {
        total += u64::from(clear_bytes) + u64::from(cipher_bytes);
        clear += u64::from(clear_bytes);
        if cipher_bytes > 0 {
            entries.push(SubsampleEntry { clear_bytes: u32::try_from(clear).ok()?, cipher_bytes });
            clear = 0;
            covered = total;
        }
    }
    (total <= size as u64).then_some((entries, covered as usize))
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(pairs: &[(u32, u32)]) -> Vec<SubsampleEntry> {
        pairs.iter().map(|&(clear_bytes, cipher_bytes)| SubsampleEntry { clear_bytes, cipher_bytes }).collect()
    }

    #[test]
    fn subsamples_without_protected_bytes_join_the_next() {
        assert_eq!(for_cdm(&[], 40), Some((Vec::new(), 40)), "a whole-sample encryption");
        assert_eq!(for_cdm(&[(5, 16), (7, 0), (3, 16), (9, 0)], 56), Some((entries(&[(5, 16), (10, 16)]), 47)));
        assert_eq!(for_cdm(&[(10, 0), (6, 0)], 16), Some((Vec::new(), 0)), "nothing protected");
        assert_eq!(for_cdm(&[(10, 32)], 40), None, "larger than the sample");
    }
}
