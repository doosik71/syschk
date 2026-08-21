//! 읽기 전용 보장.
//!
//! syschk 는 시스템을 변경하지 않는다. 이 시험이 그 약속을 지킨다.

use syschk::tasks::registry as tasks;
use syschk::util::exec::ReadOnlyCommand;

/// 카탈로그가 선언한 모든 명령은 읽기 전용 정책을 통과해야 한다.
#[test]
fn every_catalogue_command_is_read_only() {
    let mut checked = 0;
    for task in tasks::tasks() {
        for line in task.commands {
            checked += 1;
            if let Err(e) = ReadOnlyCommand::parse(line) {
                panic!(
                    "task {} declares a command that is not read-only:\n  {line}\n  {e}",
                    task.id
                );
            }
        }
    }
    assert!(checked > 100, "expected a substantial command catalogue");
}

/// 수집기가 선언한 명령도 같은 정책을 통과해야 한다.
#[test]
fn every_probe_command_is_read_only() {
    for probe in syschk::collect::probes() {
        for line in probe.commands() {
            assert!(
                ReadOnlyCommand::parse(line).is_ok(),
                "probe {} declares a non read-only command: {line}",
                probe.id()
            );
        }
    }
}

/// 시스템을 바꾸는 명령은 만들 수조차 없어야 한다.
#[test]
fn mutating_commands_are_refused() {
    let forbidden = [
        "apt install sysstat",
        "apt-get install -y smartmontools",
        "apt remove nginx",
        "dpkg -i package.deb",
        "systemctl restart nginx",
        "systemctl stop docker",
        "systemctl enable kdump-tools",
        "sysctl -w kernel.panic_on_oops=1",
        "sysctl kernel.panic=1",
        "dmesg -C",
        "dmesg --clear",
        "journalctl --rotate",
        "journalctl --vacuum-size=100M",
        "smartctl -t short /dev/sda",
        "fsck /dev/sda1",
        "fsck -y /dev/sda1",
        "fstrim -av",
        "nvidia-smi -pl 200",
        "nvidia-smi -pm 1",
        "ip link set eth0 down",
        "ip addr add 10.0.0.1/24 dev eth0",
        "ip route del default",
        "ufw disable",
        "iptables -F",
        "nft flush ruleset",
        "tune2fs -m 1 /dev/sda1",
        "mdadm --stop /dev/md0",
        "zpool destroy tank",
        "docker system prune",
        "docker rm -f web",
        "kubectl delete pod web",
        "snap refresh",
        "needrestart -r a",
        "logrotate -f /etc/logrotate.conf",
        "loginctl terminate-session 3",
        "timedatectl set-ntp true",
        "hostnamectl set-hostname other",
        "swapon /swapfile",
        "chage -E 0 user",
        "passwd -d user",
        "fuser -k /mnt/data",
        "find / -name '*.tmp' -delete",
        "prlimit --nofile=1024 -p 1",
        "efibootmgr -o 0001",
        "cpupower frequency-set -g performance",
        "mount /dev/sdb1 /mnt",
        "unattended-upgrade",
        "crontab -r",
        "ethtool -K eth0 gro off",
    ];
    for line in forbidden {
        assert!(
            ReadOnlyCommand::parse(line).is_err(),
            "this should never be constructible: {line}"
        );
    }
}

/// 허용 목록에 없는 프로그램은 실행할 수 없다.
#[test]
fn unknown_programs_are_refused() {
    for line in [
        "rm -rf /",
        "mkfs.ext4 /dev/sdb1",
        "dd if=/dev/zero of=/dev/sda",
        "shutdown -h now",
        "reboot",
        "kill -9 1",
        "pkill nginx",
        "chmod 777 /etc/shadow",
        "sh -c 'echo hi'",
        "bash -c ls",
        "sudo apt install sysstat",
        "curlx https://example.com",
    ] {
        assert!(
            ReadOnlyCommand::parse(line).is_err(),
            "unknown or dangerous program accepted: {line}"
        );
    }
}

/// 정상적인 조회 명령은 통과해야 한다(정책이 과하게 막지 않는지 확인).
#[test]
fn read_only_commands_are_accepted() {
    for line in [
        "journalctl -k -b -1",
        "journalctl --list-boots",
        "journalctl --disk-usage",
        "systemctl --failed",
        "systemctl status ssh",
        "systemctl list-timers --all",
        "systemd-analyze critical-chain",
        "sar -q",
        "iostat -xz 1 5",
        "df -hT",
        "df -i",
        "du -xh --max-depth=1 /var",
        "lsof +L1",
        "smartctl -a /dev/sda",
        "smartctl -l error /dev/sda",
        "nvme smart-log /dev/nvme0",
        "lsblk -o NAME,SIZE,TYPE,ROTA,MOUNTPOINT",
        "ip -br a",
        "ip -s link",
        "ip r",
        "ss -tulpn",
        "ping -c4 1.1.1.1",
        "nc -zv example.com 443",
        "dig @1.1.1.1 ubuntu.com",
        "mtr -rwc10 1.1.1.1",
        "ethtool eth0",
        "ufw status verbose",
        "iptables -L -n -v",
        "sysctl vm.swappiness",
        "apt list --upgradable",
        "apt-get -s dist-upgrade",
        "dpkg --audit",
        "dpkg -S /usr/bin/ls",
        "needrestart -r l",
        "snap refresh --list",
        "docker system df",
        "docker stats --no-stream",
        "nvidia-smi --query-gpu=power.draw,power.limit --format=csv",
        "sensors",
        "dmidecode -t 39",
        "ras-mc-ctl --summary",
        "fsck -N /dev/sda1",
        "fstrim -av --dry-run",
        "tune2fs -l /dev/sda1",
        "hdparm -I /dev/sda",
        "top -b -n1",
        "iotop -bon1",
        "strace -f -p 1234 -c",
        "logrotate -d /etc/logrotate.conf",
        "unattended-upgrade --dry-run",
        "passwd -S -a",
        "crontab -l",
        "cat /proc/pressure/io",
        "cat /sys/devices/system/edac/mc/mc0/ce_count",
    ] {
        if let Err(e) = ReadOnlyCommand::parse(line) {
            panic!("legitimate read-only command was refused: {line}\n  {e}");
        }
    }
}
