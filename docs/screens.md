# 화면과 메뉴

syschk 의 화면은 도구별 분류(로그 / 디스크 / 네트워크…)가 아니라 **사용자가 하려는 일**로 나뉜다.
메뉴 항목도 도구 이름이 아니라 사용자가 실제로 쓰는 표현으로 적는다.

- 배경과 근거: [motivation.md](motivation.md)
- 각 항목이 내부에서 사용하는 명령: [commands.md](commands.md)
- 구현 순서: [roadmap.md](roadmap.md)

## 홈 화면

```
┌─ syschk ─────────────────────────────────── ubuntu2743 · 24.04 LTS · up 3d 4h ─┐
│ up 3d 4h  load 0.52 0.61 0.58 / 32 cores   tools ✔ 41  ✖ 14 missing  – 3 n/a   │
│ read-only: syschk never changes this system                                     │
├────────────────────────────────────────────────────────────────────────────────┤
│  What do you want to do?                                                       │
│                                                                                │
│   1  See what the system is doing right now      live view                     │
│   2  Something is slow - find out why            find the bottleneck           │
│   3  It froze or rebooted unexpectedly           after the fact                │
│   4  Disk is full, or a drive looks bad          space and drive health     ⚠   │
│   5  Network is down or slow                     reachability and speed        │
│   6  A service failed, or boot is slow           units and boot                │
│   7  Look into one program                       one process in depth          │
│   8  Check whether the hardware is healthy       cpu, ram, disk, gpu, sensors ⚠ │
│   9  See who logged in and what is exposed       logins and exposure           │
│  10  Check updates and package state             packages and reboots      ⚠   │
│  11  Search the logs                             time, unit, pattern           │
│  12  Be ready to catch the cause next time       instrumentation           ⚠   │
│  13  Save what I found as a document             markdown and json             │
│  14  Get the tools diagnosis needs               install guidance              │
│                                                                                │
├────────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  1-14 jump  ⏎ open  / search  t tools  ? help  q quit                  │
└────────────────────────────────────────────────────────────────────────────────┘
```

- `⚠` 는 그 영역에 이미 주의 항목이 있다는 표시다. 앱을 열면 **어디를 봐야 하는지가 먼저 보인다.**
- `/` 검색은 작업 제목과 증상 표현을 함께 색인한다. "slow", "disk full", "frozen", "port" 같은 말로 바로 도달한다.
- 각 작업 화면은 상단에 **판정 요약**, 중단에 **근거 자료**, 하단에 **다음에 해볼 일**을 배치한다. 이 3단 구성은 모든 화면에서 동일하다.
- 근거 명령은 기본으로 접혀 있고 `c` 로 펼친다. 명령은 기억 대상이 아니라 근거다.


## 실시간 관찰 화면 (1번)

위쪽은 네 축의 현재 상태, 아래쪽은 선택한 항목에 따라 달라지는 상세다.

```
┌ cpu ───────────────────────────────┐┌ memory ───────────────────────────┐┌ disk ──────────────────────────────┐┌ network ──────────────────────────┐
│█████░░░░░░░░░░░░░░░░░░░ 19%        ││█░░░░░░░░░░░░░░░░░░░░░░░ 4.6%      ││░░░░░░░░░░░░░░░░░░░░░░░░ 0.0%       ││enp37s0f1  1.3M/s                  │
│user 16% sys 3.4% io 0.0%           ││23.2G used of 504G                 ││busiest nvme1n1  wait 0.0ms         ││in 17.5K/s                         │
│load 7.25 / 32 cores = 0.23         ││cache 44.2G  swap 0B               ││idle  0 waiting                     ││out 1.3M/s  err 0 drop 122291      │
│▂▃▂▁▂                               ││▁▁▁▁▁                              ││▁▁▁▁▁                               ││█▂▁▃▂                              │
└────────────────────────────────────┘└───────────────────────────────────┘└────────────────────────────────────┘└───────────────────────────────────┘
┌ 1. Right now ──────────────────────┐┌ Processes by CPU ────────────────────────────────────────────────────────────────────────────────────────────┐
│   Show me everything at a glance   ││sorted by cpu (s to change, J/K to move, p to pin, f to freeze)                                               │
│ › Who is using the CPU right now   ││      PID USER        CPU%   MEM%      RSS    READ/s   WRITE/s  COMMAND                                       │
│   Who is using memory right now    ││> 149335 doosik     198.4    0.2     1.2G         0         0  /…/python train-model.py --device cuda:1        │
│   Who is reading and writing the … ││    5827 xrdp       102.6    0.0    51.3M         -         -  /usr/sbin/xrdp                                 │
│   Who is using the network right … ││       1 root         0.0    0.0    13.2M         -         -  /sbin/init splash                              │
│   Is any program stuck and not re… ││                                                                                                              │
│   Freeze the screen and look clos… ││Per-process I/O for other users needs privileges - shown as '-' rather than 0.                                │
└────────────────────────────────────┘└──────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 ↑↓ view   J/K process   s sort   f freeze   p pin   c commands   esc back
```

