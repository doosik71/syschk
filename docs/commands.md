# 지원 명령 카탈로그

사용자가 입력하지 않는, **앱이 내부에서 사용하는** 명령 목록이다. 사용자에게는 왼쪽 열(하고 싶은 일)만 보인다.
`temp/diagnosis.md` 의 사례에서 쓰인 명령은 이 표의 일부이며, 우분투 진단에서 통상적으로 쓰이는 범위를 함께 담았다.

여기 적힌 명령은 **전부 읽기 전용**이다. 실행 전 [읽기 전용 정책](scope.md#안전-보장-방식)을 통과해야 하며,
정책을 통과하지 못하는 명령은 카탈로그에 들어갈 수 없다(`tests/policy.rs`).

각 명령은 미설치·권한 부족·미지원을 개별적으로 처리한다. 없으면 그 항목만 "계측 불가(이유)"가 되고 나머지는 정상 동작한다.

실제로 어떤 작업이 어떤 도구를 필요로 하는지는 [screens.md](screens.md) 의 표에서 확인할 수 있다.

## A. 시스템 기본 정보

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 이 시스템이 무엇인가 | `hostnamectl`, `uname -a`, `cat /etc/os-release`, `lsb_release -a` |
| 언제부터 켜져 있나 | `uptime`, `uptime -s`, `cat /proc/uptime`, `who -b`, `last reboot` |
| CPU·메모리 구성 | `lscpu`, `lscpu -e`, `lsmem`, `numactl -H`, `cat /proc/cpuinfo` |
| 하드웨어 인벤토리 | `lshw -short`, `dmidecode -t system -t baseboard -t bios -t processor -t memory` |
| 물리인가 가상인가 | `systemd-detect-virt`, `dmidecode -s system-product-name` |
| 커널 부팅 인자 | `cat /proc/cmdline` |
| 시간이 맞나 | `timedatectl`, `chronyc tracking`, `chronyc sources -v`, `systemctl status systemd-timesyncd`, `ntpq -p` |

## B. CPU · 부하

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 현재 사용률과 부하 | `top -b -n1`, `cat /proc/loadavg`, `mpstat -P ALL 1 3`, `vmstat 1 5` |
| CPU를 쓰는 프로세스 | `ps -eo pid,ppid,user,pcpu,pmem,stat,etime,args --sort=-pcpu`, `pidstat -u 1 3` |
| CPU 압박 정도 | `cat /proc/pressure/cpu`, `vmstat`의 실행 대기·컨텍스트 스위치 |
| 가상화 환경의 자원 도난 | `mpstat`의 `%steal`, `vmstat`의 `st` |
| 주파수·스로틀 | `cpupower frequency-info`, `cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq`, `turbostat`, `cat /proc/cpuinfo` |
| 인터럽트 편중 | `cat /proc/interrupts`, `cat /proc/softirqs`, `mpstat -I SUM` |
| 과거 CPU 추이 | `sar -u`, `sar -q`, `sar -P ALL` |

## C. 메모리 · 스왑

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 여유 메모리 | `free -h`, `cat /proc/meminfo`, `vmstat -s` |
| 메모리를 쓰는 프로세스 | `ps -eo pid,user,rss,vsz,pmem,args --sort=-rss`, `smem -tk`, `pmap -x PID` |
| 캐시와 실사용 구분 | `/proc/meminfo`의 available·cached·buffers·slab |
| 메모리 압박·회수 | `cat /proc/pressure/memory`, `vmstat`의 `si/so`, `sar -B`, `sar -W` |
| 스왑 사용 | `swapon --show`, `sar -S`, 프로세스별 `/proc/PID/status`의 VmSwap |
| 커널 메모리 소비 | `slabtop -o`, `cat /proc/slabinfo`, `cat /proc/buddyinfo` |
| OOM 발생 여부 | `journalctl -k --grep "Out of memory\|oom-kill\|killed process"`, `dmesg -T` |
| cgroup 한계 | `systemd-cgtop`, `cat /sys/fs/cgroup/<slice>/memory.{current,max,events}` |
| 과거 메모리 추이 | `sar -r`, `sar -R` |
| 대용량 페이지 | `cat /proc/meminfo`, `cat /sys/kernel/mm/transparent_hugepage/enabled` |

## D. 디스크 공간

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 마운트별 사용률 | `df -hT`, `df -i`, `findmnt -D` |
| 무엇이 공간을 먹나 | `du -xh --max-depth=1`, `find / -xdev -size +1G -printf ...`, `ncdu -x` |
| 지웠는데 안 줄어드는 공간 | `lsof +L1`, `lsof -nP`, `fuser -v` |
| inode 고갈 | `df -i`, `find DIR -xdev -printf "%h\n"` 집계 |
| 로그가 먹는 공간 | `journalctl --disk-usage`, `du -sh /var/log/*`, `logrotate -d /etc/logrotate.conf` |
| 예약 블록·쿼터 | `tune2fs -l DEV`, `repquota -a`, `quota -u USER` |
| 사용하지 않는 블록 정리 상태 | `systemctl status fstrim.timer`, `fstrim -av --dry-run` |

## E. 저장장치 · I/O

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 장치별 I/O 지연·큐 | `iostat -xz 1 5`, `sar -d -p`, `sar -b` |
| I/O를 일으키는 프로세스 | `pidstat -d 1 3`, `iotop -bon1`, `cat /proc/PID/io` |
| I/O 대기 프로세스 | `ps -eo pid,stat,wchan:24,args`, `cat /proc/pressure/io` |
| 디스크 건전성(SATA/SAS) | `smartctl -H DEV`, `smartctl -A DEV`, `smartctl -l error DEV`, `smartctl -a DEV` |
| 디스크 건전성(NVMe) | `nvme list`, `nvme smart-log DEV`, `nvme error-log DEV`, `nvme id-ctrl DEV` |
| 저장장치 오류 기록 | `journalctl -k --grep "I/O error\|ata[0-9]\|nvme\|reset\|medium error"`, `/var/log/kern.log` |
| 장치 구성 | `lsblk -o NAME,SIZE,TYPE,ROTA,FSTYPE,MOUNTPOINT,MODEL,SERIAL`, `blkid`, `ls -l /dev/disk/by-*` |
| RAID 상태 | `cat /proc/mdstat`, `mdadm --detail DEV`, `zpool status`, `zpool list` |
| LVM 구성 | `pvs`, `vgs`, `lvs -a -o+devices`, `dmsetup status` |
| 멀티패스·SCSI | `multipath -ll`, `lsscsi`, `cat /sys/block/*/queue/scheduler` |
| 성능 특성 확인 | `hdparm -I DEV`, `hdparm -tT DEV` (읽기 전용 측정) |

## F. 파일시스템 · 마운트

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 무엇이 어디에 어떻게 붙어 있나 | `findmnt -A`, `mount`, `cat /etc/fstab`, `systemctl list-units -t mount` |
| 읽기 전용으로 넘어갔나 | `findmnt -o TARGET,OPTIONS`, `journalctl -k --grep "Remounting filesystem read-only"` |
| 파일시스템 오류 | `journalctl -k --grep "EXT4-fs error\|XFS.*error\|Btrfs.*error"`, `dumpe2fs -h DEV` |
| 검사가 필요한가 | `tune2fs -l DEV`(마운트 횟수·마지막 검사), `fsck -N DEV` |
| XFS/Btrfs/ZFS 상세 | `xfs_info MP`, `btrfs filesystem show`, `btrfs device stats`, `zfs list`, `zpool status -v` |
| 네트워크 파일시스템 | `nfsstat -m`, `showmount -e HOST`, `smbstatus` |
| 파일 잠금·점유 | `lsof MP`, `fuser -vm MP` |

## G. 네트워크 구성 · 연결

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 주소·링크 상태 | `ip -br a`, `ip -s link`, `ip a`, `nmcli device status` |
| 경로·게이트웨이 | `ip r`, `ip -6 r`, `ip route get TARGET`, `ip rule` |
| 열린 포트와 소유 프로세스 | `ss -tulpn`, `ss -s`, `ss -tanp state established` |
| 이웃·ARP | `ip neigh`, `arp -n` |
| 도달성 확인 | `ping -c4 TARGET`, `ping -c4 -M do -s SIZE TARGET`(MTU), `nc -zv HOST PORT`, `curl -sS -o /dev/null -w ... URL` |
| 경로 추적 | `mtr -rwc10 TARGET`, `traceroute TARGET`, `tracepath TARGET` |
| 이름해석 | `resolvectl status`, `resolvectl query NAME`, `dig NAME`, `dig @SERVER NAME`, `getent hosts NAME`, `/etc/resolv.conf` |
| 설정 방식 확인 | `netplan get`, `netplan status`, `systemctl status systemd-networkd NetworkManager` |
| 방화벽 규칙 | `ufw status verbose`, `nft list ruleset`, `iptables -L -n -v`, `iptables -t nat -L -n -v` |

## H. 네트워크 성능 · 오류

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 처리량 추이 | `sar -n DEV`, `ip -s link`, `iftop -t -s10`, `nload`, `bmon` |
| 오류·드롭·재전송 | `sar -n EDEV`, `sar -n TCP,ETCP`, `ss -ti`, `netstat -s`, `nstat -az` |
| 링크 협상 상태 | `ethtool IFACE`, `ethtool -S IFACE`, `ethtool -k IFACE`, `ethtool -g IFACE` |
| 커널 큐·드롭 | `cat /proc/net/softnet_stat`, `tc -s qdisc show`, `sysctl net.core.*` |
| 연결 수 한계 | `ss -s`, `sysctl net.netfilter.nf_conntrack_count`, `conntrack -S` |
| 실제 패킷 확인 | `tcpdump -nn -i IFACE -c COUNT EXPR`, `tcpdump -nn -w FILE`, `tshark -r FILE -q -z io,stat` |
| 소켓 버퍼·튜닝 값 | `sysctl net.ipv4.tcp_*`, `sysctl net.core.rmem_max` |

## I. 프로세스

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 프로세스 찾기 | `ps -eo pid,ppid,user,stat,pcpu,pmem,etime,args`, `pgrep -a PATTERN`, `pstree -paul` |
| 상태와 대기 지점 | `cat /proc/PID/status`, `cat /proc/PID/wchan`, `cat /proc/PID/stack`, `cat /proc/PID/syscall` |
| 자원 사용 | `pidstat -urd -p PID 1 5`, `cat /proc/PID/io`, `cat /proc/PID/schedstat` |
| 열린 파일·소켓 | `lsof -p PID`, `ls -l /proc/PID/fd`, `ss -tanp` |
| 한계 | `cat /proc/PID/limits`, `prlimit -p PID`, `ulimit -a`, `sysctl fs.file-nr` |
| 메모리 매핑 | `pmap -X PID`, `cat /proc/PID/smaps_rollup` |
| 호출 추적 | `strace -f -p PID -c`, `strace -f -p PID -e trace=file,network`, `ltrace -p PID` |
| 소속 확인 | `systemctl status PID`, `cat /proc/PID/cgroup` |
| 종료 이력 | `coredumpctl list`, `coredumpctl info PID`, `journalctl _PID=PID` |

## J. 서비스 · systemd · 부팅

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 실패한 유닛 | `systemctl --failed`, `systemctl list-units --state=failed`, `systemctl is-system-running` |
| 유닛 상세 | `systemctl status UNIT -l --no-pager`, `systemctl show UNIT`, `systemctl cat UNIT` |
| 의존 관계 | `systemctl list-dependencies UNIT`, `systemctl list-dependencies --reverse UNIT` |
| 자동 시작 여부 | `systemctl is-enabled UNIT`, `systemctl list-unit-files --state=enabled,masked` |
| 유닛 로그 | `journalctl -u UNIT -b`, `journalctl -u UNIT --since ...`, `journalctl -u UNIT -p err` |
| 자원 사용·한계 | `systemd-cgtop`, `systemctl show UNIT -p MemoryCurrent,CPUUsageNSec,MemoryMax,TasksMax` |
| 부팅 시간 분석 | `systemd-analyze`, `systemd-analyze blame`, `systemd-analyze critical-chain`, `systemd-analyze plot` |
| 부팅 세션 목록 | `journalctl --list-boots`, `journalctl -b -1` |
| 예약 작업 | `systemctl list-timers --all`, `crontab -l`, `ls -l /etc/cron.*`, `journalctl -u cron` |
| 설정 검증 | `systemd-analyze verify UNIT` |

## K. 로그 · 크래시

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 시점·유닛·심각도별 조회 | `journalctl --since ... --until ...`, `journalctl -u UNIT`, `journalctl -p err..alert`, `journalctl -k`, `journalctl -f` |
| 패턴 검색 | `journalctl --grep PATTERN`, `journalctl -k --grep "oom\|hung_task\|lockup\|BUG:\|panic\|I/O error\|MCE\|Xid\|segfault"` |
| 보관 상태 | `journalctl --disk-usage`, `journalctl --verify`, `/etc/systemd/journald.conf` |
| 파일 로그 | `/var/log/syslog`, `/var/log/kern.log`, `/var/log/auth.log`, `/var/log/dpkg.log`, `/var/log/unattended-upgrades/*` |
| 커널 링버퍼 | `dmesg -T`, `dmesg -l err,crit,alert,emerg`, `dmesg -w` |
| 크래시 흔적 | `ls /var/crash`, `ls /sys/fs/pstore`, `coredumpctl list`, `journalctl -k -b -1` |
| 하드웨어 오류 로그 | `ras-mc-ctl --summary`, `ras-mc-ctl --errors`, `mcelog --client`, `journalctl --grep "Hardware Error\|MCE\|EDAC"` |

## L. 하드웨어 · 센서 · GPU

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 메모리 ECC 상태 | `cat /sys/devices/system/edac/mc/mc*/ce_count`, `.../ue_count`, `.../rank*/dimm_edac_mode`, `ras-mc-ctl --status` |
| 온도·팬 | `sensors`, `cat /sys/class/hwmon/hwmon*/name`, `cat /sys/class/thermal/thermal_zone*/temp` (검증된 드라이버만 수치로 노출) |
| PCIe 장치·링크 | `lspci -nnk`, `lspci -vv -s SLOT`, `lspci -tv` |
| USB 장치 | `lsusb -t`, `lsusb -v -s BUS:DEV` |
| 전원·섀시 정보 | `dmidecode -t 32 -t 39`, `ls /dev/ipmi*`, `ipmitool sdr`, `ipmitool sel list` |
| NVIDIA GPU | `nvidia-smi`, `nvidia-smi --query-gpu=index,name,power.draw,power.limit,temperature.gpu,utilization.gpu,clocks_event_reasons.*,pcie.link.gen.current,pcie.link.gen.max,ecc.errors.uncorrected.volatile.total --format=csv`, `nvidia-smi -q` |
| AMD GPU | `rocm-smi`, `cat /sys/class/drm/card*/device/gpu_busy_percent` |
| UPS(해당 시) | `upsc`, `apcaccess status` |

## M. 커널 · 모듈 · 튜닝

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 커널·모듈 상태 | `uname -r`, `lsmod`, `modinfo MODULE`, `dkms status` |
| 커널 파라미터 | `sysctl -a`, `sysctl vm.*`, `sysctl kernel.panic`, `sysctl kernel.hung_task_timeout_secs`, `/etc/sysctl.d/*` |
| 자원 한계 전역값 | `sysctl fs.file-max`, `sysctl kernel.pid_max`, `sysctl kernel.threads-max` |
| 커널 오류 감지 설정 | `sysctl kernel.nmi_watchdog`, `sysctl kernel.softlockup_panic`, `systemctl is-enabled kdump-tools`, `kdump-config show` |
| 부팅 파라미터 | `cat /proc/cmdline`, `ls /boot`, `efibootmgr -v` |
| 라이브 커널 패치 | `canonical-livepatch status` |

## N. 보안 · 계정 · 접속

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 현재 세션 | `who -a`, `w`, `loginctl list-sessions`, `loginctl session-status` |
| 접속 이력 | `last -a -n50`, `lastb -n50`, `lastlog`, `journalctl -u ssh --since ...` |
| 인증 실패 | `journalctl _SYSTEMD_UNIT=ssh.service --grep "Failed password\|Invalid user"`, `/var/log/auth.log`, `fail2ban-client status`, `faillock --user USER` |
| 권한 상승 이력 | `journalctl --grep "sudo:"`, `/var/log/auth.log` |
| 계정 상태 | `getent passwd`, `passwd -S -a`, `chage -l USER`, `getent group sudo`, `sudo -l -U USER` |
| 원격 접속 설정 | `sshd -T`, `sshd -t`, `systemctl status ssh` |
| 외부 노출 | `ss -tulpn`(바인딩 주소 구분), `ufw status numbered`, `nft list ruleset` |
| 접근 통제 | `aa-status`, `journalctl --grep "apparmor=\"DENIED\""` |
| 파일 권한 이상 | `find / -xdev -perm -4000 -type f`, `find / -xdev -nouser` |
| 무결성·업데이트 알림 | `debsums -c`, `/var/run/reboot-required` |

## O. 패키지 · 업데이트

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 업그레이드 대상 | `apt list --upgradable`, `apt-get -s dist-upgrade`, `/usr/lib/update-notifier/apt-check --human-readable` |
| 보안 업데이트 구분 | `apt list --upgradable`의 저장소 필드 |
| 재부팅·재시작 필요 | `cat /var/run/reboot-required`, `cat /var/run/reboot-required.pkgs`, `needrestart -r l`, `checkrestart` |
| 자동 업데이트 동작 | `systemctl status unattended-upgrades apt-daily.timer`, `unattended-upgrade --dry-run -d`, `/var/log/unattended-upgrades/*` |
| 깨진 패키지 | `dpkg --audit`, `dpkg -l`, `apt-mark showhold`, `apt-get check` |
| 변경 이력 | `/var/log/dpkg.log`, `/var/log/apt/history.log` |
| 파일↔패키지 역추적 | `dpkg -S FILE`, `dpkg -L PKG`, `apt-file search FILE`, `apt policy PKG` |
| snap | `snap list --all`, `snap changes`, `snap refresh --list`, `journalctl -u snapd` |

## P. 컨테이너 · 가상화

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 컨테이너 목록·상태 | `docker ps -a`, `podman ps -a`, `lxc list`, `nerdctl ps` |
| 컨테이너 자원 사용 | `docker stats --no-stream`, `podman stats --no-stream`, `systemd-cgtop` |
| 컨테이너 로그·이벤트 | `docker logs --tail 200 NAME`, `docker events --since ...`, `docker inspect NAME` |
| 컨테이너가 먹는 공간 | `docker system df -v`, `docker image ls`, `du -sh /var/lib/docker` |
| 엔진 상태 | `systemctl status docker containerd`, `docker info` |
| 오케스트레이션(설치 시) | `kubectl get nodes,pods -A`, `kubectl describe node`, `crictl ps` |
| 가상화 여부·게스트 | `systemd-detect-virt`, `virsh list --all`, `virsh domstats` |

## Q. 회고 지표 (성능 이력)

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| 과거 특정 시각의 상태 | `sar -f /var/log/sysstat/saDD -s HH:MM:SS -e HH:MM:SS` (`-u` CPU, `-q` 부하, `-r` 메모리, `-S` 스왑, `-b`/`-d` I/O, `-n DEV` 네트워크, `-B` 페이징, `-w` 태스크) |
| 수집이 켜져 있나 | `systemctl status sysstat`, `cat /etc/default/sysstat`, `cat /etc/cron.d/sysstat`(샘플링 간격) |
| 요약 비교 | `sadf -d -- -u`(CSV 변환), `sar -A -f FILE` |

## R. 심화 프로파일링 (선택 설치)

| 하고 싶은 일 | 사용 명령 |
| --- | --- |
| CPU 시간이 어디서 쓰이나 | `perf top`, `perf record -F99 -a -g -- sleep 10` + `perf report` |
| 커널 관점 지연 추적 | `bpftrace -e ...`, `biolatency`, `biosnoop`, `execsnoop`, `opensnoop`, `tcpretrans`, `runqlat` |
| 함수·시스템 호출 지연 | `funclatency`, `strace -c`, `perf trace` |
| 스케줄링 지연 | `runqlat`, `cat /proc/PID/schedstat`, `perf sched latency` |

> 이 영역은 부하를 유발할 수 있어 기본 비활성이다. 실행 전 예상 영향과 소요 시간을 표시하고 확인을 받는다.

## 도구 가용성 처리

| 상황 | 앱의 동작 |
| --- | --- |
| 명령 미설치 | "계측 불가 — `sysstat` 미설치"로 표시하고, 이 도구가 무엇을 해주는지와 설치 명령을 함께 제시 |
| 권한 부족 | "권한 필요"로 표시하고 필요한 권한 범위를 명시. 나머지 항목은 정상 수집 |
| 하드웨어 미지원 | "이 시스템에 해당 없음"으로 표시(예: BMC 없는 보드의 IPMI) |
| 출력 형식이 예상과 다름 | 파싱 실패로 격하하되 원본 출력은 그대로 표시하고 보고서에 포함 |
| 값이 신뢰 불가 | 수치를 노출하지 않고 이유를 표기(예: 매핑이 검증되지 않은 Super-I/O 센서) |
| 대체 경로 존재 | 설치 없이 `/proc`·`/sys`로 얻을 수 있으면 그 경로로 자동 대체하고, 정밀도 차이를 표기 |

## 권장 도구 묶음

| 묶음 | 패키지 | 언제 필요한가 |
| --- | --- | --- |
| 기본 | `sysstat`, `lsof`, `iproute2`, `procps` | 대부분의 진단에 공통으로 쓰인다 |
| 저장장치 | `smartmontools`, `nvme-cli`, `hdparm` | 디스크 고장 의심, I/O 문제 |
| 네트워크 | `dnsutils`, `mtr-tiny`, `ethtool`, `tcpdump`, `net-tools` | 연결·이름해석·성능 문제 |
| 하드웨어 | `dmidecode`, `pciutils`, `usbutils`, `lshw`, `lm-sensors`, `rasdaemon` | 하드웨어 점검, 정지 원인 추적 |
| 진단 확장 | `iotop`, `strace`, `needrestart`, `sysstat` | 프로세스 심층, 업데이트 점검 |
| 심화 | `linux-tools-common`, `bpfcc-tools`, `bpftrace` | 성능 프로파일링(부하 유발 가능) |

첫 실행 시 설치 상태를 점검해 요약으로 안내한다. `syschk doctor`로 언제든 비대화형 확인이 가능하다.

---
