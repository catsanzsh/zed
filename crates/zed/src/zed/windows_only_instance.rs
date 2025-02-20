use std::any;

use release_channel::ReleaseChannel;
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{
            GetLastError, ERROR_ALREADY_EXISTS, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, HANDLE,
        },
        Storage::FileSystem::{ReadFile, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_INBOUND},
        System::{
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
                PIPE_TYPE_MESSAGE, PIPE_WAIT,
            },
            Threading::{CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject, INFINITE},
            IO::{GetOverlappedResult, OVERLAPPED},
        },
    },
};

enum PipeState {
    Connecting,
    Reading,
}

struct PipeInfo {
    handle: HANDLE,
    overlapped: OVERLAPPED,
    pending_io: bool,
    state: PipeState,
    bytes_transferred: u32,
}

#[inline]
fn retrieve_app_identifier() -> &'static str {
    match *release_channel::RELEASE_CHANNEL {
        ReleaseChannel::Dev => "Zed-Editor-Dev",
        ReleaseChannel::Nightly => "Zed-Editor-Nightly",
        ReleaseChannel::Preview => "Zed-Editor-Preview",
        ReleaseChannel::Stable => "Zed-Editor-Stable",
    }
}

#[inline]
fn generate_identifier(name: &str) -> HSTRING {
    HSTRING::from(format!("{}-{}", retrieve_app_identifier(), name))
}

pub fn check_single_instance() -> bool {
    unsafe {
        CreateMutexW(None, false, &generate_identifier("Instance-Mutex"))
            .expect("Unable to create instance sync mutex")
    };
    let last_err = unsafe { GetLastError() };
    let ret = last_err != ERROR_ALREADY_EXISTS;

    if ret {
        // We are the first instance
        std::thread::spawn(move || {
            test_fn();
        });
    }

    ret
}

fn test_fn() {
    let pipe_event = unsafe { CreateEventW(None, true, true, PCWSTR::null()).unwrap() };
    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = pipe_event;
    let handle = unsafe {
        CreateNamedPipeW(
            &generate_identifier("Named-Pipe"),
            PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            1,
            128,
            128,
            5000,
            None,
        )
    };
    assert!(!handle.is_invalid());
    let pending_io = connect_to_new_client(handle, &mut overlapped).unwrap();
    let state = if pending_io {
        PipeState::Connecting
    } else {
        PipeState::Reading
    };
    let mut pipe = PipeInfo {
        handle,
        overlapped,
        pending_io,
        state,
        bytes_transferred: 0,
    };

    loop {
        let _ = unsafe { WaitForSingleObject(pipe_event, INFINITE) };
        if pipe.pending_io {
            let mut bytes_transferred = 0;
            let success = unsafe {
                GetOverlappedResult(handle, &pipe.overlapped, &mut bytes_transferred, false)
            };
            match pipe.state {
                PipeState::Connecting => {
                    if success.is_err() {
                        println!("Error 1: {:?}", unsafe { GetLastError() });
                        return;
                    }
                    pipe.state = PipeState::Reading;
                }
                PipeState::Reading => {
                    if success.is_err() || bytes_transferred == 0 {
                        println!("Client disconnected.");
                        disconnect_and_reconnect(&mut pipe);
                        continue;
                    }
                    pipe.bytes_transferred = bytes_transferred;
                }
            }

            match pipe.state {
                PipeState::Connecting => {
                    println!("==> Unreachable code")
                }
                PipeState::Reading => {
                    let mut buffer = [0u8; 128];
                    let result = unsafe {
                        ReadFile(
                            pipe.handle,
                            Some(&mut buffer),
                            Some(&mut pipe.bytes_transferred),
                            Some(&mut pipe.overlapped),
                        )
                    };
                    if result.is_ok() && pipe.bytes_transferred != 0 {
                        pipe.pending_io = false;
                        pipe.state = PipeState::Reading;
                        continue;
                    }
                    let reason = unsafe { GetLastError() };
                    if result.is_err() && reason == ERROR_IO_PENDING {
                        pipe.pending_io = true;
                        continue;
                    }
                    disconnect_and_reconnect(&mut pipe);
                }
            }
        }
    }
}

fn connect_to_new_client(pipe: HANDLE, lpo: &mut OVERLAPPED) -> anyhow::Result<bool> {
    unsafe { ConnectNamedPipe(pipe, Some(lpo))? };
    let reason = unsafe { GetLastError() };

    let mut pending_io = false;
    match reason {
        // The overlapped connection in progress.
        ERROR_IO_PENDING => {
            pending_io = true;
        }
        // Client is already connected, so signal an event.
        ERROR_PIPE_CONNECTED => {
            unsafe { SetEvent(lpo.hEvent) };
        }
        _ => {
            return Err(anyhow::anyhow!("Failed to connect to client: {:?}", reason));
        }
    }
    Ok(pending_io)
}

const CONNECTING_STATE: u32 = 0;
const READING_STATE: u32 = 1;

fn disconnect_and_reconnect(pipe: &mut PipeInfo) {
    if unsafe { DisconnectNamedPipe(pipe.handle).is_err() } {
        println!("DisconnectNamedPipe failed with error: {:?}", unsafe {
            GetLastError()
        });
    }
    pipe.pending_io = connect_to_new_client(pipe.handle, &mut pipe.overlapped).unwrap();
    pipe.state = if pipe.pending_io {
        PipeState::Connecting
    } else {
        PipeState::Reading
    };
}
