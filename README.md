# Screen Saver Timer

Rust로 만든 macOS 전용 입력 기반 화면 보호기 앱입니다. 한국어 GUI에서 시·분·초를 설정하고, 시작/취소, 다음 실행까지 남은 시간, 즉시 실행, 엔진 프로세스 상태를 확인할 수 있습니다. 기본값은 5분이며 1초~24시간을 지원합니다.

## 실행

`dist/Screen Saver Timer.app`을 Finder에서 더블 클릭하세요. Rust 설치 없이 실행할 수 있습니다. 현재 번들은 Apple Silicon(arm64)용이며 Intel Mac에서는 소스를 다시 빌드해야 합니다. 로컬 ad-hoc 서명만 되어 있고 Apple 공증은 받지 않았습니다.

1. 화면 보호기 실행 전 기다릴 시간을 설정하고 **감시 시작**을 누릅니다.
2. 마지막 마우스 이동, 클릭, 스크롤 또는 키 입력부터 시간을 셉니다. 어느 앱에서든 새 입력이 들어오면 설정 시간으로 돌아갑니다. 입력 내용은 읽거나 저장하지 않습니다.
3. 창을 최소화하거나 다른 앱을 사용해도 macOS 메뉴 막대의 타이머 아이콘 옆에 다음 실행까지 남은 시간이 표시됩니다. 메뉴 막대 아이콘을 클릭하면 앱 창이 다시 열립니다. 하단 Dock 앱 아이콘에도 같은 시간이 배지로 표시되며 감시를 중지하면 배지가 지워집니다. Dock에 고정하려면 아이콘을 우클릭하고 옵션에서 Dock에 유지를 선택하세요.
4. **취소** 또는 앱 종료로 감시를 중단합니다. **지금 실행**은 즉시 실행을 요청하며 설정이 켜져 있으면 감시를 계속합니다.

실행 요청 뒤에는 새로운 대기 시간이 시작됩니다. 화면 보호기에서 돌아올 때의 입력도 카운트다운을 초기화합니다. 잠자기 중 앱은 Mac을 깨우지 않으며 놓친 실행을 연달아 보내지 않습니다. 시스템 화면 보호기와 잠금 설정은 그대로 적용됩니다.

## 실행·감지 방식과 macOS 제약

- `/usr/bin/open -a /System/Library/CoreServices/ScreenSaverEngine.app`으로 LaunchServices에 요청합니다. 엔진 바이너리를 직접 실행하거나 비공개 프레임워크를 링크하지 않습니다.
- 명령 성공은 **요청 수락**을 뜻합니다. 실제 화면 보호기 표시나 잠금 성공을 보증하지 않으며 GUI도 이를 명시합니다. 실행 파일 부재, 명령 오류, 5초 응답 시간 초과는 오류로 표시합니다. 실패 시 시스템 설정에서 화면 보호기 선택과 미리 보기를 확인하세요.
- 현재 로그인 UID의 `ScreenSaverEngine` 프로세스를 2초마다 조회합니다. 감지됨/미감지/조회 불가를 구분합니다. 감지됨은 화면 표시의 증명이 아니며 미감지도 비활성 상태의 증명이 아닙니다. 화면 녹화나 접근성 권한은 요청하지 않습니다.
- Apple의 ScreenSaver 공개 문서는 화면 보호기 플러그인 생명주기를 다루며, 이 앱은 OS 버전 전체에 걸친 공개 활성 상태 API가 있다고 가정하지 않습니다.
- macOS 14/15 이후의 배경화면·확장 기반 렌더링은 엔진 프로세스와 실제 표시가 분리될 수 있습니다. Aerial 개발자의 직접 관찰에도 `open -a ScreenSaverEngine` 요청 이후 WallpaperAgent가 화면을 만드는 흐름이 기록되어 있습니다. 따라서 해당 에이전트의 존재만으로 활성 상태를 추정하지 않습니다.
- 빌드 최소 대상은 macOS 12입니다. 실제 확인 환경은 **macOS 26.6.2 / Apple Silicon**뿐입니다. 이 환경에서 엔진 앱의 존재, bundle ID `com.apple.ScreenSaver.Engine`, LaunchServices 명령 인터페이스를 확인했습니다. 이전 OS 버전의 실제 실행 호환성은 검증하지 않았습니다.
- 사용 중인 대화를 방해하지 않도록 실제 화면 보호기 실행 및 잠금, 잠자기 복귀 통합 검증은 수행하지 않았습니다. 사용자가 편한 시간에 5초 타이머와 즉시 실행을 각각 확인할 수 있습니다.