`-` 와 `0` 은 다른 뜻이다. `0` 은 "읽었고 0 이었다", `-` 는 "권한이 없어 읽지 못했다"다.
값을 0 으로 꾸미지 않는다(원칙: 모르는 것은 모른다고 말한다).

## 병목 특정 화면 (2번)

판정 → 근거 → 지표 설명 → 근거 명령의 순서로 배치한다. 마지막 두 절이 이 앱을 쓰면서
지표와 명령을 익히게 하는 부분이다.

```
┌ 2. Why is it slow ─────────────────────────────────────────────────────────────────────────────────────┐
│✔ ok        Nothing is saturated right now                                                              │
│ cpu ✔ ok         memory ✔ ok         disk ✔ ok         network ✔ ok                                    │
└────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌ Look at ───────────────────────────┐┌ Detail ──────────────────────────────────────────────────────────┐
│   Just tell me what is making i…   ││✔ ok        CPU is not the bottleneck                             │
│ › Am I running out of CPU          ││                                                                  │
│   Am I running out of memory       ││Evidence                                                          │
│   Is the disk too slow             ││  busy 23% (user 18%, system 4.7%), idle 77%                      │
│   Is one program to blame          ││  load 6.40 across 32 cores = 0.20 per core (warn above 0.8)      │
│   Was it always like this - com… M3││  pressure: work was delayed 0.0% of the last 10s                 │
│   Is a container eating the res… M6││                                                                  │
│                                    ││What these numbers mean                                           │
│                                    ││  Load counts how many processes want to run. Divided by the core │
│                                    ││  count it says whether the queue is longer than the machine can  │
│                                    ││  serve; 'busy' says how much of the CPU was actually used.       │
│                                    ││                                                                  │
│                                    ││What syschk reads                                                 │
│                                    ││  cat /proc/loadavg                                               │
│                                    ││  cat /proc/stat                                                  │
│                                    ││  cat /proc/pressure/cpu                                          │
│                                    ││Try it yourself                                                   │
│                                    ││  mpstat -P ALL 1 3                                               │
│                                    ││  uptime                                                          │
└────────────────────────────────────┘└──────────────────────────────────────────────────────────────────┘
```

## 표시 언어

화면에 표시되는 문구는 **영어**로 통일한다. 프로젝트 초기 단계에서 다국어 지원 비용을 감당하기에는 이르다는 판단이다.
문서와 코드 주석은 한국어를 쓴다. 이 규칙은 `tests/registry.rs` 의 `user_facing_text_is_ascii` 가 지킨다.

## 메뉴 — 구체적으로 어떤 작업을 하고 싶은가

아래 표는 코드의 작업 레지스트리(`src/tasks/registry.rs`)에서 생성한 것이다.
문서와 구현이 어긋나지 않도록 손으로 고치지 않는다.

```bash
cargo run -- tasks --markdown   # 이 절 이하를 다시 생성한다
```

`Status` 열의 `ready` 는 지금 동작하는 작업이고, `M3` 같은 값은 그 마일스톤에서 온다는 뜻이다.
앱도 같은 표시를 보여주므로, 사용자는 무엇을 기대할 수 있는지 알 수 있다.

<!-- generated by `syschk tasks --markdown` - do not edit by hand -->

### 1. See what the system is doing right now

