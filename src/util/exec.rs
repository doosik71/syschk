//! 읽기 전용 명령 실행 게이트.
//!
//! syschk 의 목적은 **정밀하지만 비파괴적인 진단**이다. 시스템을 변경하는 명령은
//! 실행하지 않으며, 이 원칙을 문서가 아니라 타입과 검증으로 보장한다.
//!
//! 외부 명령은 [`ReadOnlyCommand`] 로만 만들 수 있고, 생성 시점에 프로그램별 정책을
//! 통과해야 한다. 정책은 다음을 검사한다.
//!
//! 1. 프로그램이 허용 목록에 있는가 (모르는 프로그램은 실행 불가)
//! 2. 전역 금지 접두어를 쓰지 않는가 (`--set…`, `--delete…` 등)
//! 3. 프로그램별 금지 인자를 쓰지 않는가 (`dmesg -C`, `sysctl -w`, `smartctl -t` 등)
//! 4. 조회/변경이 섞인 프로그램에서 허용된 서브커맨드만 쓰는가 (`systemctl status` 등)
//! 5. 반드시 필요한 안전 인자를 포함하는가 (`fsck -N`, `fstrim --dry-run` 등)
//!
//! 새 수집기를 추가할 때 이 표에 프로그램을 등록한다. 등록되지 않은 프로그램은
//! 애초에 실행할 수 없으므로, 실수로 시스템을 바꾸는 명령이 들어갈 수 없다.

use std::process::Command;
use std::time::{Duration, Instant};

/// 정책 위반 사유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// 허용 목록에 없는 프로그램.
    UnknownProgram(String),
    /// 프로그램은 허용되지만 인자가 시스템을 변경한다.
    ForbiddenArgument { program: String, argument: String },
    /// 허용되지 않은 서브커맨드.
    ForbiddenSubcommand { program: String, subcommand: String },
    /// 안전 인자가 빠졌다(예: `fsck` 는 `-N` 없이는 쓰지 않는다).
    MissingRequiredArgument { program: String, required: String },
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::UnknownProgram(p) => {
                write!(f, "'{p}' is not on the read-only allowlist")
            }
            PolicyError::ForbiddenArgument { program, argument } => {
                write!(f, "'{program} … {argument}' could modify the system")
            }
            PolicyError::ForbiddenSubcommand {
                program,
                subcommand,
            } => write!(f, "'{program} {subcommand}' is not a read-only operation"),
            PolicyError::MissingRequiredArgument { program, required } => {
                write!(f, "'{program}' may only run with: {required}")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

/// 프로그램별 읽기 전용 정책.
#[derive(Debug, Clone, Copy)]
struct Policy {
    /// 이 토큰이 인자에 정확히 일치하면 거부한다.
    forbidden: &'static [&'static str],
    /// 인자 중 최소 하나는 이 목록에 포함되어야 한다(비어 있으면 검사 생략).
    require_any: &'static [&'static str],
    /// 첫 번째 비플래그 인자가 이 목록에 있어야 한다(비어 있으면 검사 생략).
    subcommands: &'static [&'static str],
    /// (서브커맨드, 함께 있어야 하는 인자) — 조회 모드를 강제할 때 쓴다.
    conditional: &'static [(&'static str, &'static str)],
}

const RO: Policy = Policy {
    forbidden: &[],
    require_any: &[],
    subcommands: &[],
    conditional: &[],
};

impl Policy {
    const fn forbid(mut self, v: &'static [&'static str]) -> Self {
        self.forbidden = v;
        self
    }
    const fn require(mut self, v: &'static [&'static str]) -> Self {
        self.require_any = v;
        self
    }
    const fn subs(mut self, v: &'static [&'static str]) -> Self {
        self.subcommands = v;
        self
    }
    const fn cond(mut self, v: &'static [(&'static str, &'static str)]) -> Self {
        self.conditional = v;
        self
    }
}

