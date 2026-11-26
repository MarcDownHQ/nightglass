//! The app's side of `protocol`: starts a helper, sends it requests, and gets licences through the caller.

use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Notify, oneshot};

use crate::protocol::{self, Envelope, Failure, FailureKind, Key, Message, MessageKind, Reply, Request, Sample};
use crate::cenc::Cdm;
use crate::pssh::Pssh;
use crate::{BoxFuture, DrmError, Exchange, Result, hex};

/// How long the helper may take to load the CDM and say it is ready.
pub const START: Duration = Duration::from_secs(30);
/// How long a CDM call or a decryption may take before the helper is taken for hung and stopped.
pub const CALL: Duration = Duration::from_secs(60);
/// How long a session may wait for its next message, a licence answer included.
pub const MESSAGE: Duration = Duration::from_secs(30);
/// How long, once a licence is in, the CDM may take to say which keys it brought.
pub const KEYS: Duration = Duration::from_secs(5);

/// A program that speaks `protocol` on its stdin and stdout.
#[derive(Clone, Debug, PartialEq)]
pub struct Backend {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl Backend {
    /// Google's CDM built for this OS, hosted by `nightglass-cdm`.
    pub fn native(helper: impl Into<PathBuf>, cdm: impl Into<PathBuf>) -> Self {
        Backend { program: helper.into(), args: vec![cdm.into().into_os_string()] }
    }
}

/// A running helper. Cheap to clone; the process stops with the last clone.
#[derive(Clone)]
pub struct Helper(Arc<Inner>);

type Writer = Box<dyn AsyncWrite + Send + Unpin>;

struct Inner {
    stdin: tokio::sync::Mutex<Writer>,
    child: Mutex<Option<Child>>,
    shared: Arc<Shared>,
    cdm_version: String,
}

/// Why a helper did not start: it said why, or it ended before saying anything.
enum Started {
    Not(DrmError),
    Ended,
}

/// A reply and its payload.
type Answer = (Reply, Vec<u8>);

#[derive(Default)]
struct Shared {
    next: AtomicU64,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Answer>>>>,
    sessions: Mutex<HashMap<String, Session>>,
    changed: Notify,
    stopped: Mutex<Option<String>>,
    last_words: Mutex<VecDeque<String>>,
}

#[derive(Default)]
struct Session {
    messages: VecDeque<(MessageKind, Vec<u8>)>,
    keys: Vec<Key>,
}

impl Helper {
    pub async fn start(backend: &Backend) -> Result<Helper> {
        let mut child = command(backend)
            .spawn()
            .map_err(|e| DrmError::Helper(format!("{}: {e}", backend.program.display())))?;
        let shared = Arc::new(Shared::default());
        let (Some(stdin), Some(stdout), Some(stderr)) = (child.stdin.take(), child.stdout.take(), child.stderr.take()) else {
            return Err(DrmError::Helper("no pipes".into()));
        };
        tokio::spawn(keep_last_words(Arc::downgrade(&shared), stderr));
        match Helper::connect(Box::new(stdin), stdout, shared.clone()).await {
            Ok(helper) => {
                *lock(&helper.0.child) = Some(child);
                Ok(helper)
            }
            Err(Started::Not(e)) => Err(e),
            Err(Started::Ended) => {
                let status = child.wait().await.ok();
                tokio::time::sleep(Duration::from_millis(50)).await;
                Err(DrmError::Helper(shared.last_words().or(status.map(|s| s.to_string())).unwrap_or_else(|| "did not start".into())))
            }
        }
    }

    /// A helper speaking on `stdin` and `stdout`, once it has said it is ready.
    async fn connect(stdin: Writer, stdout: impl AsyncRead + Send + Unpin + 'static, shared: Arc<Shared>) -> std::result::Result<Helper, Started> {
        let mut stdout = BufReader::new(stdout);
        let ready = tokio::time::timeout(START, read_frame(&mut stdout)).await;
        let cdm_version = match ready {
            Ok(Ok(Some((Message::Ready { protocol: protocol::PROTOCOL, cdm_version }, _)))) => cdm_version,
            Ok(Ok(Some((Message::Ready { protocol, .. }, _)))) => return Err(Started::Not(DrmError::Helper(format!("speaks protocol {protocol}, not {}", protocol::PROTOCOL)))),
            Ok(Ok(_)) | Ok(Err(_)) => return Err(Started::Ended),
            Err(_) => return Err(Started::Not(DrmError::Helper(format!("not ready after {} s", START.as_secs())))),
        };
        tokio::spawn(read_messages(shared.clone(), stdout));
        Ok(Helper(Arc::new(Inner { stdin: tokio::sync::Mutex::new(stdin), child: Mutex::new(None), shared, cdm_version })))
    }

