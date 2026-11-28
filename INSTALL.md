# Allods Data Viewer — Install Guide / Руководство по установке

Place this file in the repository, e.g. `docs/INSTALL.md`.
Положите этот файл в репозиторий, например `docs/INSTALL.md`.

**Contents / Содержание:** [English](#english) · [Русский](#русский)

---

<a id="english"></a>

# English

# Install and Build Guide (Windows and Linux)

Windows steps come first; the **Linux** section follows right after.

This guide takes you from "I downloaded the source code" to "the app window is running". No Git and no programming experience are required. Everything is done in **PowerShell**.

## What you need

| Tool | Why | Version |
|---|---|---|
| Rust | compiles the application | 1.100 (installed automatically by the project) |
| Microsoft C++ Build Tools (MSVC) | linker and compiler Rust needs on Windows | any current version |
| Node.js | builds the web interface | **26 or newer** |
| WebView2 Runtime | draws the app window | normally already in Windows 10/11 |

Git is **not** needed.

## Step 1. Unpack the source code

1. Download the source archive (`allods-data-viewer-<version>.zip`).
2. Right-click the zip → **Extract All…** and pick a folder with a short path without special characters, for example `C:\dev`.
3. You should get a folder like `C:\dev\allods-data-viewer-0.1.58` containing `Cargo.toml`, `web`, `crates`, `scripts`.

Below, this folder is called **the project folder**.

## Step 2. Install Microsoft C++ Build Tools (MSVC)

Rust on Windows needs Microsoft's linker. Install it **first**.

1. Open <https://visualstudio.microsoft.com/downloads/> and scroll to **All Downloads → Tools for Visual Studio**.
2. Download **Build Tools for Visual Studio** (the latest version) and run the installer.
3. In the installer tick the workload **Desktop development with C++** and click **Install**. Defaults are fine.
4. Wait until it finishes (it is a large download). A restart is requested only if the installer says so.

If you already have Visual Studio with the "Desktop development with C++" workload, skip this step.

## Step 3. Install Rust

1. Open <https://rustup.rs> and download **rustup-init.exe** (64-bit).
2. Run it and choose **1) Proceed with standard installation**.
3. Close the window when it says Rust is installed.

You do not need to pick a Rust version: the file `rust-toolchain.toml` in the project pins 1.100, and rustup downloads it automatically on the first build.

## Step 4. Install Node.js

1. Open <https://nodejs.org> and download the **LTS** installer for Windows (it must be **26.x** or newer; if the site offers an older LTS, use the "Current" download or the "Previous Releases" page and pick 26.x or newer).
2. Run the installer with default options (keep "Add to PATH" enabled).

## Step 5. Check the installation

Close all PowerShell windows and open a **new** one (this refreshes PATH), then run:

```powershell
rustup --version
cargo --version
node --version
npm --version
```

Expected: all four commands print a version, and `node --version` shows `v26.` or higher.

Optional, one-command alternative to steps 3–4 (Windows 10/11 with winget):

```powershell
winget install Rustlang.Rustup
winget install OpenJS.NodeJS.LTS
```

Then re-check versions as above. MSVC (step 2) still needs to be installed separately.

## Step 6. Build and run

Open PowerShell in the project folder (in Explorer: open the folder, click the address bar, type `powershell`, press Enter), or:

```powershell
cd C:\dev\allods-data-viewer-0.1.58
```

### Option A — one script

```powershell
.\scripts\run-app.ps1
```

The script installs web dependencies, builds the web interface and starts the app window. The first run takes several minutes because Rust downloads and compiles dependencies.