Watch current activity and see which processes are behind it.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Show me everything at a glance | CPU, memory, disk, network and load side by side, each with a plain verdict | - | ready |
| Who is using the CPU right now | Top processes by CPU, per-core spread, and the user/system/iowait split | - | ready |
| Who is using memory right now | Top processes by resident memory, cache versus real usage, and whether swap is in play | - | ready |
| Who is reading and writing the disk right now | Per-process I/O and per-device queue depth and wait time | - | ready |
| Who is using the network right now | Throughput per interface, with error and drop counters | - | ready |
| Is any program stuck and not responding | Processes in uninterruptible wait, and the kernel point they are waiting at | - | ready |
| Freeze the screen and look closer | Pause updates, re-sort, and pin one process to keep watching | - | ready |

### 2. Something is slow - find out why

Narrow the slowdown to one axis: cpu, memory, disk or network.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Just tell me what is making it slow | One of cpu / memory / disk / network named as the bottleneck, with the numbers behind it | - | ready |
| Am I running out of CPU | Load against core count, run queue, and steal time on virtual machines | - | ready |
| Am I running out of memory | Available versus cached memory, swap activity and reclaim pressure | - | ready |
| Is the disk too slow | Per-device busy time and average wait, iowait share, and who is waiting | - | ready |
| Is one program to blame | The heaviest consumers of cpu, memory and disk side by side | - | ready |
| Was it always like this - compare with the past | The same figures over recent days, so 'slow' can be measured against normal | `sysstat` | M3 |
| Is a container eating the resources | CPU, memory and I/O per container against its configured limit | `docker.io` | M6 |

### 3. It froze or rebooted unexpectedly

Pin down when it stopped, then rule causes in or out with what was recorded.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| When did it stop | Boot sessions listed, with the last healthy record and the restart time bracketing the stop | `systemd`, `sysstat` | M3 |
| Was it a clean shutdown or a hard stop | Whether the previous session ended with normal shutdown records or was cut off mid-line | `systemd` | M3 |
| Did the kernel leave anything behind | Kernel-only messages scanned for the known failure patterns, noise filtered out | `systemd`, `util-linux` | M3 |
| Did it run out of resources | Recorded CPU, load, memory, swap and I/O right before the stop, each ruled in or out | `sysstat` | M3 |
| Was it a hardware problem | Drive health, memory error counters, temperatures and link state at a glance | `smartmontools`, `rasdaemon`, `lm-sensors` | M6 |
| Is there a kernel crash record | Crash dumps, firmware-stored panic logs and core dumps, if any exist | `systemd`, `kdump-tools` | M3 |
| Was my program killed | OOM kill targets, units that exited with a failure code, and core dump history | `systemd` | M3 |
| Summarise the conclusion | Causes sorted into ruled out / weakened / likely / undetermined, each with its evidence | - | M3 |
| What should I check next | For each remaining cause, the least invasive way to confirm or drop it | - | M3 |

### 4. Disk is full, or a drive looks bad

Separate 'space is used up' from 'the drive is failing'.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Which filesystem is full | Usage and free space per mount point, with the ones over threshold called out | `util-linux` | M2 |
| Find what is filling up the disk | Largest directories, with drill-down into subdirectories | - | M2 |
| I deleted files but the space did not come back | Files that are unlinked but still open, and the processes holding them | `lsof` | M2 |
| There is free space but I get 'No space left' | Inode exhaustion, reserved blocks and quota limits - the three usual explanations | `util-linux`, `e2fsprogs` | M2 |
| Are logs eating the space | Journal footprint, largest log directories and whether rotation is configured | `systemd` | M2 |
| Is the drive failing | SMART health plus the attributes that actually predict failure, and NVMe wear and temperature | `smartmontools`, `nvme-cli` | M2 |
| Are there disk error records | Storage-related kernel errors: link resets, I/O errors and filesystem errors | `systemd` | M2 |
| Show me how the disks are laid out | Devices, partitions, filesystems and mounts as a tree, plus RAID and LVM state | `util-linux`, `lvm2`, `mdadm` | M2 |
| Is the filesystem healthy | Mount options, read-only fallback, check-needed flags and superblock state | `e2fsprogs`, `util-linux` | M2 |

### 5. Network is down or slow