/// 어떤 프로그램에서도 상태 변경을 뜻하는 접두어.
const GLOBAL_FORBIDDEN_PREFIXES: &[&str] = &[
    "--set",
    "--write",
    "--delete",
    "--remove",
    "--create",
    "--force",
    "--erase",
    "--reset",
    "--flush",
    "--vacuum",
    "--rotate",
    "--install",
    "--purge",
];

/// 상태를 바꾸는 것이 명백한 공통 토큰.
const MUTATE: &[&str] = &[
    "add",
    "del",
    "delete",
    "set",
    "flush",
    "change",
    "replace",
    "install",
    "remove",
    "purge",
    "start",
    "stop",
    "restart",
    "reload",
    "enable",
    "disable",
    "mask",
    "unmask",
    "kill",
    "destroy",
    "create",
    "prune",
    "rm",
    "rmi",
    "exec",
    "run",
    "build",
    "push",
    "apply",
    "edit",
    "patch",
    "scale",
    "drain",
    "cordon",
    "uncordon",
    "shutdown",
    "reboot",
    "undefine",
    "launch",
    "move",
    "publish",
    "snapshot",
    "rollback",
    "send",
    "receive",
    "mount",
    "unmount",
    "umount",
    "scrub",
    "clear",
    "attach",
    "detach",
    "offline",
    "online",
    "upgrade",
    "autoremove",
];

