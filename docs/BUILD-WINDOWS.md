# Сборка и проверка на Windows

Инструкция для владельца проекта. Все команды в PowerShell. Архив распаковывается в `%USERPROFILE%\Downloads\BUILDS\allods-data-viewer-<версия>`, там же собираем и запускаем; результаты экспорта кладём рядом, в `%USERPROFILE%\Downloads\BUILDS` (не в репозиторий). `C:\DEVELOP\allods-data-viewer` нужна только для заливки на GitHub. Требования: Rust 1.99 (ставится сам по `rust-toolchain.toml`), Node.js 26 или новее (`.nvmrc`, `engines` в `web/package.json`).

## 0. Быстрый путь: готовые скрипты

В папке `scripts` лежат скрипты для Windows (`.ps1`) и Linux (`.sh`). Файлы `.ps1` сохранены в UTF-8 с BOM, поэтому русские сообщения и пути с кириллицей работают в Windows PowerShell 5.1 и в PowerShell 7. Запуск из корня распакованной папки:

| Что нужно | Windows (PowerShell) | Linux |
|---|---|---|
| Все проверки подряд (веб, fmt, clippy, тесты, check-repo, версия) | `.\scripts\check-all.ps1` | `./scripts/check-all.sh` |
| Собрать веб-часть и запустить окно | `.\scripts\run-app.ps1` | `./scripts/run-app.sh` |
| Тесты на реальных паках | `.\scripts\samples-test.ps1 -Packs "C:\AstrumPlay\Аллоды Онлайн\data\Packs"` | `./scripts/samples-test.sh /путь/к/Packs` |
| «Паспорт» клиента (список файлов и мелкие файлы версий) | `.\scripts\collect-client-info.ps1 -Client "C:\AstrumPlay\Аллоды Онлайн" -Name current` | `./scripts/collect-client-info.sh /путь/к/клиенту current` |

