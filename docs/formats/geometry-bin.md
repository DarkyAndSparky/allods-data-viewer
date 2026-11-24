# Model geometry `X.(Geometry).bin`

**Status: container verified (2026-11-14): vertex block, index block and the next block size are read exactly; vertex stride is guessed, the tail is not decoded.** Samples: `AirElemental` and
`Anakonda` from the owner's `Desktop.zip` (2026-11-10). Analysed 2026-11-11.

## Container

- A zlib stream. Inflated sizes: AirElemental 214 606 bytes, Anakonda 97 766 bytes.
- `Kaa.(Geometry).bin` in the Anakonda folder inflates to a file that starts with the same
  bytes as `Anakonda.(Geometry).bin` (same vertex block header and size); we did not compare
  the rest.

## Layout seen

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 | u32 | blob id 0 = vertex buffer (see "Blob list" below) |
| 4 | u32 | size in bytes of the vertex block |
| 8 | block size | vertices, little endian, fixed stride |
| 8 + block | u32 | blob id 1 = index buffer |
| 12 + block | u32 | 32 718 (AirElemental), 50 154 (Anakonda); meaning unknown, it is not a byte count of the index data |
| 16 + block | u16 each | triangle list: indices, three per triangle |
| after the indices | rest | unknown table, 81 104 bytes in AirElemental; looks like a list of small integers |

## Vertices

- First 20 bytes of a vertex: position as 3 x f32, then texture coordinates as 2 x f32.
  Checked on real data: positions are plausible (AirElemental spans about 6.8 x 5.7 x 5.5
  units; Anakonda is a long thin body) and the triangles drawn from the index list form
  a recognisable shape.
- **The stride is not constant.** AirElemental: 36 bytes (2799 vertices; the last 16 bytes
  look like packed normal/tangent bytes plus a byte pattern `ff 00 00 00` that repeats).
  Anakonda: 32 bytes (1290 vertices). Where the stride is stored is not known; the file
  may carry it in the tail or in the resource database (G-19).
- In AirElemental the first indices are `0 1 2 3 4 5 ...`, then ordinary triangles. All
  indices in the first 16 361 u16 values are below the vertex count (2799), then values
  jump above it, so about 16 360 u16 belong to the triangle list (the exact count is not
  known; 16 360 is not a multiple of 3, so some sub-meshes may share the stream).

## Not known

- Stride guess (2026-11-20): candidates 24, 28, 32, 36 that divide the vertex block and keep all indices inside; the winner has the fewest non-finite positions (first three floats of each vertex), then the `ff ff ff` tag, then the fewest vertices no index points to. Verified on client 2.0 models `Ship4_Hull` (28 bytes, 12 288 vertices), `ElfFemale` (36, 21 664), `ElfFemale_CutScene` (36, 21 734), `HadaganFemale` (36, 23 830): all four have only finite positions, which the old rule (first fit, picked 24) failed. The real stride is still not stored anywhere that is known.
- Vertex stride rule, vertex format flags, number of sub-meshes and their index ranges.
- Whether the bone index of a vertex is the skeleton index or a palette slot (see "Vertex skin data").
- Which texture goes on which sub-mesh (materials are a separate, unknown format).
- Whether the unit is metres or arbitrary.

## Next steps

1. Binding of elements to textures in 7.0 is done: [model-binding.md](model-binding.md). Left: the 18.0 layout and the other material fields.
2. Skinned element data (`skinIndex`, `virtualOffset`).

## Container, confirmed (2026-11-14)