    pub fn cdm_version(&self) -> &str {
        &self.0.cdm_version
    }

    pub async fn initialize(&self) -> Result<()> {
        self.call(Request::Initialize { distinctive_identifier: false, persistent_state: false }, Vec::new()).await.map(drop)
    }

    pub async fn set_server_certificate(&self, certificate: Vec<u8>) -> Result<()> {
        self.call(Request::SetServerCertificate, certificate).await.map(drop)
    }

    /// A session for `pssh` (one or more whole boxes), whose first message waits in `next_message`.
    pub async fn create_session(&self, pssh: Vec<u8>) -> Result<String> {
        match self.call(Request::CreateSession, pssh).await? {
            (Reply::Session { session }, _) => Ok(session),
            (other, _) => Err(DrmError::Helper(format!("answered {other:?} to a new session"))),
        }
    }

    pub async fn next_message(&self, session: &str) -> Result<(MessageKind, Vec<u8>)> {
        self.0.shared.next_message(session, MESSAGE).await
    }

    pub async fn update_session(&self, session: &str, response: Vec<u8>) -> Result<()> {
        self.call(Request::UpdateSession { session: session.into() }, response).await.map(drop)
    }

    pub async fn close_session(&self, session: &str) -> Result<()> {
        self.call(Request::CloseSession { session: session.into() }, Vec::new()).await.map(drop)
    }

    /// Every key of every session, as the CDM last reported them.
    pub fn keys(&self) -> Vec<Key> {
        lock(&self.0.shared.sessions).values().flat_map(|s| s.keys.iter().cloned()).collect()
    }

    /// `data` holds `samples` back to back; they come back clear, at the same sizes.
    pub async fn decrypt(&self, samples: Vec<Sample>, data: Vec<u8>) -> Result<Vec<u8>> {
        let size = data.len();
        let video: Vec<Vec<u8>> = samples.iter().filter(|s| s.subsamples.len() > 1).map(|s| s.kid.clone()).collect();
        match self.call(Request::Decrypt { samples }, data).await {
            Ok((Reply::Decrypted, clear)) if clear.len() == size => Ok(clear),
            Ok((Reply::Decrypted, clear)) => Err(DrmError::Helper(format!("gave back {} bytes for {size}", clear.len()))),
            Ok((other, _)) => Err(DrmError::Helper(format!("answered {other:?} to a decryption"))),
            Err(DrmError::NoKey { kid }) if video.iter().any(|v| hex(v) == kid && self.0.shared.key_usable(v)) => Err(DrmError::DecoderOnly { kid }),
            Err(e) => Err(e),
        }
    }

    /// A licence for `pssh` through `exchange`: the keys it brought, once one at least is usable. Renewals the CDM
    /// asks for later go through `exchange` as well, for as long as the helper runs.
    pub async fn license(&self, pssh: Vec<u8>, exchange: Exchange) -> Result<Vec<Key>> {
        let session = self.create_session(pssh).await?;
        loop {
            let (kind, challenge) = self.next_message(&session).await?;
            let response = exchange(MessageKind::of(kind, &challenge), challenge).await?;
            self.update_session(&session, response).await?;
            let (keys, more) = self.0.shared.settled(&session, KEYS).await;
            if keys.iter().any(Key::usable) {
                tokio::spawn(renew(Arc::downgrade(&self.0), self.0.shared.clone(), session, exchange));
                return Ok(keys);
            }
            if !more {
                let statuses: Vec<String> = keys.iter().map(|k| format!("{}: {}", hex(&k.kid), k.status_name())).collect();
                return Err(DrmError::NoUsableKey(if statuses.is_empty() { "none".into() } else { statuses.join(", ") }));
            }
        }
    }

    /// Whether the CDM decrypts with `kid` itself rather than only through its decoder: a made-up sample, which
    /// CTR decrypts to noise without complaint when the licence allows it. Google's desktop CDM answers `kNoKey`,
    /// for a key it holds as usable, to any sample of several subsamples, the shape of a video frame: video then
    /// only goes through its decoder.
    pub async fn probe(&self, kid: &[u8], video: bool) -> Result<()> {
        let subsamples = if video { vec![(16, 16), (16, 16)] } else { vec![(16, 16)] };
        let size = subsamples.iter().map(|(c, p)| c + p).sum::<u32>();
        let sample = Sample { size, scheme: protocol::Scheme::Cenc, kid: kid.to_vec(), iv: vec![0; 16], pattern: None, subsamples };
        self.decrypt(vec![sample], vec![0; size as usize]).await.map(drop)
    }