Walk the path outward and find the first step that breaks.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Tell me where the connection breaks | Link, address, gateway, internet, DNS and HTTPS checked in order, stopping at the first failure | `iproute2`, `iputils-ping`, `dnsutils`, `curl` | M5 |
| What is my IP, gateway and DNS | Interface addresses, the routing table, and the name servers actually in use | `iproute2` | M5 |
| Which ports are open and who is listening | Listening sockets with owning process, separating localhost-only from externally reachable | `iproute2` | M5 |
| Can I reach a specific host or port | Reachability for a host and port you choose, naming the step that fails | `iputils-ping`, `netcat-openbsd`, `dnsutils` | M5 |
| Is name resolution the problem | Query results and response time per configured name server | `dnsutils` | M5 |
| Where does the delay or packet loss happen | Per-hop latency and loss along the path | `mtr-tiny`, `traceroute` | M5 |
| Why is the network slow | Throughput, retransmits, errors and drops, negotiated link speed and MTU | `ethtool`, `sysstat`, `iproute2` | M5 |
| Is the firewall blocking it | Whether a firewall is active, a summary of its rules, and any related block records | - | M5 |
| Are there too many connections | Connection counts by state against socket and connection-tracking limits | `iproute2` | M5 |
| Let me look at the actual packets | A short, bounded capture with a summary - after confirming the added load | `tcpdump` | M5 |

### 6. A service failed, or boot is slow

See which units failed and what the boot time went into.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Which services failed | Failed units with exit codes and their last log lines | `systemd` | M4 |
| Why does this service not start | State, dependencies, recent logs, effective configuration and restart count in one place | `systemd` | M4 |
| Is this service set to start on boot | Enabled, disabled or masked, plus the configuration files actually in effect | `systemd` | M4 |
| Why is boot slow | Total boot time broken down, the slowest units, and the critical path | `systemd` | M4 |
| Are scheduled jobs running | Timers with last and next run, cron jobs, and their execution logs | `systemd` | M4 |
| How much does this service use | CPU and memory per unit against the limits set for it | `systemd` | M4 |
| Is it restarting over and over | Restart counters, the interval between restarts, and the surrounding log window | `systemd` | M4 |

### 7. Look into one program

Everything about one process: usage, files, limits, why it waits.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| What is this program doing | State, command line, parent and children, and the unit or container it belongs to | `procps`, `psmisc` | M4 |
| How much does it use | CPU and memory over time, bytes read and written, thread count | `sysstat` | M4 |
| Which files does it have open | Open files and sockets, including deleted files it still holds | `lsof` | M4 |
| Which limits is it hitting | File descriptor, memory and task limits against current usage | - | M4 |
| Why is it stuck | Wait state and kernel wait point, with the likely blocking cause | `procps`, `strace` | M4 |
| Is it leaking memory | Usage growth over time and how the memory is mapped | `procps`, `smem` | M4 |
| What calls is it making | A short syscall sample with the slowest calls summarised | `strace` | M4 |

### 8. Check whether the hardware is healthy

Inventory and health, with untrustworthy sensor values called out as such.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| What is this machine made of | CPU, memory layout, board, firmware and populated slots | `dmidecode`, `lshw` | M6 |
| Are there memory errors | ECC mode in use, corrected and uncorrected error counts, and hardware error records | `rasdaemon` | M6 |
| Are the temperatures fine | Temperatures from drivers that can be trusted. Unverified sensors are shown as unavailable, not guessed | `lm-sensors` | M6 |
| Is the CPU running at full speed | Current and maximum frequency, throttle reasons and the active power policy | `linux-tools-common` | M6 |
| How are the GPUs doing | Power against limit, temperature, throttle reasons, memory errors and link speed, with mismatches between identical cards flagged | `nvidia-utils-580` | M6 |
| Are the expansion cards linked properly | PCIe and USB device tree with current versus maximum link speed and width | `pciutils`, `usbutils` | M6 |
| Can I see the power draw | Whether any power telemetry exists at all - and if not, that fact plus the alternatives | `dmidecode`, `ipmitool` | M6 |
| Is this a virtual machine | Whether this runs on bare metal or a hypervisor, and which one | `systemd` | M6 |

### 9. See who logged in and what is exposed

