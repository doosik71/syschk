# 아키텍처

## 계층

syschk 는 네 계층으로 나뉜다. 각 계층은 **등록만으로 확장**되며, 확장 시 기존 화면 코드를 고치지 않는다.

| 계층 | 모듈 | 확장 단위 | 추가 방법 | 자동으로 따라오는 것 |
| --- | --- | --- | --- | --- |
| 목적 | `tasks` | `Task` | `tasks/registry.rs` 에 항목 추가 | 메뉴 표시, 증상 검색, 필요 도구 역참조, 문서 표 생성 |
| 도구 | `tools` | `Tool` | `tools/registry.rs` 에 항목 추가 | 도구 준비 화면, `doctor` 출력, 미설치 안내, 묶음 안내 |
| 수집 | `collect` | `Probe` | trait 구현 + `collect::probes()` 등록 | 가용성 처리, 근거 명령 표시, 보고서 첨부 |
| 판정 | `analyze` | 규칙 | 규칙 표에 행 추가 | 판정 배지, 근거 문구, 가설 분류 |

## 모듈 구성

```
src/
├── main.rs              진입점. 서브커맨드 없으면 TUI
├── lib.rs               크레이트 루트(시험이 참조)
├── cli.rs               clap 정의: tui / doctor / tasks / check / policy
├── app/
│   ├── state.rs         화면·커서·오버레이·검색·정렬·고정 상태와 키 입력 처리
│   ├── sampler.rs       실시간 표본 추출(이전 표본을 들고 비율을 계산)
│   ├── jobs.rs          배경 수집(스레드 + 채널). 느린 수집이 UI 를 막지 않게 한다
│   ├── storage.rs       저장 공간 화면의 자료와 배경 작업 요구 판단
│   └── runtime.rs       이벤트 루프
├── tasks/
│   ├── mod.rs           Screen, Task, TaskState
│   ├── registry.rs      14개 화면과 하위 작업 선언 (메뉴의 실체)
│   └── search.rs        증상 표현 검색
├── tools/
│   ├── mod.rs           Tool, ToolStatus, Bundle, Applicability
│   ├── registry.rs      도구 카탈로그
│   └── detect.rs        설치·해당 여부 탐지 (외부 명령 없이 PATH 확인)
├── collect/
│   ├── mod.rs           Probe, ProbeResult, Availability, ProbeCtx
│   ├── system.rs        호스트·커널·부팅 시간
│   ├── cpu.rs           /proc/stat, /proc/loadavg
│   ├── memory.rs        /proc/meminfo, /proc/vmstat
│   ├── blockio.rs       /proc/diskstats
│   ├── network.rs       /proc/net/dev
│   ├── process.rs       /proc/<pid>/*  (명령줄·소유자 캐시 포함)
│   ├── pressure.rs      /proc/pressure/*  (PSI)
│   ├── mounts.rs        /proc/self/mountinfo + statvfs (용량·inode·예약 블록)
│   ├── blockdev.rs      /sys/block, /proc/mdstat (장치·파티션·RAID)
│   ├── dirsize.rs       디렉터리 용량 측정 (du 와 같은 규칙)
│   ├── deleted.rs       삭제됐지만 열려 있는 파일 (/proc/<pid>/fd)
│   ├── logspace.rs      로그 점유량 (journalctl --disk-usage + /var/log)
│   ├── smart.rs         드라이브 자기 진단 (smartctl / nvme smart-log)
│   ├── fsinfo.rs        마운트 옵션·읽기 전용·ext4 오류 카운터
│   └── storage_errors.rs 커널 로그의 저장장치 오류
├── analyze/
│   ├── rules.rs         Verdict, Finding, 임계값 표
│   ├── bottleneck.rs    네 축 판정과 병목 선택
│   └── storage.rs       공간·inode·드라이브·파일시스템 판정과 원인 좁히기
├── commands/            비대화형 서브커맨드 (doctor / tasks / check / policy)
├── ui/
│   ├── mod.rs           레이아웃, 헤더, 푸터
│   ├── theme.rs         색상·글리프, ASCII 폴백
│   ├── widgets.rs       목록 스크롤, 배지, 키·값 줄
│   ├── overlay.rs       도움말·검색 오버레이
│   └── screens/         home, live, slow, storage, task_list, tools
└── util/
    ├── exec.rs          읽기 전용 명령 게이트 (안전 보장의 핵심)
    └── fmt.rs           표시용 서식
```

## 핵심 계약

### Task — 사용자가 보는 단위

```rust
pub struct Task {
    pub id: &'static str,               // 안정적 식별자
    pub screen: Screen,
    pub title: &'static str,            // 메뉴 문구 (사용자의 표현)
    pub answers: &'static str,          // 앱이 무엇을 답하는지
    pub aliases: &'static [&'static str], // 증상 표현 (검색 색인)
    pub tools: &'static [&'static str], // 필요한 도구 id
    pub commands: &'static [&'static str], // 근거로 노출할 읽기 전용 명령
    pub state: TaskState,               // Ready / Planned
    pub milestone: &'static str,        // 구현이 오는 마일스톤
}
```

