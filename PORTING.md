# Rust port of Minecraft.World — plan & log

## Port status per topic (estimates, updated with every change)

Percent = share of the Windows64 behaviour of that C++ area that exists in Rust and is tested. They are honest estimates
by function / feature count, not line counts. Last update: infinite world, chunk workers, FPS limit.

| Topic (C++ area) | Done | Left / notes |
|---|---|---|
| Core: Random, math, NBT, compression, BoundingBox | 98% | – |
| Storage: region files, save format, chunk IO | 90% | console `Save.ms` read; own per-chunk store for play |
| Terrain generation (overworld, nether, end, flat, biomes, caves, ores) | 95% | – |
| Structures (village, stronghold, mineshaft, temples, nether fortress) | 90% | some piece variants |
| Level: lighting, ticks, fluids, falling blocks, weather | 85% | – |
| Blocks (Tile classes) | 75% | furnace/brewing/enchant table/anvil/cauldron/beacon behaviour, many plants |
| Redstone (dust, torch, lever, button, repeater, piston, TNT) | 80% | comparator, hopper, dispenser/dropper, rails |
| Items (tools, food, armor, pearls) | 55% | potions, enchanting, bows/arrows, fishing, buckets of mobs, maps |
| Inventory, crafting, containers | 60% | chest (27/54) + crafting table done; furnace, brewing, enchant, anvil, trading missing |
| Survival (hunger, XP, fall damage, death, respawn) | 70% | status effects/potions, armor damage reduction |
| Mobs / entities | 27% | 11 of ~40 mob kinds (pig, cow, sheep, chicken, zombie, skeleton, creeper, spider, cave spider, slime, lava slime); no endermen, villagers, wolves, ghasts, blazes, horses, bosses |
| Player model, hand, animations | 70% | skins, third person |
| `GameRenderer` (see below) | 60% by function (about 85% of what a single player can see) | – |
| `LevelRenderer` / chunk meshing / particles / sky / clouds | 85% | |
| Chunk streaming, infinite world, workers | 90% | server-side chunk cache semantics are simplified |
| GUI screens (title, create, pause, options, inventory, chest, sign, death) | 50% | help/controls, furnace, brewing, enchant, anvil, trading, leaderboards |
| Options (render distance, FOV, sensitivity, volume, bobbing, clouds, **Max Framerate, VSync**) | 55% | gamma, difficulty, controls, languages |
| Audio | 60% | |
| Multiplayer / splitscreen / DLC / achievements / platform services | 0% | out of scope for a single-player Windows port |
| **Overall (Windows64 single-player scope)** | **about 55%** | |

### `GameRenderer::` function by function (41 functions)
Done (17): `tick`, `pick`, `getFov`, `bobHurt`, `bobView`, `moveCameraToPlayer` (first person), `getFovAndAspect`, `setupCamera`,
`renderItemInHand`, `updateLightTexture` / `getLightTexture`, `render`, `renderLevel`, `prepareAndRenderClouds`, `tickRain`,
`renderSnowAndRain`, `setupGuiScreen`, `setupClearColor`, `getFpsCap` (now a real setting).
Partly (5): `tickFov` / `SetFovVal` / `GetFovVal` (no sprint/bow zoom), `setupFog` (no blindness/lava fog edge cases),
`getNightVisionScale` (no potions yet), `runUpdate` / `updateAllChunks` / `EnableUpdateThread` (the mesh thread pool replaces them).
Not done (19): split-screen (`ComputeViewportForPlayer`, `BuildPlayerViewports`), gamma (`ComputeGammaFromSlider`,
`CachePlayerGammas`, `ApplyGammaPostProcess`), `zoomRegion` / `unZoomRegion`, `turnOn/OffLightLayer` (the light texture is a
shader input here), the `AddForDelete` / `FinishedReassigning` GL-resource bookkeeping (wgpu frees buffers itself).

### Shaders
The Windows64 C++ build draws with the fixed-function pipeline plus its own `.hlsl` post effects, so `shader.wgsl` already matches it.
Java-edition style shader packs (GLSL, deferred shadows, water, bloom) are a different thing: wgpu can do all of it, but it is new
design work, not a port, and is not started.

