//! 도구 카탈로그.
//!
//! 새 도구를 추가하려면 이 표에 한 항목만 넣는다. 도구 준비 화면, `doctor` 출력,
//! 미설치 안내, 묶음 안내가 모두 여기서 파생된다.

use super::{Applicability, Bundle, Tool};

/// 선택 인자를 `Option` 으로 바꾼다.
macro_rules! opt {
    () => {
        None
    };
    ($e:expr) => {
        Some($e)
    };
}

/// `only_if` 가 있으면 경로 조건, 없으면 항상 해당.
macro_rules! appl {
    () => {
        Applicability::Always
    };
    ($p:expr) => {
        Applicability::IfPathExists($p)
    };
}

macro_rules! tool {
    (
        $id:literal, $pkg:literal, [$($bin:literal),+], $bundle:ident, $pre:literal,
        $purpose:literal
        $(, post: $post:literal)?
        $(, without: $without:literal)?
        $(, only_if: $only:literal)?
    ) => {
        Tool {
            id: $id,
            package: $pkg,
            binaries: &[$($bin),+],
            purpose: $purpose,
            bundle: Bundle::$bundle,
            preinstalled: $pre,
            post_install: opt!($($post)?),
            without_it: opt!($($without)?),
            applicability: appl!($($only)?),
        }
    };
}