`state` 와 `milestone` 은 정직성을 위한 필드다. 아직 구현되지 않은 작업을 메뉴에서 숨기지 않고,
"planned M3" 로 표시한다. 사용자가 무엇을 기대할 수 있는지 알 수 있어야 한다.

### Tool — 없으면 막히는 것

```rust
pub struct Tool {
    pub id: &'static str,
    pub package: &'static str,          // apt 패키지
    pub binaries: &'static [&'static str], // 설치 판단 근거
    pub purpose: &'static str,          // 비전문가용 한 줄 설명
    pub bundle: Bundle,
    pub preinstalled: bool,
    pub post_install: Option<&'static str>, // 설치 후 제약 (예: 이력은 설치 시점 이후만)
    pub without_it: Option<&'static str>,   // 설치 없이 가능한 대체 경로
    pub applicability: Applicability,       // 이 시스템에 해당하는지
}
```

`Task.tools` 하나로 **작업 → 필요 도구**와 **도구 → 못 하는 작업** 양방향 역참조가 모두 생성된다
(`tools::registry::tasks_needing`).

### Probe — 자료를 얻는 단위

```rust
pub trait Probe: Send + Sync {
    fn id(&self) -> &'static str;
    fn describe(&self) -> &'static str;
    fn commands(&self) -> Vec<&'static str> { Vec::new() }
    fn required_tools(&self) -> &'static [&'static str] { &[] }
    fn run(&self, ctx: &ProbeCtx) -> ProbeResult;
}

pub enum Availability {
    Ok,
    NotInstalled { tool: &'static str },  // → 도구 안내로 연결
    NeedsPrivilege { hint: String },
    Unsupported { reason: String },       // 이 시스템에 해당 없음
    Untrusted { reason: String },         // 값은 있으나 신뢰 불가 → 수치 미노출
    ParseFailed { reason: String },       // 원본만 표시
}
```

수집 실패는 예외가 아니라 `Availability` 의 한 상태다. 개별 수집 실패가 앱 전체로 전파되지 않는다(NFR-4).

`ProbeCtx` 는 파일시스템 루트를 갖는다. 시험은 `/proc` 대신 `tests/fixtures` 를 루트로 삼아
실제 시스템과 무관하게 파서를 검증한다.

## 읽기 전용 게이트

`util/exec.rs` 가 이 프로젝트의 안전 보장을 담당한다. 외부 명령은 `ReadOnlyCommand` 로만
만들 수 있고, 생성 시점에 다음을 검사한다.

1. 프로그램이 허용 목록에 있는가 (모르는 프로그램은 실행 불가)
2. 전역 금지 접두어를 쓰지 않는가 (`--set…`, `--delete…`, `--flush…` 등)
3. 프로그램별 금지 인자를 쓰지 않는가 (`dmesg -C`, `sysctl -w`, `smartctl -t`, `nvidia-smi -pl` 등)
4. 허용된 서브커맨드만 쓰는가 (`systemctl status` 허용 / `systemctl restart` 거부)
5. 안전 인자를 포함하는가 (`fsck -N`, `fstrim --dry-run`, `logrotate -d`, `apt-get -s`)

짧은 플래그 묶음(`-zv`, `-bon1`, `-tT`)도 개별 플래그로 인식한다.

새 수집기가 새 프로그램을 쓰려면 허용 목록에 정책을 등록해야 한다. 등록하지 않으면 실행되지 않으므로,
실수로 시스템을 바꾸는 명령이 들어갈 수 없다. `syschk policy` 로 정책과 카탈로그 검증 결과를 볼 수 있다.

## 표본 추출

CPU 사용률·처리량 같은 값은 누적 카운터의 **차이**로만 얻을 수 있다. `app/sampler.rs` 가
이전 표본을 들고 있으면서 1.5초 간격으로 비율을 계산한다. 설계상 지켜야 할 점 세 가지.

- **실시간 화면에서만 표본을 뜬다.** 다른 화면에서는 `/proc` 를 읽지 않으므로 유휴 부하가 0 이다.
- **간격이 50ms 미만이면 비율을 계산하지 않는다.** 짧은 간격은 값을 크게 왜곡한다.
- **바뀌지 않는 값은 캐시한다.** 명령줄과 소유자는 프로세스가 사는 동안 고정이므로 한 번만 읽고,
  pid 재사용은 시작 시각으로 판별한다. 권한이 없어 못 읽은 I/O 통계는 다시 시도하지 않는다.

`Probe` 는 점(point) 값과 근거 명령 노출을 담당하고, 비율 계산은 `Sampler` 가 담당한다.
이 분리 덕분에 수집기는 상태를 갖지 않고, 시험은 두 시점 픽스처로 결정적으로 검증할 수 있다.

## 배경 수집