Approach: bottom-up, one layer at a time. Every layer ships with tests; lower
layers never depend on higher ones. Closed-source middleware is replaced, not
wrapped (see "Replacements").

## Scope

**Windows 64 build only** (`_WINDOWS64` + `_LARGE_WORLDS`, as set in CMakeLists.txt). Code that exists only
for X360 / PS3 / PS4 / Vita / Xbox One, and non-large-world fallbacks, is out of scope and is not ported.
Already-written layer-2 code that mentions other platforms is left in place but will not be extended.

## Layers (by dependency order)

| # | Layer | C++ source (Minecraft.World) | Rust crate | Status |
|---|-------|------------------------------|-----------|--------|
| 0 | Math / RNG / positions / Java-style IO | Random, Mth, Pos, ChunkPos, Vec3, AABB, Data{Input,Output}Stream | `mc-core` | **done** |
| 1 | NBT | Tag, *Tag.h, CompoundTag, ListTag, NbtIo | `mc-nbt` | **done** |
| 2 | Save container & regions | RegionFile(+Cache), ConsoleSaveFile(Original), FileHeader, compression (RLE+zlib) | `mc-storage` | **done** |
| 2b | Chunk/level storage | McRegion*, ZonedChunkStorage, DirectoryLevelStorage, LevelData, ChunkStorage | `mc-level`/`mc-worldgen` | **chunk write + level.dat + save container done** (`chunk_save.rs`, `save_world.rs`, `mcsave`); lighting is not recomputed before saving, entities/players not saved |
| 3 | Core registries | Material, Direction/Facing, Tile (block) table, Biome | `mc-world` | **done** (Item base deferred to layer 6) |
| 4 | World generation | Noise, Layers, BiomeSource, RandomLevelSource, caves/canyons | `mc-worldgen` | **terrain + decoration + postProcess done** (lakes, lava, monster rooms, biome decorator, ice/snow); structures (village/mineshaft/stronghold/temples), **Nether (terrain, caves, features, fortress), End (terrain, spikes, podium) and Flat done** |
| 5 | Level simulation | Level, LevelChunk, lighting, setTile, liquids/plants/sand behaviour, minimal tile entities | `mc-level` | **partial** (no entities, pending ticks never run, no fire) |
| 6 | Tiles & items | ~180 Tile classes, ~150 Item/Recipe/Enchant classes | `mc-world` | |
| 7 | Entities & AI | Entity, Mob, Goals, Navigation, ~125 classes | `mc-entity` | |
| 8 | Network | 47 Packet classes, Connection | `mc-net` | |
| 9 | Server | MinecraftServer, PlayerList, ... (in Minecraft.Client dir) | `mc-server` | |
| 10 | Client | rendering, GUI, input, audio | `mc-client` | **started**: world viewer `mcview` (wgpu/winit, real `terrain.png` atlas, chunk mesher with AO/tints/fog, free-fly camera); no GUI/audio/player yet |

## Replacements for closed-source / platform parts

| Original | Rust replacement (plan) |
|----------|-------------------------|
| Iggy (Flash UI) | egui, or a custom immediate-mode UI; screens rebuilt by hand |
| 4J_Render (D3D11) | `wgpu` + `winit` |
| 4J_Input / XInput | `winit` events + `gilrs` for gamepads |
| 4J_Storage | plain `std::fs` + the save format from layer 2 |
| Miles Sound System (`.msscmp`) | `rodio`/`cpal`; assets must be extracted to ogg/wav |
| Sentient / Xbox LIVE / PSN | dropped |
| WinSock, TLS/thread APIs | `std::net`, `std::thread`, `parking_lot` |

## Conventions

* Names: `snake_case`; overloads become `_by`, `_xyz`, `_f32/_f64` suffixes.
* The 4J object pools (`Vec3::newTemp`, `AABB::newTemp`) are gone — value types are `Copy`.
* Raw owning pointers / `shared_ptr` → ownership, or `Rc/Arc` / arena indices where truly shared.
* `float` vs `double` widths are preserved where they affect determinism (world-gen!).
* Reading untrusted data returns `Result`; the C++ silently continued.