static TOOLS: &[Tool] = &[
    // ── core ────────────────────────────────────────────────────────
    tool!(
        "procps",
        "procps",
        ["ps", "free", "vmstat", "top"],
        Core,
        true,
        "Basic process, memory and load figures"
    ),
    tool!(
        "util-linux",
        "util-linux",
        ["lsblk", "findmnt", "dmesg"],
        Core,
        true,
        "Block device layout, mount table and kernel ring buffer"
    ),
    tool!(
        "systemd",
        "systemd",
        ["systemctl", "journalctl", "systemd-analyze"],
        Core,
        true,
        "Service state, logs and boot time analysis"
    ),
    tool!("sysstat", "sysstat", ["sar", "iostat", "mpstat", "pidstat"], Core, false,
        "Past system state: what CPU, memory and disk looked like hours or days ago",
        post: "Collection starts at install time, so history only exists from now on. It cannot show what happened before.",
        without: "Live figures are still available from /proc, but nothing retrospective."),
    tool!("lsof", "lsof", ["lsof"], Core, false,
        "Which program holds a file, a port, or deleted-but-still-used disk space",
        without: "/proc/PID/fd can be read directly, but not searched across all processes as easily."),
    tool!(
        "iproute2",
        "iproute2",
        ["ip", "ss"],
        Core,
        true,
        "Network addresses, routes and open sockets"
    ),
    tool!(
        "psmisc",
        "psmisc",
        ["fuser", "pstree"],
        Diagnostics,
        true,
        "Process trees and which process is using a mount or file"
    ),
    // ── storage ─────────────────────────────────────────────────────
    tool!("smartmontools", "smartmontools", ["smartctl"], Storage, false,
        "Drive self-diagnosis (SMART): early signs that a disk is failing",
        without: "Kernel I/O error messages hint at problems, but give no wear or reallocation counts."),
    tool!("nvme-cli", "nvme-cli", ["nvme"], Storage, false,
        "NVMe SSD health, wear level and temperature",
        only_if: "/dev/nvme0"),
    tool!(
        "hdparm",
        "hdparm",
        ["hdparm"],
        Storage,
        false,
        "Drive identity and read-speed measurement"
    ),
    tool!("lvm2", "lvm2", ["lvs", "vgs", "pvs"], Storage, false,
        "LVM volume layout, when disks are managed by LVM",
        only_if: "/sbin/lvm"),
    tool!("mdadm", "mdadm", ["mdadm"], Storage, false,
        "Software RAID array state",
        only_if: "/proc/mdstat"),
    tool!(
        "e2fsprogs",
        "e2fsprogs",
        ["tune2fs", "dumpe2fs"],
        Storage,
        true,
        "ext4 filesystem parameters and health flags"
    ),
    tool!(
        "xfsprogs",
        "xfsprogs",
        ["xfs_info"],
        Storage,
        false,
        "XFS filesystem geometry, if you use XFS"
    ),
    tool!(
        "btrfs-progs",
        "btrfs-progs",
        ["btrfs"],
        Storage,
        false,
        "Btrfs filesystem and device error counters"
    ),
    tool!(
        "zfsutils",
        "zfsutils-linux",
        ["zpool", "zfs"],
        Storage,
        false,
        "ZFS pool health and datasets"
    ),
    tool!(
        "multipath-tools",
        "multipath-tools",
        ["multipath"],
        Storage,
        false,
        "Multipath storage mapping, in SAN setups"
    ),
    tool!(
        "quota",
        "quota",
        ["repquota", "quota"],
        Storage,
        false,
        "Per-user disk quotas, when 'disk full' is really a quota"
    ),
    // ── network ─────────────────────────────────────────────────────
    tool!("dnsutils", "dnsutils", ["dig"], Network, false,
        "Name resolution testing: is DNS the problem",
        without: "getent hosts resolves names, but shows no timing or per-server detail."),
    tool!(
        "mtr",
        "mtr-tiny",
        ["mtr"],
        Network,
        false,
        "Where on the path packets are delayed or lost"
    ),
    tool!(
        "traceroute",
        "traceroute",
        ["traceroute", "tracepath"],
        Network,
        false,
        "The hops between here and a destination"
    ),
    tool!(
        "ethtool",
        "ethtool",
        ["ethtool"],
        Network,
        false,
        "Negotiated link speed and interface error counters"
    ),
    tool!(
        "tcpdump",
        "tcpdump",
        ["tcpdump"],
        Network,
        false,
        "The actual packets, when counters are not enough"
    ),
    tool!(
        "net-tools",
        "net-tools",
        ["netstat", "arp"],
        Network,
        false,
        "Classic network counters and the ARP table"
    ),
    tool!(
        "curl",
        "curl",
        ["curl"],
        Network,
        true,
        "Whether an HTTP or HTTPS endpoint answers, and how fast"
    ),
    tool!(
        "netcat",
        "netcat-openbsd",
        ["nc"],
        Network,
        false,
        "Whether a specific TCP port accepts connections"
    ),
    tool!(
        "iputils",
        "iputils-ping",
        ["ping"],
        Network,
        true,
        "Basic reachability and round-trip time"
    ),
    // ── hardware ────────────────────────────────────────────────────
    tool!(
        "dmidecode",
        "dmidecode",
        ["dmidecode"],
        Hardware,
        false,
        "What the machine is made of, as reported by the firmware"
    ),
    tool!(
        "pciutils",
        "pciutils",
        ["lspci"],
        Hardware,
        true,
        "Expansion cards and their PCIe link speed"
    ),
    tool!(
        "usbutils",
        "usbutils",
        ["lsusb"],
        Hardware,
        false,
        "Attached USB devices"
    ),
    tool!(
        "lshw",
        "lshw",
        ["lshw"],
        Hardware,
        false,
        "One combined hardware inventory listing"
    ),
    tool!("lm-sensors", "lm-sensors", ["sensors"], Hardware, false,
        "Temperature and fan readings",
        post: "Readings depend on the board. syschk only shows values from drivers it can trust, and marks the rest as unavailable rather than guessing."),
    tool!("rasdaemon", "rasdaemon", ["ras-mc-ctl"], Hardware, false,
        "Memory (ECC) and hardware error accounting",
        post: "Only records errors that happen after it starts running."),
    tool!("ipmitool", "ipmitool", ["ipmitool"], Hardware, false,
        "Board-level sensors and event log, on servers that have a BMC",
        only_if: "/dev/ipmi0"),
    tool!(
        "numactl",
        "numactl",
        ["numactl"],
        Hardware,
        false,
        "Memory layout across CPU sockets"
    ),
    tool!("nvidia-utils", "nvidia-utils-580", ["nvidia-smi"], Hardware, false,
        "NVIDIA GPU power, temperature, errors and link speed",
        only_if: "/proc/driver/nvidia"),
    tool!("rocm-smi", "rocm-smi-lib", ["rocm-smi"], Hardware, false,
        "AMD GPU utilisation and temperature",
        only_if: "/sys/module/amdgpu"),
    tool!("nut-client", "nut-client", ["upsc"], Hardware, false,
        "UPS status, if one is attached",
        only_if: "/etc/nut"),
    // ── diagnostics ─────────────────────────────────────────────────
    tool!("iotop", "iotop", ["iotop"], Diagnostics, false,
        "Which process is actually reading and writing the disk",
        without: "/proc/PID/io gives the same numbers per process, without the ranking."),
    tool!(
        "strace",
        "strace",
        ["strace"],
        Diagnostics,
        false,
        "What a stuck program is asking the kernel to do"
    ),
    tool!(
        "smem",
        "smem",
        ["smem"],
        Diagnostics,
        false,
        "Memory per process with shared pages accounted for"
    ),
    tool!(
        "linux-cpupower",
        "linux-tools-common",
        ["cpupower"],
        Diagnostics,
        false,
        "Whether the CPU is running at full speed or throttled"
    ),
    tool!("kdump-tools", "kdump-tools", ["kdump-config"], Diagnostics, false,
        "Captures a kernel crash dump, so a hard freeze leaves evidence",
        post: "Needs a reboot to reserve crash memory, and only helps for freezes that happen after that."),
    // ── updates ─────────────────────────────────────────────────────
    tool!(
        "needrestart",
        "needrestart",
        ["needrestart"],
        Updates,
        false,
        "Which running services still use old libraries after an update"
    ),
    tool!(
        "debsums",
        "debsums",
        ["debsums"],
        Updates,
        false,
        "Whether installed files still match the package they came from"
    ),
    tool!("apt-file", "apt-file", ["apt-file"], Updates, false,
        "Which package a file would come from",
        post: "Needs its index built once before searches work."),
    tool!(
        "snapd",
        "snapd",
        ["snap"],
        Updates,
        true,
        "Snap package list and recent changes"
    ),
    tool!("fail2ban", "fail2ban", ["fail2ban-client"], Diagnostics, false,
        "Whether repeated login attempts are being blocked",
        only_if: "/etc/fail2ban"),
    // ── containers ──────────────────────────────────────────────────
    tool!("docker", "docker.io", ["docker"], Containers, false,
        "Container state and resource usage",
        only_if: "/var/lib/docker"),
    tool!("podman", "podman", ["podman"], Containers, false,
        "Container state and resource usage (Podman)",
        only_if: "/etc/containers"),
    tool!("libvirt-clients", "libvirt-clients", ["virsh"], Containers, false,
        "Virtual machine state, on KVM hosts",
        only_if: "/var/run/libvirt"),
    // ── advanced ────────────────────────────────────────────────────
    tool!("linux-tools", "linux-tools-generic", ["perf"], Advanced, false,
        "Where CPU time actually goes, function by function",
        post: "Profiling adds load. syschk asks before running anything that does."),
    tool!(
        "bpfcc-tools",
        "bpfcc-tools",
        ["execsnoop", "biolatency"],
        Advanced,
        false,
        "Kernel-level latency and event tracing"
    ),
    tool!(
        "bpftrace",
        "bpftrace",
        ["bpftrace"],
        Advanced,
        false,
        "Ad-hoc kernel tracing for hard cases"
    ),
];

pub fn tools() -> &'static [Tool] {
    TOOLS
}

pub fn by_id(id: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.id == id)
}

/// 묶음에 속한 도구.
pub fn in_bundle(bundle: Bundle) -> Vec<&'static Tool> {
    TOOLS.iter().filter(|t| t.bundle == bundle).collect()
}

/// 이 도구를 필요로 하는 작업 목록(역참조). 도구 하나가 없으면 무엇을 못 하는지 보여준다.
pub fn tasks_needing(id: &str) -> Vec<&'static crate::tasks::Task> {
    crate::tasks::registry::tasks()
        .iter()
        .filter(|t| t.tools.contains(&id))
        .collect()
}
