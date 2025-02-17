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
        ReleaseChannel::Dev => "Local\\Zed-Editor-Dev-Instance-Mutex",
        ReleaseChannel::Nightly => "Local\\Zed-Editor-Nightly-Instance-Mutex",
        ReleaseChannel::Preview => "Local\\Zed-Editor-Preview-Instance-Mutex",
        ReleaseChannel::Stable => "Local\\Zed-Editor-Stable-Instance-Mutex",
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