## Behavioural notes found while porting layer 0/1

* `Random` is bit-compatible with `java.util.Random` (checked against known Java outputs).
* `Mth::sin/cos`: 65536-entry f32 table; the table is built with `f64::sin` of an f32 argument.
  If the MSVC build actually used `sinf`, a few entries could differ by 1 ulp — verify against
  a dump from the original binary before relying on bit-exact physics.
* `Pos::west(steps)` upstream ignores `steps` (bug) — the port honours it.
* `Mth::getInt/getDouble` upstream ignore `def` on parse failure — the port returns `def`.
* `NbtIo` "compressed" functions do not compress in this tree (GZIP calls removed).
* NBT read limits (depth 256, 32768 tags, list ≤ 512, byte array ≤ 2 MiB, int array ≤ 65536) are kept
  in `NbtLimits`; exceeding them is an error here but was a silent truncation upstream.
* UTF strings > 32767 bytes are rejected on read (console limit).
* Mixed-type lists are rejected by `ListTag::push` (upstream stores whatever you add but writes the first element's id).
* `ListTag` equality is order-insensitive "each element has a match" (kept from upstream).

## Layer 2 notes (mc-storage)

* A world is ONE blob: `[u32 table_offset][u32 count][i16 orig_ver][i16 ver]` + file data + file table
  (144-byte entries, UTF-16 names). On disk it is wrapped as `[0i32][u32 len][zlib(blob)]`.
* Region (`.mcr`) chunks are `u32 len|0x80000000, u32 decomp_len, zlib(RLE(data))`. It is NOT Java's
  5-byte-header format. Win64/XB1/PS4/Vita use zlib; **X360 (LZX) and PS3 (EdgeZLib) saves are rejected**
  with `UnsupportedCompression`.
* Verified only against hand-built data and round trips — no real Windows save was available.
  Please test with a real `saveData.ms` before trusting it.
* `Mth`-style quirks kept: a freshly opened empty region does not touch the save until the first write;
  sectors are only appended in whole-sector units but the file may be unaligned until reopened (padded then).
* Differences: chunk > 255 sectors is an error (upstream silently drops it); RLE overflow is an error
  (upstream returns uninitialised tail); big-endian (X360/PS3) headers parse, but writing is always little-endian.
* Not yet ported from this layer: `ConsoleSaveFileSplit`, `ConvertRegionFile`/`ConvertToLocalPlatform`,
  thumbnail/metadata handling in `Flush`, XUID/player `.dat` filename helpers (console-only).

## Layer 3 notes (mc-world)

* `Material` is an enum + `const` flag table written in the same builder order as `Material::staticCtor`
  (33 materials; Gas/Liquid/Decoration/Portal/Web subclass overrides folded into a `Kind`).
  `MaterialColor` keeps only the id — real colours come from the texture pack's colour table.
* `tile_table.rs` is **generated** from `Tile.cpp`'s `staticCtor` (id, icon name, hardness, blast
  resistance replaying the setter chain, light block/emission, and material where the registration
  names it). 164 ids. Materials chosen inside subclass constructors are `None` until layer 6.
  The four slab ids (43/44/125/126) are hand-added with approximate hardness.
* `Biome` is one data struct (23 biomes) built with `const fn` setters mirroring the fluent C++;
  surface blocks, depth/scale, temperature/downfall and `BiomeDecorator` counts are kept.
  Mob lists and colour-table lookups are not (entity layer / client resources).
* `GEN_DEPTH` is 128 (legacy world height); `SEA_LEVEL` is 63; `MAX_BUILD_HEIGHT` is 256.

## Layer 4 notes (mc-worldgen)

* `ImprovedNoise::add` reuses the corner gradients while the integer y cell is unchanged (a Java quirk
  the console kept). It is reproduced and pinned by a test — do not "fix" it, or every world changes.
* `Layer` family: pure functions of (seed, x, z). Tested by checking that any window equals the
  same window cut from a larger area (exercises all zoom/voronoi offsets incl. negative coordinates).
  The island chain shared by the river and biome branches is cloned instead of `shared_ptr`-shared.
  Not ported: SmoothZoom/Temperature/Downfall/Flat/BiomeOverride (unused by `getDefaultLayers` here).