Compared with the original AODataViewer (author's permission, no code copied) and checked on
AirElemental, Anakonda and Kaa:

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 | u32 | blob id 0 (vertex buffer) |
| 4 | u32 `V` | vertex block size in bytes |
| 8 | `V` | vertices |
| 8 + V | u32 | blob id 1 (index buffer) |
| 12 + V | u32 `F` | **index block size in bytes**; always a multiple of 6 (three u16 per triangle) |
| 16 + V | `F` | u16 indices |
| 16 + V + F | u32 | blob id 2 (skeleton) |
| 20 + V + F | u32 `S` | size of the next block (5620 in AirElemental, 6308 in Anakonda/Kaa) |
| 24 + V + F | rest | in Anakonda exactly `S` bytes; in AirElemental 81100 bytes: `S` bytes and then more blocks (a count `3`, sizes 75472, 44912, ...), not decoded |

Counts: AirElemental 2799 vertices, 5453 triangles; Anakonda 1290 vertices, 8359 triangles.
This replaces the earlier note that the u32 at `12 + V` is not a byte count: it is.

The original viewer also guesses the stride: candidates 24, 28, 32 and 36 that divide `V`, and
a candidate is accepted when the last three bytes of the 5th or 7th vertex are `ff ff ff`.
We additionally require that every index is below the vertex count. Result: AirElemental 36,
Anakonda 32. Position at +0 and UV at +12 are the first 20 bytes of each vertex.
`adv geom info` and `adv geom obj` (0.1.11) use this. `adv geom glb` (0.1.21) writes a binary glTF 2.0
(positions, UVs not flipped, u16 triangles); a file made from AirElemental loads in Three.js `GLTFLoader` with
2799 vertices and 5453 triangles. The triangle winding is not verified against the game.

## Tail block: skeleton data (draft, 2026-11-15)

Resource file names carry their type: `<Name>.(Geometry).bin`, `<Name>.(Texture).bin` (with
`<Name>.hi.(Texture).bin` for the sharp levels), `<Name>.<Clip>.(SkeletalAnimation).bin`. The app
uses the tag to tell models from textures without opening the files.

## Skeleton in the tail block (2026-11-16, read by `adv_core::skeleton`)

The block after the index data (`S` bytes, see the table above) holds the skeleton. Checked on
AirElemental (46 bones), Anakonda and Kaa (52 bones). Status: the layout below reads all three files
into a consistent skeleton (one root, no loops, sensible names and parents); what the matrix and position
mean is not verified against the game.

- It starts with four `(offset, count)` pairs of `u32`: AirElemental `(32, 46) (2416, 46) (5512, 46)
  (2768, 46)`. **Each offset is relative to the address of its own pair**, so pair `k` (at byte `8 * k`)
  points to `8 * k + offset`: bone entries at 32, name references at 2424, a float table at 2792, a
  `u16` table at 5528. Everything fits the block exactly (32 + 46 x 52 = 2424).
- **Bone entry, 52 bytes**: 9 `f32` matrix, 3 `f32` position, `u32` **parent index** (`65535` = none).
  AirElemental has one root, `VisualSceneNode`; Anakonda's hierarchy reads `Tongue_03` under
  `Tongue_02`, `Jaw` under `Jaw_root`, `Head` under `HeadRoot`. Row or column order of the matrix and
  the frame of the position are not verified. Computed as the difference of positions, two rest
  positions match two animation channels exactly (`skeletal-animation.md`).
- **Name reference, 8 bytes**: `u32` offset relative to the address of the reference, `u32` length including
  the terminating zero. Names sit in a pool after the tables, each padded to 4 bytes; bone `i` has name `i`
  (`Spine1`, `L_arm`, `L_forearm`, ... for AirElemental).
- Float table (pair 3): not decoded; may hold inverse bind matrices.
- `u16` table (pair 4): a permutation of `0..count` that also ends every animation clip of the model;
  its meaning is unknown.
- AirElemental has more blocks after the first `S` bytes (a count 3, then sizes 75472, 44912, 955, 27856,
  2841 and 1740); not decoded.

## Vertex skin data (2026-11-16, read by `Geometry::skin`)

The last 8 bytes of a vertex are four weights (`u8`) and four bone indices (`u8`), in that order:
stride 36 bytes 28..32 and 32..36, stride 32 bytes 24..28 and 28..32. Checked on all vertices of
AirElemental and Anakonda:

- a slot has a weight exactly when its index is not `0xFF` (0 mismatches in 2799 vertices);
- the weights add up to 255 or 256 (2416 and 383 of 2799 vertices in AirElemental);
- the stored index is **three times the bone number** (values 0, 3, ... 84, so bones 0..28 in
  AirElemental, below the 46 of the skeleton; the shader's register layout), and it indexes the
  bones in the order of the skeleton (see "Bind pose" below for the check: the vertices of a bone
  sit around that bone in the skeleton order, not in the order of the 46 `u16` table).

Other bytes of a 36-byte vertex: 20..23 is a normal as three unsigned bytes (x / 255 x 2 - 1, length
1 in all 2000 vertices tested) followed by `ff`; bytes 24..28 of a 36-byte vertex hold something
else (tangent?), not decoded. Stride 32: bytes 20..23 are the normal as well.

## Bind pose (2026-11-16, `Skeleton::world_pose` / `local_pose`)

**The bone entries hold the inverse bind transform**: with `M` the stored 3x3 matrix (read as nine
numbers in file order, row by row) and `p` the stored position, the bone sits at `-(M * p)` in model space
and `M` is its rotation in model space. Evidence:

- Vertices whose first weight is 200 or more, grouped by bone: the centre of each group is **0.55 units**
  from the bone computed this way in AirElemental (25 bones with 15 or more vertices; the mean distance
  to a random bone is 2.65) and **0.21** in Anakonda (random 3.05). The other readings tried (position as
  is, `-p`, `-M^T * p`, positions composed down the parents) all fit as badly as chance.
- The position of a bone relative to its parent, `parentRotation^T * (position - parentPosition)`, equals
  the first three parameters of an animation channel **exactly**: `Spine1` `(0.42, 0.009, -0.09)`,
  `R_forearm` `(-0.328, 0.006, -0.945)`, `Head` `(0, -1.529, 0.596)` (see `skeletal-animation.md`).
- Binary glTF written from this (bones as nodes with these local transforms, inverse bind matrices
  `[M^T | p]`, `JOINTS_0` and `WEIGHTS_0`) loads in Three.js and the bind pose reproduces every vertex
  to 1e-5 (AirElemental 2799 vertices, Anakonda 1290). That check is circular for the matrices
  (they are built to cancel) and only proves the file is consistent; the two bullets above are the real evidence.

## Матрицы костей с масштабом, модели 18.0 (verified на `Mirianna`, 2026-11-23)

В `Mirianna.(Geometry).bin` (клиент 18.0, 99 костей) у 89 костей строки сохранённой матрицы имеют длину 0,8333 (то есть `1/1,2`; определитель 0,5787), у остальных (`Slot_*`, `Global`, `transform*`, `VisualSceneNode`) длина 1. Положение кости в мировом пространстве получается как обратная к `[m^T | p]`: `[m / s² | -m·p / s²]`, где `s` — длина строки; у моделей 7.0 `s = 1`, формула совпадает с прежней. Локальный перенос равен `R_p^T (w - w_p) / a_p`, масштаб узла равен `a / a_p` (`a = 1/s`). Сверка: перенос локальной позы покоя совпал с первым кадром клипа `Mirianna.Idle` у костей `ClothBack1_6_R`, `Spine2`, `Breast_L`, `RightArm`, `Global` (до четырёх знаков), у `Twist_L` (длина строки 0,815, не 0,833) не совпал (0,5866 против 0,4866). До правки клип этой модели отвергался («клип не подходит к скелету»), теперь экспорт `.glb` с анимацией проходит для `Idle`.

## Blob list (2026-11-17, verified on 295 sample files)

Every geometry, animation clip and map `terrainDump` file in our samples (295 files) is an exact list of blobs, with ids 0, 1, 2 first in geometry files (`adv blobs <file>` prints the list; `adv_core::blob`). The inflated file is a plain list of blobs: `u32 localID`, `u32 size`, `size` bytes. In the
database record of the model (`Geometry`) the vertex buffer, index buffer and skeleton are
declared as blobs with `localID` 0, 1 and 2, which is exactly what the "unknown" words above
are. Later IDs hold further blobs (KD-tree geometry, portal and area fragments; the "count 3,
sizes ..." tail seen in AirElemental). The vertex declaration (stride, component types and
offsets), the sub-meshes (model elements with vertex and index ranges per LOD) and the
material to texture binding are **not in the `.bin`**; they are in the database record. See
[../TOOLS-STUDY.md](../TOOLS-STUDY.md).

## Запись Geometry в базе `pack.bin` (привязка материалов, draft)

Вершины, индексы и кости лежат в `.bin`, а элементы модели, материалы и текстуры описаны в записи Geometry в базе `pack.bin`. Подробные описания: [pack-bin-object-image.md](pack-bin-object-image.md) (образ данных, указатели), [pack-fixes.md](pack-fixes.md) (таблица Fixes), [common-material.md](common-material.md) (запись материала). Разделы ниже — журнал разбора по порядку; где они расходятся с этими тремя файлами, верны те файлы.

### Первые образцы и что не было расшифровано

Образцы: `World_Races_Folk`, `World_Races_Goblin`, `World_Races_Historian`, `Solo_Creatures_Turret.Mini` и `Solo_Creatures_AstralTurret_solo.Mini`. В паках рядом лежат `X.(Geometry).bin`, `X.(CollisionMesh).bin`, `X.(SkeletalAnimation).bin` и `Textures/*.(Texture).bin`; ни в одном `.bin` модели нет имён материалов или текстур (Turret: 3370 вершин, 7288 треугольников, 25 костей, хвост 3060 байт без строк).

В распакованном образе `pack.bin` (x64, 703 896 034 байта) есть записи, которые описывают элементы модели: в образе встречаются 7516 строк вида `…_psd_SG` (имя материала элемента, например `WaterFolk_House_Wall_psd_SG`), имена элементов (`Folk_House_02`, `Folk_House_01_Snow`, `Historian_Camp_Mattock_SG`) и рядом пары `u32` с диапазонами (начало, конец) вершин и индексов. Это совпадает с полями `ModelElement` в EditorTools V14 (`materialName`, `name`, `lods` с `indexBufferBegin/End`, `vertexBufferBegin/End`, `material.diffuseTexture`). Для `World_Races_Goblin` и `Solo_Creatures_Turret` строк в этой базе нет.

Не расшифровано: смещения полей в x64-образе (в EditorTools только 32-битные раскладки), связь записи Geometry с файлом `.bin` (имя файла в записи не найдено; кандидат: совпадение суммарных диапазонов элементов с числом вершин и индексов файла), указатель `diffuseTexture` на запись Texture и имя файла текстуры по ней (таблица имён этой базы содержит 519 корней). Статус: `unknown`.

### Объекты Geometry в образе данных (draft, 2026-11-23)

В `pack.bin` (x64) метки типов (`Database::type_tags`) дают 33 466 объектов `Geometry` (из 553 861 меток всех типов). Расстояние от метки до следующей метки у них от 648 до нескольких тысяч байт: например 648, 1320, 1424, 1608, 2000, 2120. Метка типа не обязательно покрывает все вложенные объекты (элемент `Folk_House_02` с именем и диапазонами лежит внутри области, помеченной как `Texture` размером 304 байта, то есть вложенные записи могут идти в области чужой метки). Голова Geometry в x64 содержит `f32`-значения (границы, масштабы) и пары равных `u64`-слов вида `(0x28, 0x28)`, `(0x140, 0x140)`: так, видимо, записаны массивы; смысл пар и привязка полей к `Geometry` из EditorTools V14 не подтверждены. Числа в записи элемента `Folk_House_02` (1050, 2046, 462, 960, …) не совпали с 508 вершинами и 881 треугольником файла `Folk_House_02.(Geometry).bin`, поэтому эта запись, вероятно, относится к другому файлу или поля читаются иначе. Статус: `unknown`.

### Geometry в 32-битной базе, первый разбор (draft, 2026-11-23)

Образец: `Solo/World/Props/Models/SA_Balloon_Chest_Default.(Geometry).xdb` в `pack.bin` клиента 1.x (Cloud Pirates, 32-битный образ, 64 739 692 байта в распакованном виде, данные с 7 999 148). Корень по `db_id` 37 496 768 даёт голову Geometry в 400 байт (это совпадает с `StructSize(400)` для `Geometry` в раскладке EditorTools V7_0_00_76): с +32 шесть `f32` границ (правдоподобный AABB: 0.056 … 2.9), с +352 и далее пары `(rel, rel)` у пустых массивов. Сразу за головой лежат вложенные записи с инлайн-строками: имена узлов (`Baloon01`, `Baloon02`), имя материала элемента `SA_Balloon_Chest_Default_psd_SG` (+1264 от корня), имя элемента `Baloon_Chest` и путь эффекта `ASS/FX/World/SA_Pirate_Balloon`. Строка лежит вплотную после своей структуры, а не по указателю из головы.

Не подтверждено: правило «цель указателя = адрес поля + значение» (в тех парах `(0x2c, 0x2c)`, `(0x70, 0x70)` цель попадает внутрь самой головы), границы записи `ModelElement` и положение `diffuseTexture`. Статус: `unknown`.

### Голова Geometry, проверено на двух моделях клиента 7.0 (2026-11-23)

Образец: `Bin/pack.bin` клиента 7.0 (338 145 304 байта, уже без zlib, 32-битный образ, данные с 63 554 240) и файлы `ArchdemonMage.(Geometry).bin`, `Warlock.(Geometry).bin` рядом. Корни `Creatures/ArchdemonMage/ArchdemonMage.(Geometry).xdb` (`db_id` 41 865 232) и `Warlock.(Geometry).xdb` (225 265 716). Голова 400 байт, поля по смещениям из раскладки EditorTools V7_0_00_76 (`StructSize(400)`):

| Смещение | Поле | ArchdemonMage | Warlock | Сверка с `.bin` |
|---|---|---|---|---|
| +184 / +188 | `indexBuffer` `localId` / `size` | 1 / 113 274 | 1 / 110 646 | размер = треугольники × 3 × 2 (18 879 и 18 441) |
| +356 / +360 | `vertexBuffer` `localId` / `size` | 0 / 281 088 | 0 / 254 144 | размер = вершины × stride (7808 × 36 и 7942 × 32) |
| +196, +200 | `joints`, байты | 2256 | 2256 | 94 кости × 24 |
| +232, +236 | `modelElements`, байты | 1080 | 240 | 9 и 2 элемента × 120 |
| +368, +372 | `vertexDeclarations`, байты | 116 | 116 | 1 × 116 |

Массив здесь занимает 16 байт: `u32 0`, байты, байты, `u32 0`. Вместо числа элементов записан размер в байтах (делится на размер элемента), а первое слово 0 — это указатель на начало, который подставляет таблица Fixes. Вывод по `stride` и числу вершин в `.bin` не нужен: он лежит в `vertexDeclarations`. Статус: `verified` для этих полей на двух моделях; `unknown` для содержимого массивов (элементы, материалы, текстуры): нужно разобрать таблицу Fixes (чанк 4, записи по 8 байт; примеры пар: `0x26,0`, `0x51,72`, `0x35e,0`, `0x389,484`; низшие три бита первого слова принимают значения 0, 1, 5, 6, 7), чтобы узнать, куда указывает пустое начало массива.

### Первый просмотр записи Geometry в 18.0 и расположение массивов (draft, 2026-11-23)

Модель `Mirianna` (клиент 18.0: `Mirianna.(Geometry).bin`, 4465 вершин со stride 32, 10 058 треугольников, 99 костей, блок после индексов 12 180 байт). В распакованном `pack.bin` 18.0 (723 056 098 байт) числа из файла находятся рядом: размер буфера индексов 60 348 (`10 058 × 3 × 2`), в 304 байтах дальше размер буфера вершин 142 880 (`4465 × 32`), между ними 12 180; имя материала `Mirianna_SG` лежит в 6 КиБ от головы (121 610 612). Массивы записаны тройками `u64` (начало 0, конец, ёмкость): у костей 3960 = 99 × 40, у объявлений вершин 116; ещё два массива с размерами 1152 и 704 не сопоставлены. Значит в x64 запись кости 40 байт (в 7.0 — 24).

В 7.0 для `ArchdemonMage.(Geometry).xdb` расстояние до следующей метки 3852 байта равно `400 + 1080 + 2256 + 116`: голова, элементы, кости и объявления вершин лежат подряд сразу за головой, значит начало каждого массива (слово 0) указывает на конец предыдущего. За головой с +400 идут записи, которые выглядят как `ModelElement` (в начале шесть `f32` границ, потом `3`, `238700` и повторы по 28 байт с `65535`); границы полей `ModelElement` и положение материала не подтверждены, имён материалов среди инлайн-строк 7.0 нет (материал, видимо, ссылка на другой объект). Статус: `unknown`.

### ModelElement в образе 7.0, записи `Warlock.(Geometry)` (draft, 2026-11-23)

Корень `Creatures/ArchdemonMage/Warlock.(Geometry).xdb` занимает 3176 байт (до следующего корня `Warlock.(Texture).xdb`). Массивы и строки записаны тройкой `u32` (начало, размер в байтах, ёмкость): у непустого массива начало 0 (его подставляет таблица Fixes), у пустого все три слова равны одному «живому» адресу сборочной машины (`0x3a13c918`). Строка — такая же тройка: `(0, длина, ёмкость)`, текст лежит вне структуры. Так читаются `ModelElement` из раскладки EditorTools V7:

| Смещение в элементе | Поле | Элемент 0 | Элемент 1 |
|---|---|---|---|
| +4 | `linkedElements` (тройка) | пусто | пусто |
| +20 | `lods` (тройка) | 0, 60, 60 → один `Lod` в 60 байт | то же |
| +36…+76 | `material` (40 байт) | в +48 хвост `\0xdb` (путь на `.xdb`), остальное нули | нули |
| +76 | `materialName` (тройка) | длина 10, ёмкость 17 | длина 10, ёмкость 17 |
| +88 | `name` (тройка) | длина 9, ёмкость 17 | длина 11, ёмкость 17 |
| +100…+112 | `skinIndex`, `vertexBufferOffset`, `vertexDeclarationID`, `virtualOffset` | 0, 0, 0, 0 | 0, 0, 0, 0 |

Элементы идут подряд по 120 байт, расстояние между векторами `lods` у элемента 0 (+2896) и элемента 1 (+3016) равно 120. Массив начинается со смещения +2876 от корня, после тексты имён костей (94 кости × 24 байта с +400; длинные имена `SkirtFront4_L`, `StaffChainA1` лежат с +2655). Раскладка внутри корня не равна порядку полей головы: массивы идут в порядке обхода, а не по полям, поэтому границу массива находят по соседним записям, а не по сумме размеров.

Не расшифровано: где лежат тексты `materialName` и `name` (в записи их нет, начало равно 0), как устроен `Lod` (60 байт), какой путь хранится в `ResourcePointer diffuseTexture` (виден только хвост `xdb`). Статус: `draft` для смещений `ModelElement` (совпали с V7 на двух элементах), `unknown` для текстов.

### Объявленный stride (2026-11-23)

`geometry::declared_stride(xdb)` читает `<stride>` первого блока `<vertexDeclarations>` описания Geometry в `.xdb` (допустимо от 16 до 256 байт). Это вспомогательная функция с тремя тестами; CLI пока не передаёт описание, поэтому stride по-прежнему задаётся `--stride`. Статус: `draft`; разбор образа объекта не закончен.
