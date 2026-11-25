# Размер и формат текстуры без `.xdb`: эвристика

**Статус: проверено на 438 текстурах с известными размерами (аддоны), 2026-11-11.**
Источник: сообщество, программа «Allods Online Texture Viewer» (Yuri Grachev / Altair, версия 1.6.1).
Автор дал разрешение на исследование (со слов владельца проекта; письменной лицензии нет).
Код программы не копируется: поведение описано здесь и написано заново (`crates/core/src/texheur.rs`).

## Идея

`.bin` текстуры не хранит ширину, высоту и формат. Программа выводит их из размера данных:

1. **Формат** по блокам альфы. Для каждого 16-байтового блока всех mip-уровней берутся первые
   8 байт блока. Считаются совпадения с шаблонами:
   - DXT3: `ff ff ff ff ff ff ff ff`;
   - DXT5: `04 29 03 29 aa aa aa aa`, `f3 9c d3 9c aa aa aa aa`, `ff ff 00 00 00 00 00 00`,
     `00 01 ff…ff`, `00 05 ff…ff` (8 байт), и шаблон из 6 байт `49 92 24 49 92 24` со смещением 2.
   - Итог: если есть совпадения DXT3, то DXT3, когда их больше, чем DXT5, иначе DXT5; если есть
     только DXT5, то DXT5; если совпадений нет, DXT1.
   - Перечисление в программе: Unknown=0, DXT1=1, DXT3=2, DXT5=3.
2. **Число пикселей** уровня 0: байты (для DXT1 ×2, для DXT3/DXT5 ×1).
3. **Кандидаты размера.** `s = floor(sqrt(пикселей))`. Если `s` степень двойки, то `s×s`; иначе
   `w = NextPowerOf2(s)`, `h = пикселей / w`, `h` должна быть степенью двойки.
   Затем от `(w, h)` строится список: ширина пополам и высота вдвое больше, пока произведение
   равно числу пикселей, `w >= 4`, `h <= 4096`; потом то же в обратную сторону (`w <= 4096`, `h >= 4`).
   Исходная пара попадает в список дважды. Список сортируется по ширине (стабильно).
4. **Выбор по умолчанию.** Если слотов mip больше одного (старший индекс + 1 = `n`), берётся
   первый кандидат с `min(w, h) = 2^(n+1)`; иначе средний (`count/2`, минус 1 при чётном `count`).
5. **Отступ** (прозрачные края): число полностью прозрачных столбцов справа и строк снизу
   на уровне 0; `realWidth = width − paddingWidth`.

## Проверка

На 438 текстурах из аддонов (есть `.xdb` с точным размером): размер угадан точно у 318,
у 108 верный размер есть среди кандидатов, но выбран другой, у 12 верного размера нет
(мелкие текстуры, у которых отброшены уровни). Формат совпал у 427 из 438.
Вывод: догадка полезна, но не надёжна; поэтому `adv tex convert --index N` позволяет выбрать
другого кандидата, а надёжный источник остаётся `.xdb` или база ресурсов.

## Файл `textures_database.bin` (TDB1/TDB2)

Запись: строка пути (.NET `BinaryReader.ReadString`: длина 7-битная, затем UTF-8), затем
`u16 index` (индекс кандидата в списке выше), `u8 format` (0..3 как выше) и, в TDB2,
`u16 padW`, `u16 padH`. TDB1 (`"TDB "`) без отступов. Раньше «7 неизвестных байт»
в [texture-list-tdb2.md](texture-list-tdb2.md): это именно эти поля. Индекс имеет смысл только вместе
с данными текстуры, потому что список кандидатов строится из размера файла.

## Orientation by content and wide-first tie-break (0.1.82)

Status: **verified** on the 14 108 textures of client 1.1.00.41 that have a record in `Bin/pack.bin`
(Characters, Items, Creatures, Ships, Spells, World races). The database size was taken as truth and
compared with the guess made from the `.bin` alone.

| Guess | Size right | Format right |
|---|---|---|
| Before 0.1.82 (first candidate with the mip-count short side) | 4 213 (30 %) | about 99.5 % |
| 0.1.82 | 13 824 (98 %) | about 99.5 % |

What changed:

1. The mip count of a texture fixes only the short side (`2^(mips+1)`), so a wide and a tall picture
   with the same short side are equally likely by size. The wide one is chosen by default: 11 122 of
   the 14 299 non-square textures in the database are wider than tall.
2. Equal-size candidates are then decoded at a mid-sized mip level (the smallest level with at least
   256 blocks) and scored by the luminance step across the edges of the 4x4 blocks, horizontal and
   vertical edges averaged separately. A picture decoded with the wrong width glues unrelated blocks
   together. Another candidate replaces the default only when its score is at least 15 % lower.
   Candidates with fewer than two blocks in a direction are not scored.

Left wrong (284 of 14 108): thin hair and glove textures (256x64, 256x32, 256x16) with noisy content,
`mips = 1` textures of 256x4, and about 60 textures whose format (DXT5 against DXT1) is not told apart
by the alpha block patterns. The resource database stays the reliable source when it is available.