* Windows builds define `_LARGE_WORLDS`, so the *complicated* edge fall-off (`getMinDistanceToEdge` +
  moat flags) is used; world sizes 54/64/192/320 chunks are supported through `WorldParams::xz_size`.
* `BiomeSource::find_seed` (4J "good seed" search) is ported with an injectable RNG; ~1/40 random seeds pass.
* Caves/canyons use `Mth::sin/cos` (table) and keep `float` intermediates. **Unverified assumption:** in
  `addTunnel(random.nextLong(), …, random.nextFloat()*0.5f+0.5f, …)` the original (C++) argument
  evaluation order is unspecified; Java/left-to-right is assumed (`SPLIT_ARGS_LEFT_TO_RIGHT`). Only
  branching cave tunnels would change if MSVC evaluated right-to-left.
* Not yet ported: `StructureFeature` family (village, mineshaft, stronghold, desert temple/jungle
  temple/witch hut), `postProcess` (lakes, dungeons, ore, trees, flowers, snow/ice), `FlatLevelSource`,
  `HellRandomLevelSource`, `TheEndLevelRandomLevelSource`, `SimplexNoise`/`PerlinSimplexNoise`
  (used by the Nether/End and decoration). `lightChunk`/`LevelChunk` come with layer 5.
* **Nothing here could be compared with the original binary.** Confidence comes from: Java-compatible
  `Random`, literal transcription incl. evaluation order and float widths, self-consistency tests, and
  plausible-looking maps (`samples/`). To verify: generate a world with the real game from a known seed and
  compare chunk block arrays against `mcgen --dump-chunks`.

### Try it

```
cargo run --release -p mc-worldgen --bin mcgen -- --seed 12345 --out world-out
cargo run --release -p mc-worldgen --bin mcgen -- --find-seed          # like a new console world
cargo run --release -p mc-worldgen --bin mcgen -- --seed Hello --size large --radius 60
```
writes `biomes.png`, `terrain.png`, `info.txt` (and raw chunks with `--dump-chunks`).

## Legal

The source is proprietary (Mojang / Microsoft / 4J Studios). This port is for personal study;
do not redistribute it or any game assets.

## Validation against a real save (Save.ms, seed 666, Large world 320)

Tools: `mc-storage/examples/{savedump,chunkdump,scan}.rs`, `mc-worldgen/src/bin/{mccompare,mcshape,mcpost,mcseq,mcconf,mcbiomes,mccol,mchist}.rs`.

* `mc-storage` opens the real Win64 save (version 9): all 649 overworld chunks parse; the region files are
  **16×16-chunk (split) regions**, chunk records use the compressed-tile / sparse-light binary format
  (`mc-level::chunk_io`), not NBT.
* The save was **not produced by the TU19 source in this repo**: it contains biome ids 23, 24, 129–134, 155, 157
  that do not exist in `Biome.cpp`, so biome maps (and anything biome dependent) cannot match.
* Where the biome of a chunk is identical, generated terrain matches the save block for block except for
  decoration/caves-of-structures (2 of the 10 such chunks are identical below y=40, most others differ in <2.5 % of cells).
* Decoration statistics match (dirt 14529 vs 14760, coal 2485 vs 2582, diamond 84 vs 75 cells in the sample) but
  vein *positions* do not, i.e. the per-chunk random sequence differs from the real game (different build).

## Structures status (Overworld)

Ported from the source (files were on the PC, the first upload was only a subset):
StructureFeature framework, MineShaft, Stronghold, Village (all 11 pieces + roads, `has_village` is real),
RandomScatteredLargeFeature (desert pyramid, jungle temple, swamp hut). The `postProcess` bodies of Village and Scattered
pieces are transpiled from C++ by `tools/gen_village.py` / `tools/gen_scattered.py` (`tools/cpp2rs.py`).

Approximations: entities (villagers, witch) are only recorded (`Level::spawned_mobs`), pending ticks never run, the level `random`
handed to the scattered features is a per-chunk seeded stand-in, C++ argument evaluation order is assumed left-to-right.

