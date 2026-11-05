/// The `status` of a setup answer. The HTTP status is 200 either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStatus {
    /// The answer carries the device's key and friendly ID, which it stores.
    Registered,
    /// The firmware shows its friendly ID and a message, then sleeps and asks again.
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
/// The firmware acts on these three values (`display_session.cpp`); any other
/// value leaves the screen as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayStatus {
    /// Download and show `image_url`.
    Show,
    /// No account has claimed the device: it shows its friendly ID and checks in
    /// again soon.
    Unclaimed,
    /// The key is not valid: the device forgets it and runs setup again.
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
