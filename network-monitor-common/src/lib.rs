#![no_std]

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TcpConnectEvent {
    pub pid: u32,
    pub tid: u32,
    pub family: u16,
    pub sport: u16,
    pub dport: u16,
    pub saddr: [u8; 16],
    pub daddr: [u8; 16],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TcpAcceptEvent {
    pub pid: u32,
    pub tid: u32,
    pub family: u16,
    pub sport: u16,
    pub dport: u16,
    pub saddr: [u8; 16],
    pub daddr: [u8; 16],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TcpCloseEvent {
    pub pid: u32,
    pub tid: u32,
    pub family: u16,
    pub sport: u16,
    pub dport: u16,
    pub saddr: [u8; 16],
    pub daddr: [u8; 16],
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub duration_ns: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EbpfEvent {
    pub event_type: u32,
    pub data: EbpfEventData,
}

#[repr(C)]
pub union EbpfEventData {
    pub connect: TcpConnectEvent,
    pub accept: TcpAcceptEvent,
    pub close: TcpCloseEvent,
}

impl core::fmt::Debug for EbpfEventData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EbpfEventData")
            .field("connect", unsafe { &self.connect })
            .finish()
    }
}

impl Clone for EbpfEventData {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for EbpfEventData {}

pub const AF_INET: u16 = 2;
pub const AF_INET6: u16 = 10;

pub const EVENT_TYPE_CONNECT: u32 = 0;
pub const EVENT_TYPE_CLOSE: u32 = 1;
pub const EVENT_TYPE_ACCEPT: u32 = 2;