Nether: `nether.rs` (HellRandomLevelSource, hell caves, fire/spring/glowstone features) + `structure/nether_bridge*.rs` (fortress, 13 pieces
transpiled by `tools/gen_nether.py`). End: `end.rs` (TheEndLevelRandomLevelSource, TheEndBiomeDecorator, SpikeFeature, EndPodiumFeature;
dragon/crystals only recorded in `Level::spawned_mobs`). Flat: `flat.rs` (bedrock/dirt/grass + village).
`MobSpawner::postProcessSpawnMobs` is `#if 0` in the source (no-op), so nothing to port.

Remaining approximations: entities are recorded not simulated, pending ticks never run, chunk post-process order is a deterministic
x-major stand-in, the level `random` used by some features is a per-chunk seeded stand-in, tile entities are not saved to NBT yet.
Unused so far: `SimplexNoise`/`PerlinSimplexNoise` (not referenced by any generator in this build).

**World generation: all generators ported.** Next layer: entities / tile ticking / saving.

Tool: `mcdim --dim nether|end|flat [--seed N] [--radius CHUNKS] [--out DIR]` writes top.png, side.png and block stats for the non-overworld generators.

## Running the viewer (Windows, from `mc-rs/`)

```
cargo run --release -p mc-client --bin mcview -- --dim overworld --seed 666 --radius 10
cargo run --release -p mc-client --bin mcview -- --dim nether
cargo run --release -p mc-client --bin mcview -- --save save.ms --dim overworld   # a file from mcsave
```
`terrain.png` is found automatically at `../Minecraft.Client/Common/res/TitleUpdate/res/terrain.png` (the game's own file; not copied
into this repo), or pass `--assets PATH`. Click = capture mouse, WASD/Space/Shift = fly, Ctrl = sprint, wheel = speed, Esc = release/quit.
`--screenshot out.png [--cam X,Y,Z --yaw DEG --pitch DEG]` renders one frame offscreen (works with software GL).
Not drawn yet: doors, signs, levers, buttons, repeaters, stairs shapes (drawn as cubes), entities, light levels (AO + face shading only).

## Play mode (mcview)
`mcview` now runs a fixed 20 TPS game loop: `physics::Player` (faithful Entity::move/LivingEntity::travel port: gravity, friction, step-up, sneak edge guard, swimming, flying), mouse look, `game::Game` (DDA raycast reach 5, break/place with player-overlap check, bedrock unbreakable, dirty-chunk remesh), selection outline and HUD (crosshair + 9-slot hotbar). Keys: WASD, Space jump (double-tap/F fly), Shift sneak, Ctrl sprint, LMB break, RMB place, 1-9/wheel hotbar.

## Lighting, sky, tints (mc-client `sky.rs`, `skyrender.rs`, `mesh.rs`, `shader.wgsl`)
Smooth lighting + AO per face corner (TileRenderer), 16×16 light texture (`light_texture`, sky/block), biome tints from `colours.xml`,
sky dome / sun / moon phases / stars / dawn fan / fancy clouds / fog (`GameRenderer::setupFog`), day-night cycle. Non-sRGB surface and
textures on purpose (fixed-function GL blends in gamma space). Keys in `mcview`: T/Y = ±1000 ticks, P = freeze the day cycle; `--time N`.
Known gaps: no rain/thunder, `fogBr` fixed at the default view distance, `light_all()` instead of light-while-placing.

## Mining, particles, sound (mc-client `game.rs`, `particles.rs`, `audio.rs`, `soundtypes.rs`)
* **Mining** = `MultiPlayerGameMode` / `LocalPlayer::handleMouseDown` / `Minecraft::handleMouseClick(0)`: progress per tick is
  `Tile::getDestroyProgress` (`1/hardness/30` when the material is always-destroyable or a tool applies, else `/100`; ÷5 with the head in
  water), the hit sound every 4th tick (`(vol+1)/8`, pitch×0.5), crack stage `int(progress*10)-1` drawn with `destroy_N` and the
  `(DST_COLOR, SRC_COLOR)` blend, 5-tick delay after a break, `missTime` 10 after swinging at air (survival). Creative (`Game::creative`,
  default; `mcview --survival` switches) breaks one block every 5 ticks while held. No items/tools exist yet, so the hand is the only tool.
