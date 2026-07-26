#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::bpf_probe_read_kernel,
    macros::{kprobe, kretprobe, map},
    maps::PerfEventArray,
    programs::{ProbeContext, RetProbeContext},
};
use network_monitor_common::{
    EbpfEvent, EbpfEventData, TcpAcceptEvent, TcpCloseEvent, TcpConnectEvent,
};

#[repr(C)]
struct SockCommon {
    skc_daddr: [u8; 4],
    skc_rcv_saddr: [u8; 4],
    _skc_hash: u32,
    skc_dport: u16,
    skc_num: u16,
    skc_family: u16,
}

#[repr(C)]
struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: [u8; 4],
    _sin_zero: [u8; 8],
}

#[repr(C)]
struct SockAddrIn6 {
    sin6_family: u16,
    sin6_port: u16,
    _sin6_flowinfo: u32,
    sin6_addr: [u8; 16],
    _sin6_scope_id: u32,
}

#[map]
pub static EVENTS: PerfEventArray<EbpfEvent> = PerfEventArray::new(0);

#[map]
pub static BIRTH_TIMES: aya_ebpf::maps::HashMap<u64, u64> =
    aya_ebpf::maps::HashMap::with_max_entries(10240, 0);

fn ipv4_to_v6_mapped(addr: [u8; 4]) -> [u8; 16] {
    let mut mapped = [0u8; 16];
    mapped[10] = 0xff;
    mapped[11] = 0xff;
    mapped[12] = addr[0];
    mapped[13] = addr[1];
    mapped[14] = addr[2];
    mapped[15] = addr[3];
    mapped
}

const AF_INET: u16 = 2;
const AF_INET6: u16 = 10;