/// 허용 프로그램 표. `(프로그램, 정책)`.
static ALLOWLIST: &[(&str, Policy)] = &[
    // ── 순수 조회 도구 ──────────────────────────────────────────────
    ("aa-status", RO),
    ("apcaccess", RO),
    ("biolatency", RO),
    ("biosnoop", RO),
    ("blkid", RO),
    ("bpftrace", RO),
    ("cat", RO),
    ("dig", RO),
    ("dumpe2fs", RO),
    ("execsnoop", RO),
    ("findmnt", RO),
    ("free", RO),
    ("getent", RO),
    ("head", RO),
    ("id", RO),
    ("iostat", RO),
    ("last", RO),
    ("lastb", RO),
    ("lastlog", RO),
    ("ls", RO),
    ("lsblk", RO),
    ("lscpu", RO),
    ("lshw", RO),
    ("lsmem", RO),
    ("lsmod", RO),
    ("lsof", RO),
    ("lspci", RO),
    ("lsscsi", RO),
    ("lsusb", RO),
    ("modinfo", RO),
    ("mpstat", RO),
    ("mtr", RO),
    ("netstat", RO),
    ("nfsstat", RO),
    ("nstat", RO),
    ("opensnoop", RO),
    ("pgrep", RO),
    ("pidstat", RO),
    ("pmap", RO),
    ("ps", RO),
    ("pstree", RO),
    ("pvs", RO),
    ("lvs", RO),
    ("vgs", RO),
    (
        "dmsetup",
        RO.subs(&["status", "info", "ls", "table", "deps"]),
    ),
    ("quota", RO),
    ("repquota", RO),
    ("runqlat", RO),
    ("sadf", RO),
    ("sar", RO),
    ("sensors", RO),
    ("showmount", RO),
    ("smem", RO),
    ("ss", RO),
    ("stat", RO),
    ("systemd-cgtop", RO),
    ("systemd-detect-virt", RO),
    ("tail", RO),
    ("tcpretrans", RO),
    ("traceroute", RO),
    ("tracepath", RO),
    ("uname", RO),
    ("uptime", RO),
    ("upsc", RO),
    ("vmstat", RO),
    ("w", RO),
    ("who", RO),
    ("xfs_info", RO),
    ("df", RO),
    ("du", RO),
    // ── 조회와 변경이 섞인 도구 ─────────────────────────────────────
    (
        "apt",
        RO.forbid(MUTATE)
            .subs(&["list", "policy", "show", "search", "depends", "rdepends"]),
    ),
    (
        "apt-cache",
        RO.forbid(MUTATE)
            .subs(&["policy", "show", "showpkg", "depends", "madison", "stats"]),
    ),
    (
        "apt-file",
        RO.forbid(&["update"]).subs(&["search", "list", "show"]),
    ),
    // 시뮬레이션 전용. `-s` 없이는 실행 불가.
    (
        "apt-get",
        RO.require(&["-s", "--simulate", "--dry-run", "check"]),
    ),
    (
        "apt-mark",
        RO.forbid(&["hold", "unhold", "auto", "manual"]).subs(&[
            "showhold",
            "showauto",
            "showmanual",
        ]),
    ),
    ("arp", RO.forbid(&["-d", "-s", "-f"])),
    (
        "btrfs",
        RO.forbid(MUTATE)
            .require(&["show", "stats", "usage", "df", "list"])
            .subs(&["filesystem", "device", "subvolume", "qgroup"]),
    ),
    ("canonical-livepatch", RO.forbid(MUTATE).subs(&["status"])),
    ("chage", RO.require(&["-l", "--list"])),
    (
        "chronyc",
        RO.forbid(&[
            "makestep", "burst", "password", "shutdown", "trimrtc", "writertc",
        ])
        .subs(&[
            "tracking",
            "sources",
            "sourcestats",
            "activity",
            "clients",
            "ntpdata",
            "serverstats",
        ]),
    ),
    (
        "conntrack",
        RO.forbid(&["-D", "-F", "-U"])
            .require(&["-S", "-L", "--stats", "--dump"]),
    ),
    (
        "coredumpctl",
        RO.forbid(&["debug", "dump", "gdb"]).subs(&["list", "info"]),
    ),
    (
        "cpupower",
        RO.forbid(&["frequency-set", "idle-set", "set"]).subs(&[
            "frequency-info",
            "idle-info",
            "info",
            "monitor",
        ]),
    ),
    (
        "crictl",
        RO.forbid(MUTATE)
            .subs(&["ps", "images", "info", "stats", "logs", "inspect", "pods"]),
    ),
    ("crontab", RO.forbid(&["-r", "-e", "-i"]).require(&["-l"])),
    ("apt-check", RO),
    (
        "curl",
        RO.forbid(&[
            "-X",
            "-d",
            "--data",
            "--data-raw",
            "--data-binary",
            "-T",
            "--upload-file",
            "-F",
            "--form",
        ]),
    ),
    (
        "debsums",
        RO.require(&["-c", "-s", "--changed", "--silent"]),
    ),
    ("dkms", RO.forbid(MUTATE).subs(&["status"])),
    // `dmesg -C` 는 커널 링버퍼를 지운다 — 진단 자료 파괴이므로 차단.
    (
        "dmesg",
        RO.forbid(&[
            "-C",
            "--clear",
            "-c",
            "--read-clear",
            "-D",
            "-E",
            "-n",
            "--console-off",
            "--console-on",
            "--console-level",
        ]),
    ),
    ("dmidecode", RO),
    (
        "docker",
        RO.forbid(MUTATE)
            .subs(&[
                "ps",
                "stats",
                "logs",
                "inspect",
                "info",
                "system",
                "image",
                "images",
                "events",
                "version",
                "container",
                "volume",
                "network",
            ])
            .cond(&[
                ("system", "df"),
                ("image", "ls"),
                ("images", "ls"),
                ("volume", "ls"),
                ("network", "ls"),
                ("container", "ls"),
            ]),
    ),
    (
        "dpkg",
        RO.forbid(&[
            "-i",
            "--install",
            "-r",
            "--remove",
            "-P",
            "--purge",
            "--unpack",
            "--configure",
        ])
        .require(&[
            "-l",
            "-L",
            "-S",
            "-s",
            "--list",
            "--audit",
            "--status",
            "--search",
            "--listfiles",
            "--get-selections",
        ]),
    ),
    ("dpkg-query", RO),
    (
        "efibootmgr",
        RO.forbid(&[
            "-c", "-o", "-b", "-B", "-a", "-A", "-d", "-t", "-T", "-n", "-N", "-D",
        ])
        .require(&["-v", "--verbose"]),
    ),
    (
        "ethtool",
        RO.forbid(&[
            "-K", "-G", "-C", "-s", "-L", "-N", "-U", "-E", "-f", "-A", "-p", "-r",
        ]),
    ),
    (
        "fail2ban-client",
        RO.forbid(MUTATE)
            .subs(&["status", "get", "ping", "version"]),
    ),
    ("faillock", RO.forbid(&["--reset"])),
    // 실제 검사·수정 금지. 무동작(-N)만 허용.
    (
        "fsck",
        RO.forbid(&["-y", "-p", "-a", "-f", "-r"]).require(&["-N"]),
    ),
    ("fstrim", RO.require(&["--dry-run", "-n"])),
    (
        "find",
        RO.forbid(&[
            "-delete", "-exec", "-execdir", "-ok", "-okdir", "-fls", "-fprint", "-fprintf",
        ]),
    ),
    // `fuser -k` 는 프로세스를 죽인다.
    ("fuser", RO.forbid(&["-k", "-K", "-M", "-i"])),
    (
        "hdparm",
        RO.forbid(&[
            "-W", "-B", "-S", "-M", "-K", "-k", "-Y", "-y", "-Z", "-p", "-P", "-Q", "-w",
        ])
        .require(&["-I", "-i", "-t", "-T", "-tT", "--Istdout"]),
    ),
    (
        "hostnamectl",
        RO.forbid(&[
            "set-hostname",
            "set-icon-name",
            "set-chassis",
            "set-deployment",
            "set-location",
        ])
        .subs(&["status", "show"]),
    ),
    (
        "iotop",
        RO.require(&["-b", "-bon1", "--batch", "--version"]),
    ),
    // 전원 제어·부팅 장치 변경 차단.
    (
        "ipmitool",
        RO.forbid(&["power", "reset", "bootdev", "raw", "sol", "chassis"])
            .subs(&["sdr", "sel", "sensor", "fru", "mc", "lan", "user"]),
    ),
    (
        "ip",
        RO.forbid(MUTATE).subs(&[
            "addr",
            "address",
            "a",
            "link",
            "l",
            "route",
            "r",
            "rule",
            "neigh",
            "neighbour",
            "n",
            "maddr",
            "mroute",
            "netns",
            "tunnel",
            "get",
        ]),
    ),
    (
        "iptables",
        RO.forbid(&["-A", "-D", "-I", "-R", "-F", "-X", "-P", "-Z", "-N", "-E"])
            .require(&["-L", "-S", "--list", "--list-rules"]),
    ),
    (
        "ip6tables",
        RO.forbid(&["-A", "-D", "-I", "-R", "-F", "-X", "-P", "-Z", "-N", "-E"])
            .require(&["-L", "-S", "--list", "--list-rules"]),
    ),
    // 저널 회전·삭제·키 생성 차단.
    (
        "journalctl",
        RO.forbid(&[
            "--rotate",
            "--sync",
            "--relinquish-var",
            "--setup-keys",
            "--update-catalog",
        ]),
    ),
    (
        "kdump-config",
        RO.forbid(MUTATE).subs(&["show", "status", "test"]),
    ),
    (
        "kubectl",
        RO.forbid(MUTATE).subs(&[
            "get",
            "describe",
            "top",
            "version",
            "cluster-info",
            "explain",
            "api-resources",
        ]),
    ),
    (
        "loginctl",
        RO.forbid(&[
            "terminate-session",
            "kill-session",
            "lock-session",
            "unlock-session",
            "terminate-user",
            "kill-user",
            "activate",
            "terminate-seat",
        ])
        .subs(&[
            "list-sessions",
            "session-status",
            "show-session",
            "list-users",
            "user-status",
            "show-user",
            "list-seats",
            "seat-status",
        ]),
    ),
    // 실제 회전 금지. 모의 실행(-d)만 허용.
    (
        "logrotate",
        RO.forbid(&["-f", "--force"]).require(&["-d", "--debug"]),
    ),
    (
        "lxc",
        RO.forbid(MUTATE)
            .subs(&["list", "info", "config"])
            .cond(&[("config", "show")]),
    ),
    ("ltrace", RO.require(&["-p"])),
    (
        "mdadm",
        RO.forbid(&[
            "--stop",
            "--fail",
            "--add",
            "--remove",
            "--zero-superblock",
            "--create",
            "-C",
            "-S",
            "-f",
            "-a",
            "-r",
            "--grow",
            "-G",
            "--assemble",
            "-A",
        ])
        .require(&[
            "--detail",
            "-D",
            "--examine",
            "-E",
            "--query",
            "-Q",
            "--detail-platform",
        ]),
    ),
    ("mount", RO.forbid(MUTATE).subs(&["-l", "-t"])),
    (
        "multipath",
        RO.forbid(&["-F", "-f", "-r", "-W"])
            .require(&["-ll", "-l", "-t", "-T"]),
    ),
    ("nc", RO.forbid(&["-l", "-e", "-c", "-k"]).require(&["-z"])),
    (
        "needrestart",
        RO.forbid(&["a", "-u", "i"]).require(&["-r", "-l", "-p"]),
    ),
    (
        "netplan",
        RO.forbid(&["apply", "try", "generate", "set"])
            .subs(&["get", "status", "info"]),
    ),
    ("nft", RO.forbid(MUTATE).subs(&["list"])),
    (
        "nmcli",
        RO.forbid(MUTATE)
            .subs(&[
                "device",
                "dev",
                "d",
                "connection",
                "con",
                "c",
                "general",
                "g",
                "networking",
                "n",
                "radio",
                "r",
            ])
            .cond(&[("device", "status"), ("connection", "show")]),
    ),
    ("numactl", RO.require(&["-H", "--hardware", "-s", "--show"])),
    (
        "nvme",
        RO.forbid(MUTATE).subs(&[
            "list",
            "smart-log",
            "error-log",
            "id-ctrl",
            "id-ns",
            "list-ns",
            "fw-log",
            "self-test-log",
        ]),
    ),
    // 전력 상한·persistence·ECC 초기화 등 설정 변경 플래그 차단.
    (
        "nvidia-smi",
        RO.forbid(&[
            "-pl", "-pm", "-e", "-r", "-c", "-ac", "-rac", "-lgc", "-rgc", "-lmc",
        ]),
    ),
    ("ntpq", RO.require(&["-p", "-c"])),
    (
        "passwd",
        RO.forbid(&["-d", "-l", "-u", "-e", "-x", "-n", "-w", "-i"])
            .require(&["-S"]),
    ),
    (
        "perf",
        RO.forbid(MUTATE).subs(&[
            "stat", "record", "report", "top", "trace", "sched", "list", "script", "annotate",
        ]),
    ),
    ("ping", RO.forbid(&["-f"])),
    ("prlimit", RO.forbid(&["="]).require(&["-p", "--pid"])),
    (
        "ras-mc-ctl",
        RO.forbid(&["--register-labels", "--guess-labels"])
            .require(&["--summary", "--errors", "--status", "--layout"]),
    ),
    (
        "resolvectl",
        RO.forbid(&[
            "flush-caches",
            "revert",
            "reset-server-features",
            "reset-statistics",
            "dnssec",
            "dnsovertls",
        ])
        .subs(&[
            "status",
            "query",
            "statistics",
            "dns",
            "domain",
            "llmnr",
            "mdns",
        ]),
    ),
    (
        "rocm-smi",
        RO.forbid(&[
            "--setfan",
            "--setperflevel",
            "--setoverdrive",
            "--setpoweroverdrive",
            "--resetclocks",
            "--resetfans",
            "-r",
        ]),
    ),
    // 자기 진단 실행(-t)은 장치에 부하를 주므로 차단. 조회만 허용.
    (
        "smartctl",
        RO.forbid(&[
            "-t", "--test", "-X", "--abort", "-s", "--smart", "-o", "-S", "-F",
        ]),
    ),
    ("slabtop", RO.require(&["-o", "--once"])),
    (
        "snap",
        RO.forbid(MUTATE)
            .subs(&[
                "list",
                "changes",
                "refresh",
                "info",
                "version",
                "connections",
            ])
            .cond(&[("refresh", "--list")]),
    ),
    ("sshd", RO.forbid(&["-D"]).require(&["-T", "-t"])),
    ("strace", RO.require(&["-p"])),
    (
        "swapon",
        RO.forbid(&["-a", "--all"])
            .require(&["--show", "-s", "--summary"]),
    ),
    ("sysctl", RO.forbid(&["-w", "--write", "-p", "--load", "="])),
    (
        "systemctl",
        RO.forbid(MUTATE).subs(&[
            "status",
            "show",
            "cat",
            "list-units",
            "list-unit-files",
            "list-timers",
            "list-dependencies",
            "list-sockets",
            "list-jobs",
            "is-active",
            "is-enabled",
            "is-failed",
            "is-system-running",
            "get-default",
            "show-environment",
        ]),
    ),
    (
        "systemd-analyze",
        RO.forbid(&["set-log-level", "set-log-target", "service-watchdogs"])
            .subs(&[
                "blame",
                "critical-chain",
                "time",
                "plot",
                "dump",
                "verify",
                "security",
                "calendar",
                "timestamp",
                "unit-paths",
            ]),
    ),
    ("tc", RO.forbid(MUTATE).require(&["show", "-s"])),
    (
        "timedatectl",
        RO.forbid(&["set-time", "set-timezone", "set-ntp", "set-local-rtc"])
            .subs(&[
                "status",
                "show",
                "timesync-status",
                "show-timesync",
                "list-timezones",
            ]),
    ),
    ("top", RO.require(&["-b", "-v", "-h"])),
    // 실제 파일시스템 파라미터 변경 차단. 수퍼블록 조회(-l)만 허용.
    (
        "tune2fs",
        RO.forbid(&[
            "-c", "-i", "-m", "-L", "-U", "-O", "-e", "-r", "-C", "-T", "-E", "-I", "-g", "-u",
        ])
        .require(&["-l"]),
    ),
    ("tcpdump", RO.forbid(&["-z", "-Z"])),
    ("tshark", RO.require(&["-r"])),
    ("ufw", RO.forbid(MUTATE).subs(&["status", "show"])),
    ("unattended-upgrade", RO.require(&["--dry-run"])),
    (
        "virsh",
        RO.forbid(MUTATE).subs(&[
            "list",
            "domstats",
            "dominfo",
            "nodeinfo",
            "version",
            "domblklist",
            "capabilities",
        ]),
    ),
    ("zfs", RO.forbid(MUTATE).subs(&["list", "get"])),
    (
        "zpool",
        RO.forbid(MUTATE)
            .subs(&["status", "list", "iostat", "get", "history"]),
    ),
];