`/proc` 읽기는 밀리초로 끝나지만, 디렉터리 용량 측정과 드라이브 자기 진단은 초 단위로 걸린다.
이런 수집은 `app/jobs.rs` 가 스레드로 돌리고 채널로 결과를 받는다.

- **비동기 런타임을 쓰지 않는다.** 이 작업들은 네트워크 대기가 아니라 블로킹 파일 작업이고,
  동시에 도는 수가 한 손에 꼽힌다. 스레드 하나가 곧 작업 하나라 추적도 쉽다.
  런타임은 실제로 필요해질 때 도입한다.
- **같은 작업은 중복 실행하지 않는다.** 작업 열쇠(경로 포함)로 판단한다.
- **진행 중임을 화면에 표시한다.** 멈춘 것처럼 보이지 않아야 한다.
- **예산을 넘기면 중단하고 그 사실을 알린다.** 조용히 잘라 놓고 다 센 척하지 않는다.
- 어떤 화면이 어떤 작업을 필요로 하는지는 `app/storage.rs` 의 `jobs_for` 가 결정한다.
  화면을 추가할 때 이벤트 루프를 고칠 필요가 없다.

## 판정 규칙

규칙은 코드에 흩뿌리지 않고 표로 관리한다(`analyze/rules.rs`). 판정에는 반드시 근거 문구와
**그 지표가 무엇인지에 대한 한 줄 설명**이 붙는다. 설명은 사용자가 쓰면서 지표를 익히게 하는 통로다.

병목 축은 압박 지표(PSI)를 1순위 근거로 고른다. 사용률은 축 사이 비교가 어렵지만, PSI 는
"일이 실제로 지연된 시간의 비율"이므로 CPU·메모리·I/O 를 같은 잣대로 비교할 수 있다.
커널이 PSI 를 제공하지 않으면 사용률·대기시간으로 대체하고, 그 사실을 근거 문구에 남긴다.

| 지표 | 조건 | 판정 | 근거 문구 |
| --- | --- | --- | --- |
| 스왑 사용량 | 0 | Ok | "no swap thrashing (0 B)" |
| `%iowait` | < 1% | Ok | "disk is not the bottleneck (0.05%)" |
| blocked 프로세스 | 0 | Ok | "nothing is waiting on I/O" |
| load / 논리코어 | < 0.8 | Ok | "cpu is not saturated (15.45 / 32)" |
| PSI some avg10 | > 20% | Warn | "memory reclaim pressure is building" |
| 마운트 사용률 | > 90% (95% 이상은 Critical) | Warn | "/var is getting full" + 사용량·여유량 |
| 마운트 여유량 | < 1GiB **이면서** 사용률 75% 이상 | Warn | 작은 파일시스템 오탐을 막기 위한 조건부 규칙 |
| inode 사용률 | > 90% | Critical | "out of inodes - new files will fail even with space left" |
| 삭제 후 점유 | > 1GiB 또는 사용량의 5% | Warn | "N GB is held by files that were already deleted" |
| 로그 점유 | 사용량의 25% 또는 5GiB | Warn | "Logs are using N GB" |
| 읽기 전용 전환 | 존재 | Critical | "mounted read-only" (커널이 오류를 만난 결과일 수 있다) |
| ext4 오류 카운터 | > 0 | Warn | "N error(s) counted by ext4 itself" |
| RAID 구성원 상태 | `_` 포함 | Critical | "degraded - a member is missing" |
| SMART 재할당·대기 섹터 | > 0 | Warn | "sectors have been reallocated - consider replacement" |
| 실패 유닛 수 | > 0 | Warn | "1 failed unit: foo.service (exit 1)" |
| 커널 장애 패턴 0건 + 비정상 종료 | 동시 성립 | Critical | "stopped before the kernel could log - look at hardware" |
| GPU 전력 / 상한 | > 0.9 | Warn | "power limit saturated (274.87W / 300W)" |
| 동일 모델 GPU 링크 세대 불일치 | 존재 | Warn | "only GPU 0 negotiated Gen3 - suspect slot or riser" |

## 확장 절차

새 진단 기능을 추가하는 순서는 [development.md](development.md#새-기능-추가하기) 를 참조한다.

## 크레이트

| 용도 | 크레이트 | 도입 시점 |
| --- | --- | --- |
| TUI 렌더링 | `ratatui` | M0 |
| 터미널 백엔드 | `ratatui::crossterm` 재수출 | M0 (버전 불일치 방지) |
| CLI | `clap` (derive) | M0 |
| 오류 처리 | `anyhow` | M0 |
| 직렬화 | `serde`, `serde_json` | M7 (보고서) |
| 파일시스템 용량 | `libc` | M2. 여유 용량과 inode 수는 `statvfs` 로만 얻을 수 있다 |
| 비동기 | `tokio` | 아직 없음. M2 의 느린 수집은 표준 스레드와 채널로 처리했다(아래 참조) |

의존성은 필요한 마일스톤에서 추가한다. M0 은 세 개뿐이며, 그만큼 빌드가 빠르고 공격 표면이 작다.