#[kprobe]
pub fn kprobe_tcp_connect(ctx: ProbeContext) -> u32 {
    match unsafe { try_kprobe_tcp_connect(ctx) } {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

unsafe fn try_kprobe_tcp_connect(ctx: ProbeContext) -> Result<u32, u32> {
    let sk_ptr: u64 = ctx.arg(0).ok_or(1u32)?;
    let uaddr_ptr: u64 = ctx.arg(1).ok_or(1u32)?;
    let pid_tgid = aya_ebpf::helpers::bpf_get_current_pid_tgid();
    let now = aya_ebpf::helpers::bpf_ktime_get_ns();
    let pid = (pid_tgid >> 32) as u32;
    let tid = pid_tgid as u32;

    unsafe { BIRTH_TIMES.insert(&sk_ptr, &now, 0).map_err(|_| 1u32)? };

    let addr_in: SockAddrIn =
        bpf_probe_read_kernel(uaddr_ptr as *const SockAddrIn).map_err(|_| 1u32)?;
    let skc: SockCommon =
        bpf_probe_read_kernel(sk_ptr as *const SockCommon).map_err(|_| 1u32)?;

    let dport = u16::from_be(addr_in.sin_port);

    let event = EbpfEvent {
        event_type: 0,
        data: EbpfEventData {
            connect: TcpConnectEvent {
                pid,
                tid,
                family: AF_INET,
                sport: u16::from_be(skc.skc_num),
                dport,
                saddr: ipv4_to_v6_mapped(skc.skc_rcv_saddr),
                daddr: ipv4_to_v6_mapped(addr_in.sin_addr),
            },
        },
    };

    EVENTS.output(&ctx, &event, 0);
    Ok(0)
}

#[kprobe]
pub fn kprobe_tcp_connect6(ctx: ProbeContext) -> u32 {
    match unsafe { try_kprobe_tcp_connect6(ctx) } {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

unsafe fn try_kprobe_tcp_connect6(ctx: ProbeContext) -> Result<u32, u32> {
    let sk_ptr: u64 = ctx.arg(0).ok_or(1u32)?;
    let uaddr_ptr: u64 = ctx.arg(1).ok_or(1u32)?;
    let pid_tgid = aya_ebpf::helpers::bpf_get_current_pid_tgid();
    let now = aya_ebpf::helpers::bpf_ktime_get_ns();
    let pid = (pid_tgid >> 32) as u32;
    let tid = pid_tgid as u32;

    unsafe { BIRTH_TIMES.insert(&sk_ptr, &now, 0).map_err(|_| 1u32)? };

    let addr_in6: SockAddrIn6 =
        bpf_probe_read_kernel(uaddr_ptr as *const SockAddrIn6).map_err(|_| 1u32)?;
    let dport = u16::from_be(addr_in6.sin6_port);

    let event = EbpfEvent {
        event_type: 0,
        data: EbpfEventData {
            connect: TcpConnectEvent {
                pid,
                tid,
                family: AF_INET6,
                sport: 0,
                dport,
                saddr: [0u8; 16],
                daddr: addr_in6.sin6_addr,
            },
        },
    };

    EVENTS.output(&ctx, &event, 0);
    Ok(0)
}

#[kprobe]
pub fn kprobe_tcp_close(ctx: ProbeContext) -> u32 {
    match unsafe { try_kprobe_tcp_close(ctx) } {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

unsafe fn try_kprobe_tcp_close(ctx: ProbeContext) -> Result<u32, u32> {
    let sk_ptr: u64 = ctx.arg(0).ok_or(1u32)?;
    let pid_tgid = aya_ebpf::helpers::bpf_get_current_pid_tgid();
    let now = aya_ebpf::helpers::bpf_ktime_get_ns();
    let pid = (pid_tgid >> 32) as u32;
    let tid = pid_tgid as u32;

    let birth_time = unsafe { BIRTH_TIMES.get(&sk_ptr).copied().unwrap_or(now) };
    let duration = now.saturating_sub(birth_time);

    let skc: SockCommon =
        bpf_probe_read_kernel(sk_ptr as *const SockCommon).map_err(|_| 1u32)?;

    let is_v6 = skc.skc_family == AF_INET6;
    let saddr = if is_v6 {
        [0u8; 16]
    } else {
        ipv4_to_v6_mapped(skc.skc_rcv_saddr)
    };
    let daddr = if is_v6 {
        [0u8; 16]
    } else {
        ipv4_to_v6_mapped(skc.skc_daddr)
    };

    let event = EbpfEvent {
        event_type: 1,
        data: EbpfEventData {
            close: TcpCloseEvent {
                pid,
                tid,
                family: if is_v6 { AF_INET6 } else { AF_INET },
                sport: u16::from_be(skc.skc_num),
                dport: u16::from_be(skc.skc_dport),
                saddr,
                daddr,
                rx_bytes: 0,
                tx_bytes: 0,
                duration_ns: duration,
            },
        },
    };

    EVENTS.output(&ctx, &event, 0);
    Ok(0)
}

#[kretprobe]
pub fn kretprobe_inet_csk_accept(ctx: RetProbeContext) -> u32 {
    match unsafe { try_kretprobe_inet_csk_accept(ctx) } {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

unsafe fn try_kretprobe_inet_csk_accept(ctx: RetProbeContext) -> Result<u32, u32> {
    let new_sk_ptr: u64 = ctx.ret().ok_or(1u32)?;
    if new_sk_ptr == 0 {
        return Ok(0);
    }

    let pid_tgid = aya_ebpf::helpers::bpf_get_current_pid_tgid();
    let now = aya_ebpf::helpers::bpf_ktime_get_ns();
    let pid = (pid_tgid >> 32) as u32;
    let tid = pid_tgid as u32;

    unsafe { BIRTH_TIMES.insert(&new_sk_ptr, &now, 0).map_err(|_| 1u32)? };

    let skc: SockCommon =
        bpf_probe_read_kernel(new_sk_ptr as *const SockCommon).map_err(|_| 1u32)?;

    let is_v6 = skc.skc_family == AF_INET6;
    let saddr = if is_v6 {
        [0u8; 16]
    } else {
        ipv4_to_v6_mapped(skc.skc_rcv_saddr)
    };
    let daddr = if is_v6 {
        [0u8; 16]
    } else {
        ipv4_to_v6_mapped(skc.skc_daddr)
    };

    let event = EbpfEvent {
        event_type: 2,
        data: EbpfEventData {
            accept: TcpAcceptEvent {
                pid,
                tid,
                family: if is_v6 { AF_INET6 } else { AF_INET },
                sport: u16::from_be(skc.skc_num),
                dport: u16::from_be(skc.skc_dport),
                saddr,
                daddr,
            },
        },
    };

    EVENTS.output(&ctx, &event, 0);
    Ok(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
