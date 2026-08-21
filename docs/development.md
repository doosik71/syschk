# 개발 안내

## 환경

- Rust 1.88 이상 (edition 2024)
- Ubuntu 24.04 LTS 기준 개발, 22.04 호환 유지
- 외부 진단 도구는 전부 선택 사항이다. 없으면 해당 항목만 비활성화되고 앱은 정상 동작한다

## 빌드와 실행

```bash
cargo build --release            # target/release/syschk (단일 실행 파일)
cargo run                        # TUI
cargo run -- doctor              # 진단 도구 설치 상태와 설치 안내
cargo run -- doctor --missing    # 빠진 것만
cargo run -- tasks               # 지원 작업 목록
cargo run -- tasks "disk full"   # 증상으로 검색
cargo run -- tasks --markdown    # docs/screens.md 의 표 생성
cargo run -- check               # 한 번 점검 후 요약 (종료 코드로 상태 반환)
cargo run -- policy              # 읽기 전용 정책과 카탈로그 검증 결과
```

배포 시에는 `target/release/syschk` 파일 하나만 복사하면 된다. 별도 설치 과정이 없다.

## 검증

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -- policy              # 읽기 전용 보장 확인
```

CI(`.github/workflows/ci.yml`)가 같은 순서로 검사하고, 릴리스 빌드까지 확인한다.

## 시험 구성

| 파일 | 무엇을 지키는가 |
| --- | --- |
| `tests/policy.rs` | 시스템을 변경하는 명령이 카탈로그에 들어가지 못하게 막는다 |
| `tests/registry.rs` | 작업·도구 레지스트리 정합성, UI 문구가 영어인지 |
| `tests/search.rs` | 증상 표현으로 올바른 작업이 나오는지 |
| `tests/probes.rs` | 픽스처로 파서 검증, 읽기 실패 시 정상 격하 |
| `tests/ui.rs` | 화면 렌더링, 키 조작, 좁은 터미널, 정렬·고정·정지 |
| `tests/live_metrics.rs` | 두 시점 픽스처로 사용률·처리량·대기시간 계산 검증 |
| `tests/bottleneck.rs` | 지표 조합 → 기대 판정 (병목 축 선택 규칙) |
| `tests/storage.rs` | 마운트·장치·RAID 파싱, 디렉터리 측정, SMART 파싱, 공간 부족 원인 판정 |

`tests/fixtures/` 는 `/proc`, `/etc` 를 흉내낸 트리다. `ProbeCtx::with_root` 로 루트를 바꿔
실제 시스템과 무관하게 파서를 시험한다. 비율 지표는 두 시점이 필요하므로 `t0/`, `t1/`
두 트리를 두고 간격을 인자로 넘긴다 — 시험이 타이밍에 흔들리지 않는다.

기본 실행에서 빠지는 시험 두 개가 있다. 실제 시스템을 재는 것이라 결과가 환경에 따라 달라진다.

```bash
cargo test --test live_metrics -- --ignored --nocapture   # 표본 하나의 비용 측정
cargo test --test ui -- --ignored --nocapture screenshot  # 화면을 텍스트로 덤프
```

## 새 기능 추가하기

계층별로 등록만 하면 된다. 기존 화면 코드를 고치지 않는다.

### 1. 새 작업(메뉴 항목) 추가

`src/tasks/registry.rs` 에 항목 하나를 넣는다.

```rust
t!("storage.trim", Storage, "M2", Planned,
    "Is unused space being released to the SSD",
    "Whether periodic trim runs, and when it last did",
    aka ["trim", "ssd", "discard"],
    needs ["util-linux"],
    uses ["systemctl status fstrim.timer", "fstrim -av --dry-run"]),
```

메뉴 표시, 증상 검색, 필요 도구 역참조, `docs/screens.md` 표가 자동으로 따라온다.

`uses` 는 **앱이 실제로 읽는 것**을, `learn` 은 **사용자가 직접 쳐볼 수 있는 동등한 명령**을 적는다.
syschk 는 `/proc` 를 직접 읽는 경우가 많아 둘이 다르며, `learn` 이 사용자가 시스템 관리 명령을
익히는 통로가 된다. 양쪽 모두 `cargo test` 가 읽기 전용 정책으로 검증한다.

```rust
t!("live.cpu", Live, "M1", Ready,
    "Who is using the CPU right now", "...",
    aka ["cpu", "busy"], needs [],
    uses ["cat /proc/stat", "cat /proc/pressure/cpu"],
    learn ["mpstat -P ALL 1 3", "pidstat -u 1 3"]),
```

### 2. 새 도구 추가

`src/tools/registry.rs` 에 항목 하나를 넣는다.

```rust
tool!("hdparm", "hdparm", ["hdparm"], Storage, false,
    "Drive identity and read-speed measurement",
    without: "Kernel logs still show the negotiated link speed."),
```

`purpose` 는 **비전문가가 읽는 문장**이다. 도구 이름을 반복하지 말고 무엇을 해주는지 쓴다.
설치 후 제약이 있으면 `post:` 에 적는다(예: 성능 이력은 설치 시점 이후만 조회 가능).

### 3. 새 수집기 추가

`Probe` 를 구현하고 `collect::probes()` 에 등록한다.

- 외부 명령을 쓰면 `util/exec.rs` 허용 목록에 프로그램 정책을 등록해야 한다
- 실패는 `Availability` 로 표현한다. `panic!` 이나 오류 전파로 앱을 죽이지 않는다
- 값이 있으나 신뢰할 수 없으면 `Untrusted` 로 두고 **수치를 노출하지 않는다**
- 픽스처를 `tests/fixtures/` 에 추가하고 파서 시험을 붙인다

### 4. 새 프로그램을 허용 목록에 넣기

`src/util/exec.rs` 의 `ALLOWLIST` 에 정책과 함께 등록한다.

```rust
("newtool", RO.forbid(&["--apply"]).subs(&["status", "show"])),
```

조회와 변경이 섞인 도구라면 반드시 서브커맨드를 좁힌다. 확신이 없으면 `require(&["--dry-run"])`
같은 안전 인자를 강제한다. `tests/policy.rs` 의 거부 목록에 위험한 사용례를 추가한다.

## 코드 규칙

- 화면에 표시되는 문구는 영어. 문서와 코드 주석은 한국어(→ [screens.md](screens.md#표시-언어))
- 사용자에게 보이는 문장은 비전문가 기준으로 쓴다. 전문 용어를 쓰면 한 줄 설명을 붙인다
- 판정에는 항상 수치 근거와, 그 지표가 무엇인지에 대한 한 줄 설명을 함께 담는다
- 읽지 못한 값은 0 으로 꾸미지 않는다. "권한 없음"과 "0"은 다른 사실이다
- 시스템을 변경하는 코드는 추가하지 않는다(→ [scope.md](scope.md))
- `AGENTS.md` 의 커밋 메시지 규칙을 따른다