    async fn call(&self, request: Request, payload: Vec<u8>) -> Result<Answer> {
        let shared = &self.0.shared;
        let id = shared.next.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        lock(&shared.pending).insert(id, sender);
        if let Some(reason) = lock(&shared.stopped).clone() {
            lock(&shared.pending).remove(&id);
            return Err(DrmError::Helper(reason));
        }
        let header = serde_json::to_vec(&Envelope { id, request }).map_err(|e| DrmError::Helper(e.to_string()))?;
        let lengths = protocol::frame_lengths(header.len(), payload.len()).map_err(|e| DrmError::Malformed(e.to_string()))?;
        let written = async {
            let mut stdin = self.0.stdin.lock().await;
            stdin.write_all(&lengths).await?;
            stdin.write_all(&header).await?;
            stdin.write_all(&payload).await?;
            stdin.flush().await
        };
        if let Err(e) = written.await {
            lock(&shared.pending).remove(&id);
            return Err(DrmError::Helper(shared.last_words().unwrap_or_else(|| e.to_string())));
        }
        match tokio::time::timeout(CALL, receiver).await {
            Ok(Ok(answer)) => answer,
            Ok(Err(_)) => Err(DrmError::Helper(lock(&shared.stopped).clone().unwrap_or_default())),
            Err(_) => {
                lock(&shared.pending).remove(&id);
                if let Some(child) = lock(&self.0.child).as_mut() {
                    let _ = child.start_kill();
                }
                Err(DrmError::Helper(format!("no answer after {} s", CALL.as_secs())))
            }
        }
    }
}

/// `backend` with pipes for its stdin, stdout and stderr, and nothing else the app has open: the GPU, the
/// sockets and the files of the app's libraries stay out of the helper.
fn command(backend: &Backend) -> Command {
    let mut command = Command::new(&backend.program);
    command.args(&backend.args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(unix)]
    unsafe {
        command.pre_exec(inherit_nothing);
    }
    command
}

/// Runs between fork and exec, where only async-signal-safe calls are allowed. Marking the descriptors
/// close-on-exec rather than closing them keeps the one the spawn reports exec failures through.
#[cfg(unix)]
fn inherit_nothing() -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        const CLOSE_RANGE_CLOEXEC: libc::c_uint = 1 << 2;
        if unsafe { libc::syscall(libc::SYS_close_range, 3 as libc::c_uint, libc::c_uint::MAX, CLOSE_RANGE_CLOEXEC) } == 0 {
            return Ok(());
        }
    }
    let max = match unsafe { libc::sysconf(libc::_SC_OPEN_MAX) } {
        n if n > 0 => n.min(1 << 16) as libc::c_int,
        _ => 1 << 16,
    };
    for fd in 3..max {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        if flags >= 0 && flags & libc::FD_CLOEXEC == 0 {
            unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
        }
    }
    Ok(())
}

impl Cdm for Helper {
    fn decrypt(&self, samples: Vec<Sample>, data: Vec<u8>) -> BoxFuture<'_, Result<Vec<u8>>> {
        Box::pin(Helper::decrypt(self, samples, data))
    }

    fn license(&self, pssh: Pssh, exchange: Exchange) -> BoxFuture<'_, Result<Vec<Key>>> {
        Box::pin(Helper::license(self, pssh.bytes, exchange))
    }

    fn probe<'a>(&'a self, kid: &'a [u8], video: bool) -> BoxFuture<'a, Result<()>> {
        Box::pin(Helper::probe(self, kid, video))
    }

    fn keys(&self) -> Vec<Key> {
        Helper::keys(self)
    }
}