fn policy_for(program: &str) -> Option<Policy> {
    ALLOWLIST
        .iter()
        .find(|(name, _)| *name == program)
        .map(|(_, p)| *p)
}

/// 허용 목록에 있는 프로그램 이름 전체. `doctor` 출력과 시험에 쓴다.
pub fn allowlisted_programs() -> Vec<&'static str> {
    ALLOWLIST.iter().map(|(n, _)| *n).collect()
}

/// 인자가 정책 토큰과 일치하는지 본다.
///
/// `-zv` 처럼 짧은 플래그가 묶여 있는 경우도 `-z` 와 일치로 본다. 금지 검사에서는
/// 더 엄격하게, 필수 검사에서는 실제 사용 형태를 인정하기 위해 필요하다.
fn arg_matches(arg: &str, token: &str) -> bool {
    // "=" 은 `key=value` 형태(값 설정)를 뜻하는 특수 규칙.
    if token == "=" {
        return arg.contains('=');
    }
    if arg == token {
        return true;
    }
    let is_short_flag = token.len() == 2 && token.starts_with('-');
    let arg_is_cluster = arg.starts_with('-') && !arg.starts_with("--");
    if is_short_flag && arg_is_cluster {
        let letter = token.as_bytes()[1];
        return arg.bytes().skip(1).any(|b| b == letter);
    }
    false
}