* **Particles** = `ParticleEngine` / `Particle` / `TerrainParticle`: 4×4×4 pieces on break, one piece per tick while hitting, `Entity::move`
  physics, ≤200 alive, camera-facing quads using a quarter of the block's bottom icon, tinted `0.6×getColor`, lit by `getLightColor`.
  Other particle types (smoke, flame, …) are not ported yet.
* **Sound**: the sounds are plain `.ogg` under `Windows64Media/Sound/Minecraft/**` (no XACT needed). `audio::SoundBank` resolves a name like
  `dig.stone` exactly as `SoundEngine::play` does (`dig/stone.ogg`, else a random `stone1..N.ogg`); `Mixer` does pitch resampling, volume
  clamped to 1, miniaudio's default inverse-distance attenuation and left/right balance from the listener; `AudioEngine` plays through
  `cpal` (feature `audio`, on by default; `--no-default-features` builds without an audio device, e.g. on Linux without ALSA headers).
  Per-block sounds: `soundtypes::sound_type(id)` (generated from `Tile.cpp`). Footsteps follow `Entity::move` (`moveDist`/`nextStep`,
  volume `0.15×`, silent when sneaking on the ground or flying), placing plays the place sound at `(vol+1)/2`, pitch×0.8.
  Not yet: music streaming, UI sounds, mob sounds (no mobs), ambient cave/rain.
* **Chunk streaming** (`stream.rs`) = `ServerChunkCache`/`MultiPlayerChunkCache`: `Streamer::update` runs every tick with an 8 ms budget around the
  player's chunk: terrain (from the save folder, else the dimension's generator) → decoration once the `+x/+z/+x+z` neighbours exist →
  light (`Level::light_chunk`) + mesh once every decoration that can touch the chunk is done. Chunks farther than `render_distance + 3` are
  written to the save (decorated ones) and dropped together with their mesh. Bounded by the 54-chunk world (Nether/End by their own size).
* **Persistence** (`save.rs`): `options.txt` (render distance, FOV, sensitivity, sound, music, bobbing, clouds) and `saves/<id>/level.txt`
  (name, seed, dimension, mode, times, position, health/food, flying, spawn, inventory) + `chunks/<dim>_<cx>_<cz>.dat` (flag byte + zlib of
  `chunk_save::write_chunk`). Location: `MCRS_DATA`, else `%APPDATA%\mc-rs`, else `~/.mc-rs`. Autosave every 60 s (edited chunks only), full
  save when quitting to the title or closing the window. The title's Singleplayer opens the world list (`ScreenId::LoadWorld`).
  Not saved yet: mobs, dropped items, tile-entity contents. `mcview --no-save` / screenshots never touch the disk.
* **Creative inventory** has the five category tabs (`CREATIVE_TABS`), a scrollbar and the hotbar row; E opens crafting, R the creative/survival inventory.

## Weather, special-shaped blocks (latest)
- `weather.rs`: `Level::tickWeather` rain / thunder timers, `GameRenderer::renderSnowAndRain` precipitation quads (repeat sampler), lightning bolts. `mcview --weather rain|thunder`, key U cycles. Not saved in `level.txt` yet.
- `meshpool.rs`: chunk meshing on worker threads (versioned jobs on 3x3 snapshots, 6 uploads per frame).
- Chest GUI (`Chest` screen, tile-entity contents saved with the chunk), creative search box + tabs, world name field, world delete with confirmation, mobs / dropped items saved per dimension (`entities_<dim>.txt`).
- `shapes.rs` + `Kind::Shaped`: stairs (two boxes; no corner merging yet), doors (both halves hold dir/open bits, bit 8 = upper half), trapdoors, fence gates, signs (board only, no text), levers, buttons (pop back after 20/30 ticks), pressure plates, repeaters, cake. Per-part meshing, multi-box collision (`shapes::collision`), union selection box. Placement data follows `facing4()` / clicked face; right click toggles doors / trapdoors / gates / levers / buttons; supports knocked off by `break_dependents`. `mcview --showcase` places a row of them for screenshots.
- Not done: stair corners, door hinge flip, redstone power, double chests, flower pot, bed halves, sign text.

