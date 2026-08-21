# syschk

우분투 시스템의 **상태 점검**과 **장애 원인 분석**을 도와주는 터미널 UI(TUI) 도구.

화면은 명령이 아니라 **목적**으로 구성되고, 메뉴는 도구 이름이 아니라 **"지금 무엇을 하고 싶은가"**로
구성된다. 사용자는 어떤 명령이 어떤 옵션을 갖는지 기억할 필요가 없다.
진단에 필요한 도구가 없으면, 그 도구가 무엇이고 왜 필요한지 설명한 뒤 설치 방법까지 안내한다.

**syschk 는 읽기만 한다.** 설정을 바꾸거나 서비스를 재시작하거나 패키지를 설치하지 않는다.
목적은 "정밀하지만 비파괴적인 진단"까지이며, 이 경계는 코드가 강제한다.

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
│   8  Check whether the hardware is healthy       cpu, ram, disk, gpu, sensors  │
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

## 무엇을 해결하는가

우분투를 쓰는 사람은 더 이상 시스템 관리자만이 아니다. 그러나 장애는 종종 발생하고,
그때 필요한 것은 `journalctl --list-boots`, `sar -f … -qrSb`, `lsof +L1`, `ss -tulpn` 같은
명령과 그 출력을 해석하는 지식이다. 비전문가에게 이 간극은 사실상 막힌 길이다.

syschk 는 그 번역과 해석을 대신한다. 사용자는 증상을 고르고, 앱이 자료를 모아 **판정과 근거**를 제시한다.
그리고 쓰다 보면 각 수치가 무엇을 뜻하는지 자연스럽게 알게 된다 — 근거 명령이 항상 함께 표시되기 때문이다.

→ 자세한 배경, TUI 와 Rust 를 고른 이유: [docs/motivation.md](docs/motivation.md)

## 빠른 시작

```bash
cargo build --release
./target/release/syschk            # TUI
./target/release/syschk doctor     # 진단 도구가 갖춰졌는지 점검
./target/release/syschk check      # 지금 병목이 있는지 한 번 판정 (종료 코드로 알림)
./target/release/syschk policy     # 이 앱이 실행할 수 있는 명령 전체 확인
```

단일 실행 파일이므로 복사해서 실행 권한만 주면 동작한다. 런타임·인터프리터·의존 라이브러리 설치가 필요 없다.

## 문서

| 문서 | 내용 |
| --- | --- |
| [docs/motivation.md](docs/motivation.md) | 왜 만드는가, 대상 사용자, 왜 TUI 인가, 왜 Rust 인가 |
| [docs/scope.md](docs/scope.md) | 하는 일과 하지 않는 일, 비파괴 원칙과 그 보장 방식, 설계 원칙 10 |
| [docs/screens.md](docs/screens.md) | 14개 화면과 90여 개 작업 메뉴 (코드에서 생성) |
| [docs/commands.md](docs/commands.md) | 앱이 내부에서 사용하는 명령 카탈로그 (A~R, 18개 영역) |
| [docs/requirements.md](docs/requirements.md) | 사용자 요구사항 FR-1~26, 비기능 요구사항 NFR-1~10 |
| [docs/architecture.md](docs/architecture.md) | 계층 구조, 핵심 계약, 읽기 전용 게이트, 판정 규칙 |
| [docs/roadmap.md](docs/roadmap.md) | 마일스톤 M0~M8, 검증 전략, 위험 |
| [docs/development.md](docs/development.md) | 빌드·검증 방법, 새 기능 추가 절차, 코드 규칙 |
| [AGENTS.md](AGENTS.md) | 기여 규칙 |

## 원칙 요약

1. 사용자는 명령을 기억하지 않는다 — 화면은 목적으로, 메뉴는 작업으로
2. 명령은 기억 대상이 아니라 근거다 — 기본은 접힘, `c` 로 펼침, 보고서에는 항상 포함
3. 판정에는 수치 근거를 붙인다
4. 모르는 것은 모른다고 말한다 — 신뢰할 수 없는 값은 수치로 노출하지 않는다
5. 없는 것도 정보다 — 오류 로그 0건도 결론을 좁힌다
6. 막히면 길을 알려준다 — 도구가 없어서 진단이 멈추지 않게 한다
7. 읽기 전용이 기본이며 예외가 없다 — 조치는 제안으로만
8. 최소 권한 — 앱이 `sudo` 를 호출하지 않는다
9. 결론은 단정이 아니라 좁혀진 가설 목록
10. 확장은 등록으로 끝난다

→ 각 원칙의 근거: [docs/scope.md](docs/scope.md#설계-원칙)

## 구현 현황

| 마일스톤 | 상태 | 내용 |
| --- | --- | --- |
| M0 골격 | ✅ 완료 | 계층 계약, 작업 90여 개·도구 55개 레지스트리, 홈 메뉴 14화면, 증상 검색, 도구 준비 화면, `doctor`/`tasks`/`check`/`policy` |
| M1 실시간 관찰 + 병목 특정 | ✅ 완료 | 화면 1·2 — CPU·메모리·디스크·네트워크·프로세스 실시간 관찰, 병목 축 판정과 근거, 정렬·고정·정지 |
| M2 저장 공간·저장장치 | ✅ 완료 | 화면 4 — 파일시스템 사용량과 inode, 디렉터리 용량 드릴다운, 삭제 후 점유, 로그 점유, 드라이브 자기 진단, 장치·RAID 구성, "왜 찼는지" 원인 좁히기 |
| M3 로그·회고 지표 | 다음 | 화면 11·3 |
| M4~M8 | 예정 | [docs/roadmap.md](docs/roadmap.md) |

지금 동작하는 것은 `cargo run -- tasks` 로 확인할 수 있다(작업마다 `ready` 또는 도착 예정 마일스톤 표시).
시험 93개와 CI(fmt · clippy · test · 읽기 전용 정책 검증 · 릴리스 빌드)로 지킨다.

메뉴에는 아직 구현되지 않은 작업도 표시되며, `planned M3` 처럼 어느 마일스톤에서 오는지 함께 보인다.
무엇을 기대할 수 있는지 사용자가 알 수 있어야 하기 때문이다.

## 라이선스

미정.