impl Shared {
    /// A session's keys and whether it has a message waiting, once it has either or `wait` has passed.
    async fn settled(&self, session: &str, wait: Duration) -> (Vec<Key>, bool) {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let (keys, more) = lock(&self.sessions).get(session).map(|s| (s.keys.clone(), !s.messages.is_empty())).unwrap_or_default();
            if !keys.is_empty() || more || lock(&self.stopped).is_some() || tokio::time::timeout_at(deadline, changed).await.is_err() {
                return (keys, more);
            }
        }
    }

    async fn next_message(&self, session: &str, wait: Duration) -> Result<(MessageKind, Vec<u8>)> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if let Some(message) = lock(&self.sessions).get_mut(session).and_then(|s| s.messages.pop_front()) {
                return Ok(message);
            }
            if let Some(reason) = lock(&self.stopped).clone() {
                return Err(DrmError::Helper(reason));
            }
            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                return Err(DrmError::Cdm(format!("session {session} asked nothing within {} s", wait.as_secs())));
            }
        }
    }

    fn last_words(&self) -> Option<String> {
        lock(&self.last_words).back().cloned()
    }

    fn key_usable(&self, kid: &[u8]) -> bool {
        lock(&self.sessions).values().flat_map(|s| &s.keys).any(|k| k.kid == kid && k.usable())
    }

    fn failure(&self, failure: Failure) -> DrmError {
        let kid = failure.kid.as_deref().map(hex).unwrap_or_default();
        match failure.kind {
            FailureKind::Rejected => match failure.system_code {
                Some(code) if code != 0 => DrmError::Rejected(format!("{} (code {code})", failure.message)),
                _ => DrmError::Rejected(failure.message),
            },
            FailureKind::Initialization => DrmError::Cdm(failure.message),
            FailureKind::NoKey => DrmError::NoKey { kid },
            FailureKind::DecryptError if failure.kid.as_deref().is_some_and(|k| self.key_usable(k)) => DrmError::SecureDecodeRequired { kid },
            FailureKind::DecryptError => DrmError::Cdm(format!("{} ({kid})", failure.message)),
            FailureKind::Malformed => DrmError::Malformed(failure.message),
        }
    }

    fn stop(&self, reason: String) {
        let reason = self.last_words().map_or(reason.clone(), |words| format!("{reason}: {words}"));
        *lock(&self.stopped) = Some(reason.clone());
        for (_, sender) in lock(&self.pending).drain() {
            let _ = sender.send(Err(DrmError::Helper(reason.clone())));
        }
        self.changed.notify_waiters();
    }
}

async fn read_frame(input: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Option<(Message, Vec<u8>)>> {
    let mut lengths = [0; 8];
    match input.read_exact(&mut lengths).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let (header, payload) = protocol::read_lengths(lengths)?;
    let mut header_bytes = vec![0; header];
    input.read_exact(&mut header_bytes).await?;
    let mut payload_bytes = vec![0; payload];
    input.read_exact(&mut payload_bytes).await?;
    let message = serde_json::from_slice(&header_bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Some((message, payload_bytes)))
}

async fn read_messages(shared: Arc<Shared>, mut stdout: impl AsyncRead + Unpin) {
    loop {
        let (message, payload) = match read_frame(&mut stdout).await {
            Ok(Some(frame)) => frame,
            Ok(None) => return shared.stop("the helper quit".into()),
            Err(e) => return shared.stop(e.to_string()),
        };
        match message {
            Message::Reply { id, reply } => {
                if let Some(sender) = lock(&shared.pending).remove(&id) {
                    let _ = sender.send(Ok((reply, payload)));
                }
            }
            Message::Failed { id, failure } => {
                let error = shared.failure(failure);
                if let Some(sender) = lock(&shared.pending).remove(&id) {
                    let _ = sender.send(Err(error));
                }
            }
            Message::SessionMessage { session, kind } => {
                lock(&shared.sessions).entry(session).or_default().messages.push_back((kind, payload));
                shared.changed.notify_waiters();
            }
            Message::Keys { session, keys } => {
                lock(&shared.sessions).entry(session).or_default().keys = keys;
                shared.changed.notify_waiters();
            }
            Message::Closed { session } => {
                lock(&shared.sessions).remove(&session);
                shared.changed.notify_waiters();
            }
            Message::Expiration { .. } | Message::Ready { .. } => {}
        }
    }
}

async fn keep_last_words(shared: Weak<Shared>, stderr: impl AsyncRead + Unpin) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        log::debug!("nightglass-cdm: {line}");
        let Some(shared) = shared.upgrade() else { return };
        let mut words = lock(&shared.last_words);
        if words.len() == 8 {
            words.pop_front();
        }
        words.push_back(line);
    }
}

