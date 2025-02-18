use release_channel::ReleaseChannel;
use windows::{
    core::HSTRING,
    Win32::{
        Foundation::{GetLastError, ERROR_ALREADY_EXISTS},
        System::Threading::CreateMutexW,
    },
};

fn retrieve_app_instance_event_identifier() -> &'static str {
    match *release_channel::RELEASE_CHANNEL {
        ReleaseChannel::Dev => "Zed-Editor-Dev-Instance-Mutex",
        ReleaseChannel::Nightly => "Zed-Editor-Nightly-Instance-Mutex",
        ReleaseChannel::Preview => "Zed-Editor-Preview-Instance-Mutex",
        ReleaseChannel::Stable => "Zed-Editor-Stable-Instance-Mutex",
    }
}

pub fn check_single_instance() -> bool {
    unsafe {
        CreateMutexW(
            None,
            false,
            &HSTRING::from(retrieve_app_instance_event_identifier()),
        )
        .expect("Unable to create instance sync mutex")
    };
    let last_err = unsafe { GetLastError() };
    last_err != ERROR_ALREADY_EXISTS
}
