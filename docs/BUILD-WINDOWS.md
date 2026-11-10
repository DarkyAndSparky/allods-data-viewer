# Сборка и проверка на Windows

Инструкция для владельца проекта. Все команды в PowerShell. Проект лежит в `C:\DEVELOP\allods-data-viewer`. Требования: Rust 1.99 (ставится сам по `rust-toolchain.toml`), Node.js 26 или новее (`.nvmrc`, `engines` в `web/package.json`).

## 1. Один раз: поставить инструменты

| Что | Где взять | Проверка |
|---|---|---|
| Git | git-scm.com | `git --version` |
| Rust | rustup.rs, запустить `rustup-init.exe`, вариант по умолчанию | `rustc --version` |
| Сборочные инструменты MSVC | «Visual Studio Build Tools», компонент «Разработка классических приложений на C++» (если стоит Visual Studio с C++, достаточно) | `cargo build` не ругается на `link.exe` |
| Node.js 26 | nodejs.org, установщик LTS 26.x | `node --version` показывает v26.x |

После установки откройте **новое** окно PowerShell, чтобы подхватился PATH.

## 2. Проверить ядро и CLI

```powershell
cd C:\DEVELOP\allods-data-viewer
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p adv-cli
```

Первый запуск скачает Rust 1.99 (rustup сделает это сам из `rust-toolchain.toml`) и зависимости, это несколько минут. Последняя команда должна напечатать `Allods Data Viewer 0.1.3 (core 0.1.2)`.

## 3. Проверить интерфейс

```powershell
cd C:\DEVELOP\allods-data-viewer\web
npm ci
npm run typecheck
npm test
npm run build
```

Если `npm ci` пишет про версию Node, стоит не 26.

## 4. Проверки репозитория

```powershell
cd C:\DEVELOP\allods-data-viewer
node scripts/check-repo.mjs
node --test scripts/guard.test.mjs scripts/check-repo.test.mjs
```

Первая команда должна вывести `check-repo: ok`.

## 4а. Попробовать на своих паках

Пути ниже примерные, подставьте свои. Игровые файлы в репозиторий не кладите.

```powershell
cd C:\DEVELOP\allods-data-viewer
cargo run -p adv-cli -- info "D:\Games\Allods\Packs\Texts.pak"
cargo run -p adv-cli -- list "D:\Games\Allods\Packs\Texts.pak" --long
cargo run -p adv-cli -- extract "D:\Games\Allods\Packs\Texts.pak" -o C:\DEVELOP\adv-out
cargo run -p adv-cli -- loc dump C:\DEVELOP\adv-out\Bin\pack.rus.loc --format csv --bom -o C:\DEVELOP\rus.csv
cargo run -p adv-cli -- search "D:\Games\Allods\Packs" "Камень" --limit 5
```

Папку `adv-out` держите вне репозитория. Код выхода 2 означает «часть пропущена или файлы уже были», это не сбой.

## 5. Что прислать

Текст вывода каждого шага целиком (особенно красные строки). Если всё зелёное, достаточно «всё прошло» и версии `rustc --version`, `node --version`.

## 6. Если что-то не так

| Симптом | Что делать |
|---|---|
| `link.exe not found` | не стоят Build Tools MSVC (раздел 1) |
| `rustc 1.9x is not supported` | старый Rust: `rustup update` и повторить из папки проекта |
| `npm ci` ругается на engines | поставить Node 26 и открыть новое окно |
| Падает один тест | прислать вывод, не править тест |

## 7. Сбор образцов

Для проверки форматов запустите `tools\collect-samples.ps1`, см. `docs/SAMPLES_REQUEST.md`. Образцы в репозиторий не кладём.