If Windows says scripts are disabled:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-app.ps1
```

or allow local scripts once: `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`.
If the files came from a downloaded zip and are blocked: `Get-ChildItem -Recurse .\scripts | Unblock-File`.

### Option B — manual commands

```powershell
cd web
npm ci
npm run build
cd ..
cargo run --release -p adv-desktop
```

To build without launching:

```powershell
cargo build --release -p adv-desktop
```

The result is `target\release\allods-data-viewer.exe`. The web interface is embedded in it, so the file can be launched directly.

## Step 7. Use the app

Enter the path to your game data folder (for example `C:\AstrumPlay\Аллоды Онлайн\data`) or drag a folder or a `.pak` file into the window, then click **Open**.

## Optional: run the project checks

```powershell
.\scripts\check-all.ps1
```

## Troubleshooting

| Problem | Solution |
|---|---|
| `link.exe not found` | MSVC Build Tools are missing — do Step 2 and open a new PowerShell |
| `cargo` or `node` is not recognized | Open a **new** PowerShell window; if it persists, reinstall the tool |
| `npm ci` complains about Node version | Install Node.js 26 or newer (Step 4) |
| `rustc 1.9x is not supported` | Run `rustup update`, then retry inside the project folder |
| Script cannot be run | See "If Windows says scripts are disabled" in Step 6 |
| Blank window or WebView error | Install/repair Microsoft Edge WebView2 Runtime from <https://developer.microsoft.com/microsoft-edge/webview2/> |
| Build is very slow the first time | Normal: dependencies are compiled once, later builds are fast |

---

# Linux

Same project, same result. The Linux script is `scripts/run-app.sh`. The window needs a desktop session (X11 or Wayland); it will not open on a headless server.

## L1. Unpack the source code

```bash
unzip allods-data-viewer-0.1.58.zip -d ~/dev
cd ~/dev/allods-data-viewer-0.1.58
```

(or use your file manager: right-click → Extract). If `unzip` is missing, install it with your package manager.

## L2. Install system packages (compiler and WebKitGTK)

The app window uses Tauri 2, which needs **WebKitGTK 4.1** development files and a C toolchain.

**Debian / Ubuntu / Mint:**

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl wget file unzip \
  libwebkit2gtk-4.1-dev libssl-dev libxdo-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

**Fedora:**

```bash
sudo dnf install -y @development-tools pkgconf-pkg-config curl wget file unzip \
  webkit2gtk4.1-devel openssl-devel libxdo-devel \
  libappindicator-gtk3-devel librsvg2-devel
```

**Arch / Manjaro:**

```bash
sudo pacman -Syu --needed base-devel curl wget file unzip \
  webkit2gtk-4.1 openssl xdotool libappindicator-gtk3 librsvg
```

Other distributions: install the equivalents of the packages above (`webkit2gtk-4.1` dev package, OpenSSL dev package, a C compiler, `pkg-config`). If the Rust build later complains about a missing `.pc` file, install the dev package it names.

## L3. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Choose **1) Proceed with standard installation**, then load Rust into the current shell:

```bash
. "$HOME/.cargo/env"
```

The project pins Rust 1.100 in `rust-toolchain.toml`; rustup downloads it automatically on the first build.

## L4. Install Node.js 26 or newer

Use one of these:

**Option 1 — nvm (reads the version from the project's `.nvmrc`):**

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/master/install.sh | bash
# open a new terminal, then, in the project folder:
nvm install
nvm use
```

**Option 2 — official downloads:** get the Linux build (**26.x or newer**) from <https://nodejs.org> and follow the instructions there, or use your distribution's package if it provides Node 26+.

## L5. Check the installation

```bash
rustup --version
cargo --version
node --version    # must be v26 or higher
npm --version
pkg-config --modversion webkit2gtk-4.1
```

All commands must print a version. If `cargo` is not found, run `. "$HOME/.cargo/env"` or open a new terminal.

## L6. Build and run

In the project folder:

### Option A — one script

```bash
bash ./scripts/run-app.sh
```

(If you prefer `./scripts/run-app.sh`, make it executable once: `chmod +x scripts/*.sh`. Zip archives often lose the executable bit.)