## Shapes / redstone / falling blocks / orientation
- `shapes.rs`: multi-box blocks (stairs with corner joining, fence arms, gate, anvil, lever tilt, repeater+torches, plates, buttons, cake); neighbour-aware parts/collision/selection.
- `redstone.rs`: Java 1.6 style power model (dust strong/weak power, repeaters 2 ticks/level, torches, levers, buttons, plates, lamps, doors).
- Falling blocks: `FallingBlock` entity; `Level.fall_requests` (sand, gravel, anvil).
- Placement orientation: logs, pistons, furnace/dispenser, pumpkin, anvil, stairs, slabs (merge), levers etc. via `Game::oriented`.
- Grass-side atlas fringe fixed in `render.rs::fix_grass_side`.
- `mcview --showcase2` renders the test scene (camera: `--yaw 180 --pitch -38`).

## Pistons, TNT, chests, doors, signs (latest)
- Pistons (`piston.rs`): port of `PistonBaseTile` (push/pull chain of up to 12 blocks, sticky pull, push reactions, extended bit 8).
  `PistonMovingPiece` / `PistonPieceEntity` are not in the source: rebuilt from Java 1.6 (0.5 progress per tick, entities pushed).
  Moving pieces are drawn by `entrender::piston_layers`; breaking a head/base removes its partner.
- TNT (`tnt.rs`): `TntTile` + `PrimedTnt` (fuse 80, chain fuse 10-30, radius 4), lit by flint & steel, redstone, fire and other explosions.
  Swelling / flashing in `entrender::primed_layers`.
- Chests (`chest.rs`): boxed model, lid swings 0.1/tick (`ChestAnim`); the chunk mesh leaves the lid out while it is animated
  (`chest::set_animating`). No chest texture ships in the repo, so planks + an iron-block lock are used.
