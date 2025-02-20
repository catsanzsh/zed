use release_channel::ReleaseChannel;
use windows::{
    core::HSTRING,
    Win32::{
        Foundation::{GetLastError, ERROR_ALREADY_EXISTS},
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
            with_pipe(|urls| println!("Received URLs: {}", urls));
        });
    }

    ret
}

fn with_pipe(f: impl Fn(String)) {
    let handle = unsafe {
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
    assert!(!handle.is_invalid());

    loop {
        unsafe { ConnectNamedPipe(handle, None).unwrap() };
        let mut buffer = [0u8; 128];
        unsafe {
            ReadFile(handle, Some(&mut buffer), None, None).unwrap();
        }
        let message = String::from_utf8_lossy(&buffer).to_string();
        unsafe { DisconnectNamedPipe(handle).unwrap() };
        f(message);
    }
}
