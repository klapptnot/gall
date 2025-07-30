use crate::{Arc, Mutex};

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::mem::{align_of, size_of};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

pub(crate) type MessageQueue = Arc<Mutex<VecDeque<Vec<u8>>>>;
static SOCKET_PATH: OnceLock<PathBuf> = OnceLock::new();

#[repr(C)]
#[derive(Debug)]
pub enum AppMessage {
    TogglePicker(crate::pickers::PickerKind),
    AppPing,
    AppClose,
    AppReload,
}
impl From<Vec<u8>> for AppMessage {
    fn from(bytes: Vec<u8>) -> Self {
        assert_eq!(bytes.len(), size_of::<AppMessage>(), "Wrong size");
        assert_eq!(
            bytes.as_ptr() as usize % align_of::<AppMessage>(),
            0,
            "Misaligned buffer"
        );

        unsafe { (bytes.as_ptr() as *const AppMessage).read_unaligned() }
    }
}

impl From<&[u8]> for AppMessage {
    fn from(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), size_of::<AppMessage>());
        assert_eq!(
            bytes.as_ptr() as usize % align_of::<AppMessage>(),
            0,
            "Misaligned buffer"
        );

        unsafe { (bytes.as_ptr() as *const AppMessage).read() }
    }
}

impl Into<Vec<u8>> for AppMessage {
    fn into(self) -> Vec<u8> {
        let size = size_of::<Self>();
        let ptr = &self as *const Self as *const u8;
        unsafe { std::slice::from_raw_parts(ptr, size).to_vec() }
    }
}

pub fn get_socket_path() -> &'static PathBuf {
    SOCKET_PATH.get_or_init(|| {
        let dir = std::env::var("XDG_RUNTIME_DIR").expect("Could not get XDG_RUNTIME_DIR");
        PathBuf::from(dir).join("gall.socket")
    })
}

pub fn start_socket_listener(message_queue: MessageQueue) {
    let listener = match UnixListener::bind(get_socket_path()) {
        Ok(listener) => listener,
        Err(_) => {
            if let Ok(mut queue) = message_queue.lock() {
                queue.push_back(AppMessage::AppClose.into());
            }
            return;
        }
    };

    if let Err(_) = listener.set_nonblocking(true) {
        if let Ok(mut queue) = message_queue.lock() {
            queue.push_back(AppMessage::AppClose.into());
        }
        return;
    }

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let queue = Arc::clone(&message_queue);
                thread::spawn(move || handle_client(stream, queue));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => (),
        }
    }
}

pub fn handle_client(mut stream: UnixStream, message_queue: MessageQueue) {
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(n) => {
                if n == std::mem::size_of::<AppMessage>() {
                    match AppMessage::from(buffer[..n].to_vec()) {
                        AppMessage::AppPing => {
                            let response: Vec<u8> = AppMessage::AppPing.into();
                            let _ = stream.write_all(&response);
                        }
                        msg => {
                            if let Ok(mut queue) = message_queue.lock() {
                                queue.push_back(msg.into());
                            }
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
}

pub fn send_message(message: AppMessage) -> Result<(), Box<dyn std::error::Error>> {
    if !process_is_running() {
        return Err("Process is dead!".into());
    }
    let mut stream = UnixStream::connect(get_socket_path())?;
    stream.write_all(Into::<Vec<u8>>::into(message).as_slice())?;
    stream.flush()?;
    Ok(())
}

pub fn process_is_running() -> bool {
    if !Path::new(get_socket_path()).exists() {
        return false;
    }

    match UnixStream::connect(get_socket_path()) {
        Ok(mut stream) => {
            let ping_msg = AppMessage::AppPing;
            let ping_bytes: Vec<u8> = ping_msg.into();
            if let Err(_) = stream.write_all(&ping_bytes) {
                return false;
            }

            let mut buffer = [0u8; std::mem::size_of::<AppMessage>()];
            match stream.read_exact(&mut buffer) {
                Ok(_) => {
                    let reply = AppMessage::from(buffer.to_vec());
                    matches!(reply, AppMessage::AppPing)
                }
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}
