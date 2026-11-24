# Sound to object binding (resource database)

Status: `draft`. `CharacterSoundVariation` is partly decoded (client 1.1, 32-bit image); the other
record types are `unknown`.

## Finding (2026-11-24)

Sounds are tied to game objects through records of the resource database, not through the `.fev`
event data. The debug symbols of the client engine (`Sound.pdb`, `SoundExport.pdb`, in the
`ao-helper-rv168362` repository, client 1.1 revision 168362) show that `SoundProject` is built from
an `NDb::FMODProject` plus an FMOD event project, and sound events are created by name from it.

Struct types present in the `pack.bin` of both 7.0 (`Bits32`, 1 278 structs, 397 765 files) and
18.0 (`V2`): `FMODProject`, `Sound3DAction`, `Sound2DAction`, `CharacterSoundVariation`,
`AliasSound3d`, `SoundParameter`, `Sound3DActionDevice`, `SoundRoot`, `UIRelatedSounds`,
`UIWidgetSound`, `CreatureStepSoundParamAction`; 18.0 also has `AttachedSoundComponent`.

In the 7.0 database 73 resources are of these types, for example
`SFX/Creatures/Creatures_L.(FMODProject).xdb`, `SFX/Voice/VoiceElf.(FMODProject).xdb`,
`Characters/Elf_male/ElfMale.(CharacterSoundVariation).xdb`. A `FMODProject` is a project of
`.fev`/`.fsb` files in `SFX_*` paks; a `CharacterSoundVariation` belongs to a character model.

## CharacterSoundVariation (checked on client 1.1.00.41, `ElfMale`)

Root record (offsets from the root, 32-bit image, pointers resolved with the pointer-fix table):

- +64: reference (fix tag 0) to a `FMODProject` resource, here `SFX/Voice/Voice.(FMODProject).xdb`.
- +24 / +28 / +32: array pointer (tag 1), byte size, capacity; 896 bytes = 28 entries of 32 bytes.
- +40 / +44 / +48: second array, 1 224 bytes.
- Entry of 32 bytes: +8 reference to a `FMODProject`, +16 string `(pointer, length, capacity)` with
  the FMOD event path, +28 a small number (event kind or index, meaning not known).
- The text of a string is read at `pointer` in the data block, `length` bytes, for example
  `PC/ELfMale/Animation/ElfMale_attack_Universal` (length 45, capacity 65).

The event names match the event names inside `Voice.bev` (`SFX/Voice/Voice.bev` in `BaseLoceng.pak`):
`ElfMale_attack_Universal` occurs 13 times there, `ElfMale_attack_Pierce` 7 times. The event path in
the record uses the category `PC/ELfMale/Animation/` (case differs from the `.bev`, compare
case-insensitively).

Resource names seen in 1.1: 11 `CharacterSoundVariation` (Elf, Hadagan, Kania, Undead/Zem, Orc,
`NoSound`), 14 `FMODProject` (Creatures, Music, AmbientsOutdoor, Ships, World, AmbientsIndoor, ...),
37 `AliasSound3d` (`Client/CreatureSoundAliases/*`).

## FMODProject (checked on 1.1, `Voice` and `Footstep`)

Root record, offsets from the root:

- +32 string `(pointer, length, capacity)`: path of the event project, `SFX/Voice/Voice.bev`,
  `SFX/Foley/Footstep.bev`.
- +44 array pointer, +48 byte size, +52 capacity: one `u32` per bank (180 bytes = 45 values in
  `Voice`), values look like hashes or checksums, meaning not known.
- +60 array pointer, +64 byte size: 12-byte string triples `(pointer, length, capacity)`, the paths of
  the sound banks, 540 bytes = 45 entries, e.g. `SFX/Voice/Voice_ElfFemale_Animation.bsb`.

So an event path from a `CharacterSoundVariation` entry is looked up in the `.bev` named by its
`FMODProject`, and the sample data is in the banks listed there.

## Who points at sound projects

- `Voice.(FMODProject)` is referenced by the `CharacterSoundVariation` records (9 times at +128 in
  `ElfFemale` alone, further entries at +1024…+1280).
- `SFX/Creatures/Creatures.(FMODProject)` is referenced from effect records, for example
  `Spells/FX/Mobs/GhostSmoke.(ParticleAnimation)` (offsets +448, +500, … at a 52-byte step): the
  sounds of creatures are played by effects, not by a character record.

## Link from the character (checked on 1.1, `ElfMale`)

`Characters/Elf_male/ElfMale.(VisCharacterTemplate).xdb` is the only record that points at
`ElfMale.(CharacterSoundVariation).xdb` (reference at +472). The same template holds the other
links of a character (references, fix tag 0, offsets from the template root): +120
`(CharacterVariations)`, +332 `SFX/Foley/Footstep.(FMODProject)`, +360 `(MobAnimations)`, +384 the
skin `(Texture)`, +392/+376/+320/+132 `(AnimationController)`, +352 `(ModelMorphSettings)`, +404/+412
hit `(EffectsList)`, +480 `(VisualItem)`, +488 `(VisObjectTemplate)`. So model, skin, animations and
sound of a character are reached from one `VisCharacterTemplate`.

The command `adv db sound` prints these records. The number at +28 of an entry looks like an animation id (4, 5, 19, 390, ...).

## Not known

- The meaning of the second array and of the number at +28 of an entry.
- Layouts of `Sound3DAction`, `AliasSound3d`; the 52-byte sound entry inside `ParticleAnimation`.
- The meaning of the `u32` array of `FMODProject`.
- How creatures (not player characters) point at sounds, and the `AttachedSoundComponent` of 18.0.
- Whether 7.0 and 18.0 use the same layout.

## Next step

Decode `FMODProject`, `Sound3DAction` and the animation link the way the Geometry record was decoded
(`docs/formats/model-binding.md`): take a record, list its pointer fixes, follow the targets, and
check the result against the `.fev`/`.bev` event names of the same project.