- Doors (`DoorTile` / `DoorItem`): composite data (dir, open, upper, right hinge in the upper half's bit 0), hinge chosen at placement,
  box per hinge/open state, mirrored textures, only the lower half stores open; iron doors only by redstone.
- Signs: `TileEntity::Sign` (Text1..Text4 in NBT), edit screen (`ScreenId::Sign`: type, Enter = next line, Done), text drawn in the world
  by `entrender::sign_text_layers` with the font sheet (`EntTex::Font`).
- Buttons: floor placement stays unsupported on purpose (`ButtonTile::mayPlace` accepts side faces only).

## Survival, XP, death, ender pearl (latest)
- Chest texture: `mc-client/src/chest_tex.rgba` (64x64, from the user's sheet) is cut into atlas cells by `chest::paint_icons` (lid / base / lock / inside); lid hinge and lock box in `chest_parts`. 
- Pistons: arm uses the wooden `piston_arm` cell (16px long, `rot_for_up`); an extended base no longer occludes its neighbours and chests don't either (`Ctx::occludes`), which fixed the see-through gaps. `piston_check` repairs an extended base that has no head.
- `survival.rs`: `FoodData` (1.6 constants; the C++ file is not in the repo), exhaustion from walking / sprinting / jumping / mining / attacking, natural regen, starvation, eating (32 ticks, golden apple / poison / hunger effects), drowning (air 300), `ExperienceOrb` + levels, block XP. Fall damage, no flying in survival.
- Death screen (`ScreenId::Death`) + respawn; XP / saturation / weather saved in `level.txt`.
- `pearl.rs`: `ThrownEnderpearl` (speed 1.5, gravity 0.03, drag 0.99 / 0.8 in water), portal particles, teleport + 5 damage, sprite via `entrender::pearl_layers`. Right click with item 368. No endermen / endermites.

## Large chests, camera hurt, eating / portal particles (latest)
- `chest.rs`: `ChestTile` port — `mayPlace` (one neighbour max, no third chest), `onPlace` / `recalcLockDir`, `setPlacedBy`, `getContainer` (pair = -x/-z chest first, 54 slots, blocked by a solid block on top of either half, both lids swing). Model: each half reaches to the shared edge (`updateShape`), the lock sits on the joint; the shared edge uses `half_cell` copies of the single-chest cells (atlas rows 28-29, painted by `paint_half_cells`).
- Large chest GUI: `Menu::chest` takes 27 or 54 slots (`ChestMenu` rows, inventory `(rows - 4) * 18` lower), panel height 114 + rows * 18.
- Levers / buttons need a full cube behind them (`mesh::full_cube`), so they no longer stick to chests or extended pistons.
- `Game::hurt_view` = `GameRenderer::bobHurt` (camera roll up to 14 degrees, direction from the attacker, 0 for falls).
- Eating: `Player::spawnItemParticles` (5 `BreakingItemParticle` every 4 ticks from tick 25 on, drawn by `entrender::item_particle_layers` from items.png).
- Ender pearl: no trail in flight; on impact 32 `PortalParticle`s (particles.png tiles 0-7 in atlas row 30), the player is moved with `Player::set_pos` (the old code forgot the bounding box and was snapped back).
- Chest inventory icon uses data 3 so the lock shows on the left face. XP bar / food icons are drawn procedurally (`ui::hud_extras`): the repo's `icons.png` has no art for them (the console HUD is Iggy/SWF).

## Infinite world, chunk workers, FPS limit (latest)
- World border: `world::INFINITE_XZ` = 2^20 chunks (±8 million blocks) replaces the 54-chunk "classic" size, so `height_falloff` / edge moat, structure `is_ok_box` limits and `Level` bounds never trigger. Strongholds use the large-world ring path (`xz_size >= 72`). The Nether uses `hell_scale 8` accordingly.
- `stream.rs`: terrain generation and save reads run on 1–3 worker threads (each owns its generator), save writes on a writer thread. `pending` keeps a chunk readable while its write is queued, so walking back never loads a stale file. The main thread only inserts finished chunks, decorates and lights within a 5 ms budget; one scan of the window per tick instead of one per step. Dropping the streamer flushes the writer (nothing is lost on quit).
- Render: frustum culling of chunk draws, each chunk meshed at most once per frame, meshes uploaded 6 per frame.
- Options: **Max Framerate** slider (10–250 fps in steps of 10, last notch = Unlimited; `about_to_wait` sleeps then spins to the frame time) and **VSync** toggle (`AutoVsync` / `AutoNoVsync`), saved in `options.txt`. Uncapped needs VSync off.

## Mobs, round 2: spiders and slimes (latest)

The full C++ source is on the PC (`MinecraftConsoles-main`); the copy first uploaded to the sandbox was partial, which is why the
earlier notes claimed "100 % of the available source" for mobs. Measured against the real source only 7 (now 11) of about 40 mob
kinds are ported, so the table above uses the kind count.

| Mob | State | Notes |
|---|---|---|
| Spider / Cave spider | ~85% | old (pre-goal) `PathfinderMob::serverAiStep`, wall climbing, leap, dark-only aggro, cave spider poison, eye glow layer. Missing: spider jockey (needs riding), random potion effects, player sneaking range |
| Slime | ~85% | sizes 1/2/4, `size²` health, hop AI (`jumpDelay`, ÷3 near the player), squish animation, split on death (2..4, max 35), slimeball drop, contact damage (= size, size ≥ 2), inner body + translucent shell model, slime-chunk / swamp spawn rules, saved with its size. Missing: tile step sounds (`Entity::playStepSound`), the ground shadow, `getNearestAttackablePlayer`'s sneaking reduction |
| Lava slime (magma cube) | ~75% | `LavaSlimeModel` (8 slices + core), armor `size × 3`, no fall damage, fire immune, ×4 jump delay, higher jump, magma cream drop, full-bright. Missing: flame landing particle (no flame particle yet), Nether spawn list (the Nether still uses the overworld list) |

Spawning: `MONSTER_SPAWNS` now has slimes (weight 10) like `Biome::enemies`; the per-biome lists and per-type `canSpawn` of
`MobSpawner` are still simplified.