The script runs `npm ci`, builds the web interface and starts the window with `cargo run --release -p adv-desktop`. The first run takes several minutes.

### Option B — manual commands

```bash
cd web
npm ci
npm run build
cd ..
cargo run --release -p adv-desktop
```

To build without launching:

```bash
cargo build --release -p adv-desktop
```

The result is `target/release/allods-data-viewer`. The web interface is embedded in it. Note that it still needs the WebKitGTK 4.1 runtime library on the machine where it runs.

## L7. Optional: project checks

```bash
bash ./scripts/check-all.sh
```

## Linux troubleshooting

| Problem | Solution |
|---|---|
| `The system library webkit2gtk-4.1 ... was not found` or a missing `.pc` file | Install the WebKitGTK 4.1 dev package from L2 (not the older 4.0 one) |
| `linker cc not found` | Install the C toolchain (`build-essential` / `@development-tools` / `base-devel`) |
| `cargo` not found | Run `. "$HOME/.cargo/env"` or open a new terminal |
| `npm ci` complains about Node version | Install Node.js 26+ (L4); check with `node --version` |
| `Permission denied` on the script | Run it as `bash ./scripts/run-app.sh` or `chmod +x scripts/*.sh` |
| Window does not open / no display | A graphical session is required; over SSH use a desktop session or X forwarding |

---

<a id="русский"></a>

# Русский

# Руководство по установке и сборке (Windows и Linux)

Сначала шаги для Windows, раздел **Linux** идёт следом.

Руководство ведёт от «я скачал исходники» до «окно приложения запущено». Git и опыт программирования не нужны. Все команды выполняются в **PowerShell**.

## Что понадобится

| Инструмент | Зачем | Версия |
|---|---|---|
| Rust | компилирует приложение | 1.100 (проект ставит нужную версию сам) |
| Microsoft C++ Build Tools (MSVC) | компилятор и компоновщик, которые Rust требует на Windows | любая актуальная |
| Node.js | собирает веб-интерфейс | **26 или новее** |
| WebView2 Runtime | рисует окно приложения | обычно уже есть в Windows 10/11 |

Git **не нужен**.

## Шаг 1. Распаковать исходники

1. Скачайте архив с исходниками (`allods-data-viewer-<версия>.zip`).
2. Правый клик по zip → **Извлечь все…** и выберите папку с коротким путём без спецсимволов, например `C:\dev`.
3. Должна получиться папка вроде `C:\dev\allods-data-viewer-0.1.58`, внутри которой лежат `Cargo.toml`, `web`, `crates`, `scripts`.

Дальше эта папка называется **папкой проекта**.

## Шаг 2. Установить Microsoft C++ Build Tools (MSVC)

Rust в Windows нужен компоновщик от Microsoft. Ставьте его **первым**.

1. Откройте <https://visualstudio.microsoft.com/downloads/> и прокрутите до **Все загрузки → Инструменты для Visual Studio**.
2. Скачайте **Build Tools for Visual Studio** (последнюю версию) и запустите установщик.
3. В установщике отметьте рабочую нагрузку **Разработка классических приложений на C++** (Desktop development with C++) и нажмите **Установить**. Параметры по умолчанию подходят.
4. Дождитесь окончания (загрузка большая). Перезагрузка нужна, только если установщик об этом скажет.

Если у вас уже стоит Visual Studio с этой нагрузкой, шаг можно пропустить.

## Шаг 3. Установить Rust

1. Откройте <https://rustup.rs> и скачайте **rustup-init.exe** (64-bit).
2. Запустите его и выберите **1) Proceed with standard installation**.
3. Когда появится сообщение, что Rust установлен, закройте окно.

Выбирать версию Rust не нужно: файл `rust-toolchain.toml` в проекте закрепляет 1.100, и rustup скачает её сам при первой сборке.

## Шаг 4. Установить Node.js

