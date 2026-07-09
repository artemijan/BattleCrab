//! Ports of `loginserver/enums/*` (values used so far; extended as the
//! packet set grows).

/// `LoginFailReason.java`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LoginFailReason {
    ReasonNoMessage = 0x00,
    ReasonSystemErrorLoginLater = 0x01,
    ReasonUserOrPassWrong = 0x02,
    ReasonAccessFailedTryAgainLater = 0x04,
    ReasonAccountInfoIncorrectContactSupport = 0x05,
    ReasonNotAuthed = 0x06,
    ReasonAccountInUse = 0x07,
    ReasonServerOverloaded = 0x0F,
    ReasonServerMaintenance = 0x10,
    ReasonSystemError = 0x14,
    ReasonAccessFailed = 0x15,
    ReasonRestrictedIp = 0x16,
}

/// `loginserver/network/ConnectionState.java`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    AuthedGg,
    AuthedLogin,
}