Who is connected, who tried, and what the outside world can reach.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Who is logged in right now | Active login sessions, where they came from, and what they are running | `systemd` | M6 |
| Who logged in recently | Login history with failures, and the source addresses that appear most | - | M6 |
| Are there floods of failed logins | Authentication failure trend, the accounts targeted, and whether blocking is active | `fail2ban` | M6 |
| What is exposed to the outside | Externally bound listening ports with their owning process, checked against firewall policy | `iproute2` | M6 |
| Are the account permissions sane | Administrator accounts, passwordless accounts, expiry policy and remote access settings | - | M6 |
| Is access control enabled | AppArmor profile state and recent denials | - | M6 |
| Show me administrator command history | Privilege escalation records in time order | `systemd` | M6 |

### 10. Check updates and package state

What is pending, what needs a reboot, and what changed recently.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Is there anything to update | How many packages can be upgraded, and which of those are security updates | - | M6 |
| Do I need to reboot | Whether a reboot is pending, which packages want it, and which services still need restarting | `needrestart` | M6 |
| Are automatic updates working | Recent unattended upgrade runs and whether any failed | `systemd` | M6 |
| Are any packages broken | Half-configured packages, dependency problems and packages held back | - | M6 |
| What changed recently | Install, upgrade and removal history in time order - to line up against when trouble started | - | M6 |
| Which package owns this file | File to package, and package to file listing | `apt-file` | M6 |
| How are snaps doing | Installed snaps, recent changes and pending refreshes | `snapd` | M6 |

### 11. Search the logs

Find the entries that matter and fold away the repeating noise.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Show me what happened around a certain time | Log entries for a time window you pick, navigable by boot session | `systemd` | M3 |
| Show me only this service's log | Entries filtered to one unit, with priority filter and live follow | `systemd` | M3 |
| Show me only errors | Severity-filtered entries, with kernel messages viewable on their own | `systemd` | M3 |
| Hide the repeating noise | High-frequency repeated messages collapsed into counts, with known noise sources toggleable | `systemd` | M3 |
| Search for a specific string | Regex search plus ready-made patterns for common failures | `systemd` | M3 |
| How far back do the logs go | Retention window, disk footprint and integrity of the journal | `systemd` | M3 |
| Include the plain log files too | Traditional log files read alongside the journal, with timestamp format differences absorbed | - | M3 |
| Keep the part I found | The selected range attached to a report or saved to a file | - | M7 |

### 12. Be ready to catch the cause next time

Check whether this machine can even record the cause of a hard stop.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Am I ready to capture the cause | A checklist: crash dump, panic policy, hang detection, hardware error daemon, performance history, core dumps | `kdump-tools`, `rasdaemon`, `sysstat` | M7 |
| How do I set up what is missing | For each gap, the exact command or configuration file content - shown, never run | - | M7 |
| The performance history is too coarse | The current sampling interval, how much it can miss, and how to change it | `sysstat` | M7 |
| How do I keep logs after a kernel death | What it takes to keep evidence when the kernel dies mid-write, and what this machine can support | - | M7 |
| Start recording metrics from now on | Chosen metrics written to a file at a fixed interval, for when the problem is intermittent | - | M7 |

### 13. Save what I found as a document

Keep the evidence: findings, verdicts and the exact commands used.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| Save the current screen | This screen's data, verdicts and the commands behind them, as Markdown | - | M7 |
| Save the whole diagnosis session | Everything done in this session in time order: timeline, evidence and how each cause was classified | - | M7 |
| Make it safe to share | The same report with hostnames, addresses and account names masked | - | M7 |
| Give me machine-readable output | The same findings as JSON, for other tools to consume | - | M7 |
| Run checks on a schedule | How to run syschk without the interface, and what its exit codes mean | - | M7 |

### 14. Get the tools diagnosis needs

See what is missing, what it would give you, and how to install it.

| Menu item | What you get | Needs | Status |
| --- | --- | --- | --- |
| What is installed and what is missing | Every tool syschk can use, with its state: installed, missing, or not applicable here | - | ready |
| What does this tool do | A one-line purpose, plus the list of tasks that stop working without it | - | ready |
| What do I need for what I am about to do | Only the tools missing for the task you picked, nothing else | - | ready |
| How do I install it | The package name and the exact install command, shown for you to run. syschk does not install anything itself | - | ready |
| Install a recommended bundle | Grouped install commands: core, storage, network, hardware, diagnostics, advanced | - | ready |
| Installation fails - what now | The usual causes (no privileges, no network, stale package index) and what to do about each | - | M8 |
| What can I do without installing anything | Which checks work from /proc and /sys alone, and where precision is lost | - | ready |