1. Откройте <https://nodejs.org> и скачайте установщик **LTS** для Windows (версия должна быть **26.x или новее**; если сайт предлагает более старую LTS, возьмите «Current» или страницу «Previous Releases» и выберите 26.x или новее).
2. Запустите установщик с параметрами по умолчанию (оставьте включённым «Add to PATH»).

## Шаг 5. Проверить установку

Закройте все окна PowerShell и откройте **новое** (так обновится PATH), затем выполните:

```powershell
rustup --version
cargo --version
node --version
npm --version
```

Ожидаемый результат: все четыре команды выводят версию, а `node --version` показывает `v26.` или выше.

Необязательная альтернатива шагам 3–4 одной командой (Windows 10/11 с winget):

```powershell
winget install Rustlang.Rustup
winget install OpenJS.NodeJS.LTS
```

После этого снова проверьте версии. MSVC (шаг 2) всё равно ставится отдельно.

## Шаг 6. Собрать и запустить

Откройте PowerShell в папке проекта (в Проводнике: откройте папку, щёлкните адресную строку, введите `powershell`, нажмите Enter) или выполните:

```powershell
cd C:\dev\allods-data-viewer-0.1.58
```

### Вариант A — одним скриптом

```powershell
.\scripts\run-app.ps1
```

Скрипт ставит зависимости веб-части, собирает веб-интерфейс и запускает окно приложения. Первый запуск занимает несколько минут: Rust скачивает и компилирует зависимости.

Если Windows пишет, что запуск скриптов отключён:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-app.ps1
```

или один раз разрешите локальные скрипты: `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`.
Если файлы из скачанного zip заблокированы: `Get-ChildItem -Recurse .\scripts | Unblock-File`.

### Вариант B — вручную командами

```powershell
cd web
npm ci
npm run build
cd ..
cargo run --release -p adv-desktop
```

Чтобы только собрать, без запуска:

```powershell
cargo build --release -p adv-desktop
```

Результат: `target\release\allods-data-viewer.exe`. Веб-интерфейс встроен в него, поэтому файл можно запускать напрямую.

## Шаг 7. Работа с приложением

Введите путь к папке с данными игры (например `C:\AstrumPlay\Аллоды Онлайн\data`) или перетащите папку либо файл `.pak` в окно и нажмите **Открыть**.

## По желанию: запустить проверки проекта

```powershell
.\scripts\check-all.ps1
```

## Если что-то не так

| Проблема | Решение |
|---|---|
| `link.exe not found` | Не стоят MSVC Build Tools — выполните шаг 2 и откройте новый PowerShell |
| `cargo` или `node` не распознаётся | Откройте **новое** окно PowerShell; если не помогло, переустановите инструмент |
| `npm ci` ругается на версию Node | Установите Node.js 26 или новее (шаг 4) |
| `rustc 1.9x is not supported` | Выполните `rustup update` и повторите в папке проекта |
| Скрипт не запускается | См. «Если Windows пишет, что запуск скриптов отключён» в шаге 6 |
| Пустое окно или ошибка WebView | Установите или восстановите Microsoft Edge WebView2 Runtime: <https://developer.microsoft.com/microsoft-edge/webview2/> |
| Первая сборка очень долгая | Это нормально: зависимости компилируются один раз, дальше быстрее |

---

# Linux

Тот же проект, тот же результат. Скрипт для Linux: `scripts/run-app.sh`. Окну нужна графическая сессия (X11 или Wayland); на сервере без экрана оно не откроется.

## L1. Распаковать исходники

```bash
unzip allods-data-viewer-0.1.58.zip -d ~/dev
cd ~/dev/allods-data-viewer-0.1.58
```

(или файловым менеджером: правый клик → Извлечь). Если `unzip` нет, поставьте его пакетным менеджером.

## L2. Установить системные пакеты (компилятор и WebKitGTK)

Окно приложения построено на Tauri 2, которому нужны dev-файлы **WebKitGTK 4.1** и C-компилятор.

**Debian / Ubuntu / Mint:**

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl wget file unzip \
  libwebkit2gtk-4.1-dev libssl-dev libxdo-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

**Fedora:**

```bash
sudo dnf install -y @development-tools pkgconf-pkg-config curl wget file unzip \
  webkit2gtk4.1-devel openssl-devel libxdo-devel \
  libappindicator-gtk3-devel librsvg2-devel
