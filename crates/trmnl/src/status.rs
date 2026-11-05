/// The `status` of a setup answer. The HTTP status is 200 either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStatus {
    /// The answer carries the device's key and friendly ID, which it stores.
    Registered,
    /// The firmware shows the image at `image_url`, or else `message`, then sleeps
    /// for its default interval and asks again.
    NotRegistered,
}

impl SetupStatus {
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Registered => 200,
            Self::NotRegistered => 404,
        }
    }
}

/// The `status` of a check-in answer. The HTTP status is 200 either way.
///
/// Firmware 1.8.18 acts on these three values (`display_session.cpp`); any other
/// value leaves the screen as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayStatus {
    /// Download and show `image_url`.
    Show,
    /// The device shows its friendly ID and checks in again soon.
    Unclaimed,
    /// The device erases its saved Wi-Fi networks and every stored setting, the
    /// server URL included, and restarts into its setup portal
    /// (`resetDeviceCredentials` in `bl.cpp`), where a person must provision it
    /// again. `reset_firmware: true` does the same.
    Reset,
}

impl DisplayStatus {
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Show => 0,
            Self::Unclaimed => 202,
            Self::Reset => 500,
        }
    }
}