/// 프로그램 + 인자가 읽기 전용 정책을 만족하는지 검사한다.
pub fn check(program: &str, args: &[String]) -> Result<(), PolicyError> {
    // 경로가 붙어 있어도 기본 이름으로 판단한다.
    let base = program.rsplit('/').next().unwrap_or(program);
    let policy = policy_for(base).ok_or_else(|| PolicyError::UnknownProgram(base.to_string()))?;

    for arg in args {
        for prefix in GLOBAL_FORBIDDEN_PREFIXES {
            if arg.starts_with(prefix) {
                return Err(PolicyError::ForbiddenArgument {
                    program: base.to_string(),
                    argument: arg.clone(),
                });
            }
        }
        for bad in policy.forbidden {
            if arg_matches(arg, bad) {
                return Err(PolicyError::ForbiddenArgument {
                    program: base.to_string(),
                    argument: arg.clone(),
                });
            }
        }
    }

    // 첫 번째 비플래그 인자를 서브커맨드로 본다.
    let subcommand = args.iter().find(|a| !a.starts_with('-'));
    if !policy.subcommands.is_empty()
        && let Some(sub) = subcommand
        && !policy.subcommands.contains(&sub.as_str())
    {
        return Err(PolicyError::ForbiddenSubcommand {
            program: base.to_string(),
            subcommand: sub.clone(),
        });
    }

    for (sub, required) in policy.conditional {
        if args.iter().any(|a| a == sub) && !args.iter().any(|a| a == required) {
            return Err(PolicyError::MissingRequiredArgument {
                program: base.to_string(),
                required: format!("{sub} {required}"),
            });
        }
    }

    if !policy.require_any.is_empty()
        && !args
            .iter()
            .any(|a| policy.require_any.iter().any(|r| arg_matches(a, r)))
    {
        return Err(PolicyError::MissingRequiredArgument {
            program: base.to_string(),
            required: policy.require_any.join(" | "),
        });
    }

    Ok(())
}

