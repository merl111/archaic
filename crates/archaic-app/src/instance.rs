//! Per-user authenticated loopback delivery. No tokens, passwords or Matrix requests cross this IPC.
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use subtle::ConstantTimeEq;
#[derive(Clone, Serialize, Deserialize)]
pub struct Open {
    pub link: Option<String>,
    pub profile: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Endpoint {
    port: u16,
    token: [u8; 32],
}
#[derive(Serialize, Deserialize)]
struct Request {
    token: [u8; 32],
    open: Open,
}
pub struct Instance {
    pub incoming: mpsc::Receiver<Open>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    _lock: File,
}
pub enum Launch {
    Owner(Instance),
    Forwarded,
}
impl Drop for Instance {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid local activation")
}
fn validate(open: &Open) -> io::Result<()> {
    if let Some(link) = &open.link {
        archaic_matrix::links::parse(link).map_err(|_| invalid())?;
    }
    if let Some(profile) = &open.profile {
        archaic_matrix::storage::validate_profile_name(profile).map_err(|_| invalid())?;
    }
    Ok(())
}
pub fn launch(directory: &Path, open: Open) -> io::Result<Launch> {
    validate(&open)?;
    std::fs::create_dir_all(directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
    }
    let path = directory.join("instance.lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut lock = options.open(&path)?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            for _ in 0..10 {
                lock.rewind()?;
                let mut bytes = Vec::new();
                (&lock).take(4097).read_to_end(&mut bytes)?;
                if let Ok(endpoint) = serde_json::from_slice::<Endpoint>(&bytes) {
                    return forward(endpoint, &open).map(|()| Launch::Forwarded);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            return Err(io::Error::other("running Archaic instance is not ready"));
        }
        Err(std::fs::TryLockError::Error(error)) => return Err(error),
    }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let mut token = [0; 32];
    getrandom::fill(&mut token).map_err(io::Error::other)?;
    let endpoint = Endpoint {
        port: listener.local_addr()?.port(),
        token,
    };
    lock.set_len(0)?;
    lock.rewind()?;
    lock.write_all(&serde_json::to_vec(&endpoint)?)?;
    lock.sync_all()?;
    let (tx, incoming) = mpsc::sync_channel(8);
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let thread = std::thread::spawn(move || {
        while !stopping.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                    let result = receive(&mut stream, &token).and_then(|open| {
                        tx.try_send(open)
                            .map_err(|_| io::Error::other("activation queue busy"))
                    });
                    let _ = stream.write_all(if result.is_ok() { b"OK" } else { b"NO" });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Err(_) => break,
            }
        }
    });
    Ok(Launch::Owner(Instance {
        incoming,
        stop,
        thread: Some(thread),
        _lock: lock,
    }))
}
fn receive(stream: &mut TcpStream, token: &[u8; 32]) -> io::Result<Open> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > 16_384 {
        return Err(invalid());
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    let request: Request = serde_json::from_slice(&bytes)?;
    if !bool::from(request.token.ct_eq(token)) {
        return Err(invalid());
    }
    validate(&request.open)?;
    Ok(request.open)
}
fn forward(endpoint: Endpoint, open: &Open) -> io::Result<()> {
    let address = SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, endpoint.port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let bytes = serde_json::to_vec(&Request {
        token: endpoint.token,
        open: open.clone(),
    })?;
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)?;
    let mut reply = [0; 2];
    stream.read_exact(&mut reply)?;
    if reply != *b"OK" {
        return Err(io::Error::other("running instance rejected activation"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forwards_to_running_instance_and_rejects_wrong_secret() {
        let dir = tempfile::tempdir().unwrap();
        let open = Open {
            link: Some("matrix:u/alice:local".into()),
            profile: Some("work".into()),
        };
        let Launch::Owner(owner) = launch(dir.path(), open.clone()).unwrap() else {
            panic!("owner")
        };
        assert!(matches!(
            launch(dir.path(), open.clone()).unwrap(),
            Launch::Forwarded
        ));
        let received = owner.incoming.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(received.link, open.link);
        assert_eq!(received.profile, open.profile);
        let mut endpoint: Endpoint =
            serde_json::from_slice(&std::fs::read(dir.path().join("instance.lock")).unwrap())
                .unwrap();
        endpoint.token[0] ^= 1;
        assert!(forward(endpoint, &open).is_err());
        assert!(owner.incoming.try_recv().is_err());
        drop(owner);
        assert!(matches!(
            launch(dir.path(), open).unwrap(),
            Launch::Owner(_)
        ));
    }
}