Если PowerShell запрещает запуск скриптов, один раз выполните `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`, либо запускайте так: `powershell -ExecutionPolicy Bypass -File .\scripts\check-all.ps1`. Скрипты ничего не копируют из игры и ничего не отправляют; паспорт клиента кладётся в `%USERPROFILE%\Downloads\BUILDS\client-<имя>`. Если шаг упал, скрипт останавливается и пишет, на каком шаге; пришлите последние строки вывода. Разделы ниже описывают те же шаги вручную.

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
cd $env:USERPROFILE\Downloads\BUILDS\allods-data-viewer-<версия>
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p adv-cli
```

Первый запуск скачает Rust 1.99 (rustup сделает это сам из `rust-toolchain.toml`) и зависимости, это несколько минут. Последняя команда должна напечатать строку вида `Allods Data Viewer <версия> (core <версия>)`.

## 3. Проверить интерфейс

```powershell
cd $env:USERPROFILE\Downloads\BUILDS\allods-data-viewer-<версия>\web
npm ci
npm run typecheck
npm test
npm run build
```

Если `npm ci` пишет про версию Node, стоит не 26.

## 3а. Окно приложения (0.1.73)

Нужны: Rust (уже стоит), Node 26, Visual Studio Build Tools с C++ (у вас есть) и WebView2 (в Windows 10/11 обычно уже установлен). Сначала собрать веб-часть, потом запустить окно:

```powershell
cd $env:USERPROFILE\Downloads\BUILDS\allods-data-viewer-<версия>\web
npm ci
npm run build
cd ..
cargo run --release -p adv-desktop
```

Первая сборка Tauri занимает несколько минут. В окне введите путь `C:\AstrumPlay\Аллоды Онлайн\data` (или перетащите папку или файл `.pak` в окно) и нажмите «Открыть». Слева дерево, справа предпросмотр: текст (в том числе `.log`, `.cfg.bak` и другие файлы, которые похожи на текст), картинки (`.ico`, `.dds`, `.png`, `.jpg`, `.gif`, `.bmp`; масштаб: ползунок или Ctrl и колесо мыши над картинкой), текстуры (размер угадывается, другой размер выбирается списком), звук (кнопка «Играть» на каждом потоке). Язык переключается справа сверху. Если что-то не работает, пришлите скриншот и текст из блока «Подробности».

На Linux для сборки окна нужны системные пакеты webkit2gtk 4.1, на Windows ничего дополнительного не требуется.

## 4. Проверки репозитория

```powershell
cd $env:USERPROFILE\Downloads\BUILDS\allods-data-viewer-<версия>
node scripts/check-repo.mjs
node --test scripts/guard.test.mjs scripts/check-repo.test.mjs
```

Первая команда должна вывести `check-repo: ok`.

## 4а. Попробовать на своих паках

Пути ниже примерные, подставьте свои. Игровые файлы в репозиторий не кладите.

```powershell
cd $env:USERPROFILE\Downloads\BUILDS\allods-data-viewer-<версия>
cargo run -p adv-cli -- info "C:\AstrumPlay\Аллоды Онлайн\data\Packs\Texts_x64.pak"
cargo run -p adv-cli -- list "C:\AstrumPlay\Аллоды Онлайн\data\Packs\Texts_x64.pak" --long
cargo run -p adv-cli -- extract "C:\AstrumPlay\Аллоды Онлайн\data\Packs\Texts_x64.pak" -o $env:USERPROFILE\Downloads\BUILDS\adv-out
cargo run -p adv-cli -- loc dump $env:USERPROFILE\Downloads\BUILDS\adv-out\Bin\pack.rus.loc --format csv --bom -o $env:USERPROFILE\Downloads\BUILDS\rus.csv
cargo run -p adv-cli -- search "C:\AstrumPlay\Аллоды Онлайн\data\Packs" "Камень" --limit 5
```

Звук (с 0.1.34 всё внутри программы, внешние программы не нужны):

```powershell
cargo run -p adv-cli -- audio list C:\путь\Voice_Kvator_Phrases_3_rus.fsb
cargo run -p adv-cli -- audio export C:\путь\Voice_Kvator_Phrases_3_rus.fsb -o $env:USERPROFILE\Downloads\BUILDS\adv-wav
```

Папку `adv-out` держите вне репозитория. Код выхода 2 означает «часть пропущена или файлы уже были», это не сбой.

### Тесты на реальных файлах

Эти тесты читают настоящие паки и проверяют, что каждая запись читается с верной CRC, каждый `.loc` разбирается, индексы геометрий лежат внутри буфера вершин и каждая анимация декодируется. Без переменной `ADV_SAMPLES` они ничего не делают. Файлы из папки никуда не копируются.

```powershell
cd $env:USERPROFILE\Downloads\BUILDS\allods-data-viewer-<версия>
$env:ADV_SAMPLES = "C:\AstrumPlay\Аллоды Онлайн\data\Packs"
cargo test -p adv-core --test samples -- --nocapture
```

Папка просматривается вместе с подпапками. Нужны только `.pak`. Если тест упал, пришлите последние строки вывода.

### Список файлов клиента для изучения баз ресурсов

Файлы не копируются, нужен только список. Результат сохраняется в `%USERPROFILE%\Downloads\BUILDS\data-list.txt`:

```powershell
$d = "C:\AstrumPlay\Аллоды Онлайн\data"
Get-ChildItem -Path $d -Recurse -File | Where-Object { $_.Extension -in ".bin",".pak",".rev",".version" -and $_.FullName -notmatch "\\Cache\\|\\Mods\\" } | Select-Object @{n="path";e={$_.FullName.Substring($d.Length)}}, Length | Sort-Object path | Format-Table -AutoSize -Wrap | Out-String -Width 400 | Set-Content "$env:USERPROFILE\Downloads\BUILDS\data-list.txt" -Encoding UTF8
```

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
