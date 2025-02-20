use anyhow::Context;
use release_channel::ReleaseChannel;
use util::ResultExt;
use windows::{
    core::HSTRING,
    Win32::{
        Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HANDLE},
        Storage::FileSystem::{ReadFile, PIPE_ACCESS_INBOUND},
        System::{
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
                PIPE_TYPE_MESSAGE, PIPE_WAIT,
            },
            Threading::CreateMutexW,
        },
    },
};

use crate::OpenListener;

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

#[inline]
fn generate_identifier_with_prefix(prefix: &str, name: &str) -> HSTRING {
    HSTRING::from(format!("{}{}-{}", prefix, retrieve_app_identifier(), name))
}

pub fn check_single_instance(opener: OpenListener) -> bool {
    unsafe {
        CreateMutexW(None, false, &generate_identifier("Instance-Mutex"))
            .expect("Unable to create instance sync mutex")
    };
    let last_err = unsafe { GetLastError() };
    let ret = last_err != ERROR_ALREADY_EXISTS;

    if ret {
        // We are the first instance
        std::thread::spawn(move || {
            with_pipe(|url| opener.open_urls(vec![url]));
        });
    }

    ret
}

fn with_pipe(f: impl Fn(String)) {
    let pipe = unsafe {
        CreateNamedPipeW(
            &generate_identifier_with_prefix("\\\\.\\pipe\\", "Named-Pipe"),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            1,
            128,
            128,
            0,
            None,
        )
    };
    if pipe.is_invalid() {
        log::error!("Failed to create named pipe: {:?}", unsafe {
            GetLastError()
        });
        return;
    }

    loop {
        if let Some(message) = retrieve_message_from_pipe(pipe)
            .context("Failed to read from named pipe")
            .log_err()
        {
            f(message);
        }
    }
}

fn retrieve_message_from_pipe(pipe: HANDLE) -> anyhow::Result<String> {
    unsafe { ConnectNamedPipe(pipe, None)? };
    let message = retrieve_message_from_pipe_inner(pipe);
    unsafe { DisconnectNamedPipe(pipe).log_err() };
    message
}

fn retrieve_message_from_pipe_inner(pipe: HANDLE) -> anyhow::Result<String> {
    let mut buffer = [0u8; 128];
    unsafe {
        ReadFile(pipe, Some(&mut buffer), None, None)?;
    }
    let message = std::ffi::CStr::from_bytes_until_nul(&buffer)?;
    Ok(message.to_string_lossy().to_string())
}