/// Renewals of one session, for as long as the helper runs.
async fn renew(helper: Weak<Inner>, shared: Arc<Shared>, session: String, exchange: Exchange) {
    loop {
        let Ok((kind, challenge)) = shared.next_message(&session, Duration::from_secs(24 * 3600)).await else {
            if lock(&shared.stopped).is_some() || !lock(&shared.sessions).contains_key(&session) {
                return;
            }
            continue;
        };
        if kind == MessageKind::LicenseRelease {
            continue;
        }
        let Some(helper) = helper.upgrade().map(Helper) else { return };
        let answer = exchange(MessageKind::of(kind, &challenge), challenge).await;
        match answer {
            Ok(response) => {
                if let Err(e) = helper.update_session(&session, response).await {
                    log::warn!("widevine: renewal of {session}: {e}");
                }
            }
            Err(e) => log::warn!("widevine: renewal of {session}: {e}"),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Envelope;

    async fn send(to: &mut (impl AsyncWrite + Unpin), message: &Message, payload: &[u8]) {
        let mut frame = Vec::new();
        protocol::write_frame(&mut frame, message, payload).unwrap();
        for byte in frame {
            to.write_all(&[byte]).await.unwrap();
            to.flush().await.unwrap();
        }
        tokio::task::yield_now().await;
    }

    async fn request(from: &mut (impl AsyncRead + Unpin)) -> Envelope {
        let mut lengths = [0; 8];
        from.read_exact(&mut lengths).await.unwrap();
        let (header, payload) = protocol::read_lengths(lengths).unwrap();
        let mut bytes = vec![0; header + payload];
        from.read_exact(&mut bytes).await.unwrap();
        serde_json::from_slice(&bytes[..header]).unwrap()
    }

    /// A helper over in-memory pipes whose CDM reports keys only after resolving the update, and sends every
    /// frame a byte at a time.
    async fn late_keys(keys_first: bool) -> Result<Vec<Key>> {
        let (client_out, mut helper_in) = tokio::io::duplex(1 << 16);
        let (mut helper_out, client_in) = tokio::io::duplex(1 << 16);
        tokio::spawn(async move {
            send(&mut helper_out, &Message::Ready { protocol: protocol::PROTOCOL, cdm_version: "test".into() }, &[]).await;
            loop {
                let envelope = request(&mut helper_in).await;
                let id = envelope.id;
                match envelope.request {
                    Request::CreateSession => {
                        send(&mut helper_out, &Message::Reply { id, reply: Reply::Session { session: "s".into() } }, &[]).await;
                        send(&mut helper_out, &Message::SessionMessage { session: "s".into(), kind: MessageKind::LicenseRequest }, b"challenge").await;
                    }
                    Request::UpdateSession { .. } => {
                        let keys = Message::Keys { session: "s".into(), keys: vec![Key { kid: vec![7; 16], status: Key::USABLE, system_code: 0 }] };
                        if keys_first {
                            send(&mut helper_out, &keys, &[]).await;
                        }
                        send(&mut helper_out, &Message::Reply { id, reply: Reply::Done }, &[]).await;
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        if !keys_first {
                            send(&mut helper_out, &keys, &[]).await;
                        }
                    }
                    _ => send(&mut helper_out, &Message::Reply { id, reply: Reply::Done }, &[]).await,
                }
            }
        });
        let helper = Helper::connect(Box::new(client_out), client_in, Arc::new(Shared::default())).await.map_err(|_| DrmError::Helper("not ready".into()))?;
        let exchange: Exchange = Arc::new(|_, challenge| Box::pin(async move { Ok(challenge) }));
        helper.license(vec![1], exchange).await
    }

    #[tokio::test]
    async fn keys_count_whether_they_come_before_or_after_the_update_is_done() {
        for keys_first in [true, false] {
            let keys = late_keys(keys_first).await.unwrap();
            assert_eq!(keys, vec![Key { kid: vec![7; 16], status: Key::USABLE, system_code: 0 }], "keys first: {keys_first}");
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn the_helper_inherits_only_its_pipes() {
        let file = std::fs::File::open("/proc/self/exe").unwrap();
        let leaked = unsafe { libc::dup2(std::os::fd::AsRawFd::as_raw_fd(&file), 300) };
        assert_eq!(leaked, 300, "a descriptor open without close-on-exec, as a library might leave one");
        let script = "ls /proc/$$/fd | tr '\\n' ' '; test -e /proc/$$/fd/300 && echo leaked || echo clean";
        let probe = Backend { program: "sh".into(), args: vec!["-c".into(), script.into()] };
        let out = command(&probe).output().await.unwrap();
        unsafe { libc::close(300) };
        let out = String::from_utf8_lossy(&out.stdout);
        assert!(out.ends_with("clean\n"), "{out}");
    }
}