참고 자료:
- [Apple ScreenSaver framework](https://developer.apple.com/documentation/ScreenSaver)
- [Apple: 핫 코너로 화면 보호기 시작](https://support.apple.com/en-afri/guide/mac-help/mchlp3000/mac)
- [Aerial: screensaver extension lifecycle 직접 관찰](https://aerialscreensaver.github.io/developers/appex-lifecycle/)

## 개발 및 검증

GUI는 eframe/egui 0.31.1, 타이머와 macOS 제어는 Rust로 작성했습니다. Core Graphics에서 마지막 세션 입력 이후 경과 시간만 조회합니다. AppKit 메뉴 막대는 메인 런루프에서 창 렌더링과 별도로 갱신합니다. 타이머 작업 스레드와 상태 조회 스레드를 분리하여 GUI 렌더링 및 프로세스 조회가 타이머를 멈추지 않도록 했습니다. OS 요청 중에는 중복 실행 버튼을 비활성화하며 취소는 계속 가능합니다. 앱 번들은 App Nap을 비활성화하지만 운영체제 잠자기를 막지는 않습니다.

```sh
cd screensaver-timer
./scripts/cargo.sh test --locked
./scripts/cargo.sh fmt --check
./scripts/cargo.sh clippy --locked --all-targets -- -D warnings
./scripts/build-app.sh
./scripts/build-dmg.sh
```

이 컴퓨터에 Rust가 없어 `.cargo/`, `.rustup/`에 도구체인을 프로젝트 내부 설치했습니다. 전역 PATH나 셸 설정은 변경하지 않았습니다. `scripts/cargo.sh`가 해당 도구체인을 자동 사용합니다. 다른 Mac에서는 일반 Rust 도구체인을 설치하면 됩니다. `Cargo.lock`을 포함하여 의존성을 고정합니다.

화면 보호기를 켜지 않는 확인 명령:

```sh
./target/release/screensaver-timer --diagnose
./target/release/screensaver-timer --dry-run
./target/release/screensaver-timer --smoke-test
```

- `--diagnose`: 엔진 파일과 프로세스 상태만 조회하고 종료합니다.
- `--dry-run`: 실제 GUI를 사용하되 모든 실행 요청을 기록 메시지로 대체합니다.
- `--smoke-test`: 안전 모드를 강제하고 GUI에서 1초 타이머를 예약합니다. 최소 3주기의 모의 실행 이후 취소하고 추가 주기가 실행되지 않는지 1.6초 이상 확인하면 `SMOKE_OK`를 출력하고 종료합니다. 전체 제한은 15초입니다.

단위 테스트는 경계값, 취소, 입력 초기화, 여러 주기 반복과 취소 후 중단, 늦은 요청의 몰아 실행 방지, 입력 상태 조회 실패, 남은 시간 올림, 명령 오류 및 안전 모드 경로를 검증합니다.

## DMG 설치와 GitHub 배포

[GitHub Release](https://github.com/alzkdpf/screensaver-timer/releases/latest)에서 DMG를 받을 수 있습니다.

1. 제공된 `Screen-Saver-Timer-1.3.1-arm64.dmg`를 엽니다.
2. `Screen Saver Timer.app`을 같은 창의 `Applications` 바로가기로 드래그합니다.
3. DMG를 추출하고 응용 프로그램 폴더의 앱을 실행합니다.

Apple Silicon 전용입니다. Developer ID 인증서가 없어 ad-hoc 서명만 했으며 Apple 공증은 받지 않았습니다. 다운로드한 앱은 Gatekeeper에 의해 차단될 수 있습니다. 출처를 확인한 후 최초 실행을 시도하고 **시스템 설정 → 개인정보 보호 및 보안 → 그래도 열기**에서 해당 앱을 승인할 수 있습니다. 시스템 전체 보안 설정이나 격리 속성은 자동 변경하지 않습니다. [Apple의 공식 안내](https://support.apple.com/en-us/102445)를 참고하세요.

`SHA256SUMS.txt`로 DMG 무결성을 확인할 수 있습니다. Intel Mac 및 다른 macOS 버전은 검증하지 않았습니다.

배포 전 정보 노출 검사 범위와 수정 내역은 [SECURITY-CHECK.md](SECURITY-CHECK.md)를 확인하세요.
