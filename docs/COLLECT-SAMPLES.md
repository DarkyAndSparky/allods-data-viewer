# Как собрать базы ресурсов из игры (одна команда)

Базы ресурсов (`pack.bin` и похожие) лежат внутри паков, но не во всех и под разными
именами. `adv db find` сам обходит паки и находит их по сигнатуре.

В PowerShell, всё в `%USERPROFILE%\Downloads\BUILDS` (в PowerShell `$env:USERPROFILE\Downloads\BUILDS`):

```powershell
cd $env:USERPROFILE\Downloads\BUILDS
$adv = ".\pakscope\target\release\adv.exe"   # или target\debug\adv.exe
& $adv db find "C:\AstrumPlay\Аллоды Онлайн\data\Packs" -o .\dbs > .\dbs-list.txt
Compress-Archive -Path .\dbs,.\dbs-list.txt -DestinationPath .\dbs.zip -Force
```

Что получится:
- `dbs-list.txt`: по строке на базу (пак, файл, размер, 12 байт версии);
- папка `dbs\<имя пака>\<файл>`: копии баз;
- `dbs.zip`: всё вместе.

Пришлите `dbs.zip`. Если он слишком большой (главная `pack.bin` весит сотни мегабайт
в распакованном виде), пришлите сначала только `dbs-list.txt` и несколько файлов поменьше.

Команда только читает паки, ничего в них не меняет. Обход 711 паков может занять несколько минут.