/// 정책을 통과한, 실행 가능한 읽기 전용 명령.
///
/// 이 타입을 거치지 않고 외부 명령을 실행하는 코드는 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyCommand {
    program: String,
    args: Vec<String>,
}

impl ReadOnlyCommand {
    /// 정책을 검사하고 명령을 만든다. 위반 시 `Err`.
    pub fn new<S: AsRef<str>>(program: &str, args: &[S]) -> Result<Self, PolicyError> {
        let args: Vec<String> = args.iter().map(|a| a.as_ref().to_string()).collect();
        check(program, &args)?;
        Ok(Self {
            program: program.to_string(),
            args,
        })
    }

    /// 공백으로 구분된 한 줄에서 만든다. 카탈로그 검증과 표시에 쓴다.
    pub fn parse(line: &str) -> Result<Self, PolicyError> {
        let mut it = line.split_whitespace();
        let program = it.next().unwrap_or_default().to_string();
        let args: Vec<String> = it.map(str::to_string).collect();
        check(&program, &args)?;
        Ok(Self { program, args })
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// 사용자에게 근거로 보여줄 문자열.
    pub fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }

    /// 명령을 실행한다. 실패도 값으로 돌려준다(오류를 전파하지 않는다).
    pub fn run(&self) -> CommandOutput {
        let started = Instant::now();
        let result = Command::new(&self.program).args(&self.args).output();
        let elapsed = started.elapsed();
        match result {
            Ok(out) => CommandOutput {
                command: self.display(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                exit_code: out.status.code(),
                elapsed,
                spawn_error: None,
            },
            Err(e) => CommandOutput {
                command: self.display(),
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
                elapsed,
                spawn_error: Some(e.to_string()),
            },
        }
    }
}

/// 실행 결과 원본. 보고서와 근거 표시에 그대로 쓰인다.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub elapsed: Duration,
    pub spawn_error: Option<String>,
}

impl CommandOutput {
    pub fn succeeded(&self) -> bool {
        self.spawn_error.is_none() && self.exit_code == Some(0)
    }
}

/// 프로그램이 PATH 에 있는지 확인한다. (`which` 를 실행하지 않는다)
pub fn find_in_path(binary: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|c| is_executable(c))
}

fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