```

**Arch / Manjaro:**

```bash
sudo pacman -Syu --needed base-devel curl wget file unzip \
  webkit2gtk-4.1 openssl xdotool libappindicator-gtk3 librsvg
```

Другие дистрибутивы: поставьте аналоги этих пакетов (dev-пакет `webkit2gtk-4.1`, dev-пакет OpenSSL, C-компилятор, `pkg-config`). Если сборка Rust потом пожалуется на отсутствующий `.pc`-файл, поставьте dev-пакет, который она назвала.

## L3. Установить Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Выберите **1) Proceed with standard installation**, затем подключите Rust в текущую оболочку:

```bash
. "$HOME/.cargo/env"
```

Версия Rust 1.100 закреплена в `rust-toolchain.toml`, rustup скачает её сам при первой сборке.

## L4. Установить Node.js 26 или новее

Любой из способов:

**Способ 1 — nvm (берёт версию из `.nvmrc` проекта):**

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/master/install.sh | bash
# откройте новый терминал и в папке проекта выполните:
nvm install
nvm use
```

**Способ 2 — официальные сборки:** скачайте Linux-версию (**26.x или новее**) с <https://nodejs.org> и следуйте инструкции там, либо возьмите пакет из репозитория дистрибутива, если в нём есть Node 26+.

## L5. Проверить установку

```bash
rustup --version
cargo --version
node --version    # должна быть v26 или выше
npm --version
pkg-config --modversion webkit2gtk-4.1
```

Все команды должны вывести версию. Если `cargo` не найден, выполните `. "$HOME/.cargo/env"` или откройте новый терминал.

## L6. Собрать и запустить

В папке проекта.

### Вариант A — одним скриптом

```bash
bash ./scripts/run-app.sh
```

(Если хотите запускать как `./scripts/run-app.sh`, один раз сделайте файлы исполняемыми: `chmod +x scripts/*.sh`. В zip-архивах бит исполнения часто теряется.)

Скрипт выполняет `npm ci`, собирает веб-интерфейс и запускает окно командой `cargo run --release -p adv-desktop`. Первый запуск занимает несколько минут.

### Вариант B — вручную командами

```bash
cd web
npm ci
npm run build
cd ..
cargo run --release -p adv-desktop
```

Чтобы только собрать, без запуска:

```bash
cargo build --release -p adv-desktop
```

Результат: `target/release/allods-data-viewer`. Веб-интерфейс встроен в него. Учтите, что на машине, где файл запускается, всё равно нужна библиотека WebKitGTK 4.1.

## L7. По желанию: проверки проекта

```bash
bash ./scripts/check-all.sh
```

## Если что-то не так на Linux

| Проблема | Решение |
|---|---|
| `The system library webkit2gtk-4.1 ... was not found` или нет `.pc`-файла | Поставьте dev-пакет WebKitGTK 4.1 из L2 (не старый 4.0) |
| `linker cc not found` | Поставьте C-компилятор (`build-essential` / `@development-tools` / `base-devel`) |
| `cargo` не найден | Выполните `. "$HOME/.cargo/env"` или откройте новый терминал |
| `npm ci` ругается на версию Node | Поставьте Node.js 26+ (L4), проверьте `node --version` |
| `Permission denied` при запуске скрипта | Запускайте как `bash ./scripts/run-app.sh` или `chmod +x scripts/*.sh` |
| Окно не открывается / нет дисплея | Нужна графическая сессия; по SSH используйте рабочий стол или X-forwarding |
