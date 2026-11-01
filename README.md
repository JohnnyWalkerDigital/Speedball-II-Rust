# Speedball 2 (Rust)

A Rust port of the Speedball 2 match engine, with sound, goal replays and
the front-end menus. Like the original, it reads graphics and the game's
lookup tables straight from the Atari ST / Amiga disk images in
`../src/Resources`.

## Running

```bash
cargo run --release
```

starts with the intro (fire or space skips it), then the main menu.
`--mode` skips both and plays one match:

```bash
cargo run --release -- --atari --mode two-players --team2 5
```

Options (`--help` lists them all): `--atari`/`--amiga` (match graphics),
`--mode knockout|two-players|practice|demo`, `--team1/--team2 <0-15>` (with
`--mode`), `--seed <n>` (same seed, same matches), `--scale 1|2|4|8`,
`--data <dir>`.

| Keys | Action |
| --- | --- |
| Arrows + Space / Right Ctrl | Team 1 (and the menus) |
| W A S D + Left Shift / Left Ctrl | Team 2 |
| P / N / F | In a match: pause / step one frame / fast forward |
| F | In a league or cup match as team manager: speed up / back to normal (the original's) |
| 1-9 | During a goal replay: save it to that slot |
| Esc | Leave a match or menu for the main menu; during a goal replay, end it; on the main menu, quit |

With `--mode knockout`, both teams are computer controlled until you touch a
team's controls.

## Asset packs

The graphics, the menus' and intro's words, the game's tables and its
sound can be extracted from your originals into a folder of PNG, JSON,
text and WAV files, edited, and played with:

```bash
cargo run --release -- --extract assets
```

```bash
cargo run --release -- --assets assets
```

`content/` is the seam: the game asks a `ContentSource`, which is either
`Originals` (decoding the disk images and dumps as before) or a `Pack`. A
pack holds the match graphics for both platforms as RGBA PNGs (pitch,
tiles, sprites, panels, fonts, status bar), and the menu and intro
pictures, fonts and manager-screen sheets as indexed PNGs, since the menus
fade by palette. `pack.json` lists each sprite sheet's cell size and count
and holds the palettes.

`text/menus.json` holds every string the menus show, by purpose (titles,
list menus and their arrows, messages, the panel screens' lines, team
names), and `text/intro.json` the intro's. Strings are written in a
readable form that converts back to the game's bytes exactly
(`data/readable.rs`): `{x,y}` for cursor codes, `_` for a blank the game
writes a number into, and box-drawing characters for the panel tables'
lines. The game writes into some strings at fixed places, so a pack's text
is checked when it loads (`MenuText::check`) and problems are reported
rather than crashing. The pack's `README.txt` explains the rules for
editing.

`tables/` holds the game's numbers:
- `match.json`: the match's lookup tables (`data/lookups.rs`): velocities,
  the skill curves, tackling, equipment, the AI's support grid, ball and
  sprite offsets. A lookup outside an edited table reads the original
  memory around it, as the original game did, so out-of-range indices behave
  exactly as before; a test checks every lookup, in range and out,
  against the raw ROM.
- `teams.json`: the league teams' skill templates and match-panel names.
- `squads.json`: the starting squads, the transfer market, the gym's
  upgrades and prices, and the results table for matches without Brutal
  Deluxe (`menu/tables.rs`). Saves still name players by their original
  name offsets, mapped through these tables.
- `manager_screens.json`: the keypads and what each key does, the gym's
  buttons and armour, labels and the squad grid (`menu/manager/data.rs`).
- `animations.json`: the animation scripts as named frame lists, and the
  per-direction animations (`data/animations.rs`). In the original they are
  one block of memory that entities point into, and an entity carries its
  place over when its script changes, so it can read past a script's end.
  The scripts are laid out in the same order, so a position in a script is
  still a position in that block (unedited, the original address); a read
  past the block reads the original memory.
- `start.json`: every entity's and player's starting record and the match
  roster (`game/start.rs`), with scripts, behaviours and sprite banks by
  name. Entities are known by `EntityId` rather than their addresses.

`sound/` holds what the sound driver plays (`sound/data.rs`):
- `sounds.json`: the driver's tables, shared by every bank: the sequence
  each of the 56 sounds starts on each channel, the tremolo and vibrato
  effects, and the pitch tables.
- `game/`: the match's bank. `sequences.txt` is the sequencer's bytecode
  as text (`sound/sequences.rs`): notes as numbers, commands as words
  (`tempo 188 instrument 12 length 10 53 stop`), any byte that is no
  command as `byte 0xNN`, so it reads back exactly. `instruments.json`
  names each instrument's sample, whether it loops and its octave, and
  `samples/` holds the samples as 8-bit WAV files.
- `menus/samples/`: the samples the menus load over the match's (the
  menus play the match's sequences and instruments).
- `intro/`: the intro's bank, with its music.

In memory a bank is the sequences, their table, the samples and the
instrument table, back to back. Some instruments share a sample or play
on past its end into the next, so a sample here is the run of bytes from
one instrument's start to the next's, and an instrument whose length is
not its sample's says so. Each bank is laid back out over a copy of the
original memory in the same order, so unedited it is byte for byte the
original (a test checks all three); edited, what follows moves along,
and reads past the end still find the original memory. The no-play
flags, sequences and effects are written into the driver's tables in
place; envelopes, which no sequence uses, are still read from the
originals.

Each is checked when it loads. A test extracts a pack and checks every
image, sheet, palette, string, table and sound loads back identical to
the originals.

`fingerprint_matches` (an ignored test) plays 22 whole matches and prints
a hash of every frame drawn and sound requested; refactors are checked
against its output from before, and `SPEEDBALL_PACK=<dir>` plays the same
matches from a pack's tables.

The match's panel messages ("GOAL", "HALF TIME" and the rest) and the
injured players' position names are defined in `game/text.rs`; a test
checks them against the original bytes. The
memory the sound banks are laid over still comes from the originals, so
they are still needed. `/assets`
is in `.gitignore`: an extracted pack is the game's own art, so it should
not be shared.

## Menus

`menu/` is a port of the Amiga front end: the main menu and the game
modes. It runs on the Amiga data whichever graphics the match uses:
`data/amiga.rs` reads the disk's overlay directory and rebuilds the menus'
memory from `Amiga.dmp` the way the game does on entering them, loading the menu
graphics (overlay 1) and the menu sound samples (overlay 28) at their
original addresses. The menu code then reads its strings, layouts and fonts
from there, and the sound driver switches between the menu and match
samples as the original did.

The menu code is written like the original: straight-line code that waits
for the next frame. `menu/host.rs` is the seam: the window implements it
for play and a scripted player implements it for the tests. Screens are
drawn as palette indices into a back buffer and brought up with the
original's palette fades (15 steps, 40 ms apart).

Ported:

- Main menu ("SELECT GAME"), with the demo starting after 30 idle seconds.
- One player: knockout (the 15 other teams in turn; a draw replays the
  round, a defeat ends it; the knockout trophy screen after the last),
  league, cup and practice.
- League (`menu/league.rs`, `game/league.rs`): two divisions of eight,
  Brutal Deluxe starting in division 2. Each season is two halves of seven
  rounds (every team plays every other in its division, then again with
  ends swapped). Brutal Deluxe's matches are played; the others come from
  the original's table of pre-baked results. After each round come the
  results and the tables (sorted by league points, then goal difference,
  with arrows for movement and Brutal Deluxe's name blinking; up and down
  switch division). At the end of a season in division 2, the top team
  swaps with the bottom of division 1, and seventh in division 1 plays
  second in division 2 for the other place. Brutal Deluxe go on to a new
  season if promoted; otherwise, or after a season in division 1 (the
  trophy if they win it), the game ends. "TEAM MANAGER" has the computer
  play Brutal Deluxe's matches, for at least 300 a match.
- Cup (`menu/cup.rs`, `game/cup.rs`): the sixteen teams drawn into
  two-legged ties, with extra legs while the aggregate is level, redrawn
  among the winners before rounds two and three. Coins are worth double,
  both players start with 2000, and Brutal Deluxe's money doubles after
  each round from the second.
- The next-fixture screen before each of Brutal Deluxe's knockout, league
  and cup matches: both teams' records and average skills.
- Manager screens (`menu/manager/`, `game/squad.rs`): before each league and
  cup match, the manager screen; before each knockout match, and for each
  player before each two-player leg, the gym; after the demo, a look at
  both. Everything is worked from a keypad (moving off its sides goes into
  the squad table or the gym's upgrade buttons, as the Amiga's one fire
  button required):
  - Gym: eight upgrades, each ten points on one skill, for a player, a
    group of three or the whole squad. Skills can only be raised to 30 above
    the weakest group's average (rounded down to ten), and no higher than
    240 in a knockout, 220 otherwise (230 for a team manager in division 1).
    Fire on an upgrade shows it; fire again buys it.
  - Transfers: up to three of eighteen market players are for sale at a
    time, coming and going over the management rounds; which ones depends on
    their value and the division. Buying one costs the difference in value
    from the player he replaces, and he can only play in his own position
    (or on the bench). He can be trained back up to his market skills.
  - Substitutions: swap any two of the squad's own players; market players
    keep to their row or the bench.
  - The league tables or cup fixture list, the next fixture, and the last
    match's statistics.
- Squads persist between matches. A player injured in a match goes to the
  end of the bench with his skills rounded down to tens, and stays there,
  as in the original (it changed the squad's own records).
- Two players: one match or the best of three or five, each match two legs
  with ends swapped and extra legs while the aggregate is level.
- The demo game.
- Result screens: score and winner, the aggregate after two legs, the
  victory and defeat pictures for Brutal Deluxe, "PLAYER n WINS" and
  "GAME OVER".
- After each match (`game/career.rs`): prize money, the teams' records
  and the match statistics for the statistics
  screen (`game/statistics.rs`). These persist between games, as the
  original's variables did: cash resets on the main menu, but each team's
  earnings cap does not.

Prize money: a team keeps what it picked up in coins (100 each), at least
the game's starting amount (0, or 300 in league games) and at most its cap
(1100 in a knockout, 2000 for two players, 10000 in practice). Whatever it
fell short of the cap is added to its next cap. Coins stop appearing once
team 1 reaches its cap (in two-player games, once both teams have).

- Saving and loading leagues and cups (`menu/saving.rs`, `game/save.rs`):
  the manager screen's save key (every fourth management round) and "LOAD
  GAME" in the league and cup menus. The original wrote to a blank "game
  save disk"; here that disk is an 880K image file, `saves/save-disk.adf`
  (`--save-disk` to choose another), with each save at the original's
  sectors (a league at sector 0, a cup at 22) in the original's byte layout,
  so an emulator's save disk can be used. A failed read or write gets the
  original's "DISK ERROR" menu: retry or cancel. A loaded game rejoins where
  it was saved: the same round, or the second leg of a cup tie.

- Goal replays saved to the same disk image (keys 1-9 during a replay) and
  the "REPLAY GOALS" viewer (`menu/replays.rs`); see Goal replay.
- Sped-up matches: F in a league or cup match as team manager runs it
  without drawing the pitch, which stays as it was (with any panel message
  drawn over it) while the status bar keeps up, and skips the goal
  replays. F again (or F during a replay) toggles it; it lasts for the rest
  of the game, until the main menu.
- The intro (`menu/intro.rs`): "IMAGEWORKS PRESENTS", "A BITMAP BROTHERS
  GAME", then the title with its music, the story and the credits typed out
  letter by letter over the arena, round and round until fire or space is
  held, and "LOADING GAME". It comes from `Intro - Amiga.dmp`; without that
  file the game starts at the main menu.

There is no team selection in the original: one-player games are always
Brutal Deluxe.

## Sound

`sound/` is a port of the original Amiga sound driver and its sequencer,
plus a small emulation of the Amiga's sound chip. It plays the original sequences,
instruments, envelopes and effects from the memory they were in
(`Amiga.dmp`, with the sound data laid back over it from the originals or
a pack), ticking at 50 Hz like the original's vertical-blank interrupt. Channels 0 and 3 play on
the left and 1 and 2 on the right, as on the Amiga. It needs `Amiga.dmp` even
with `--atari` graphics; without it, or with `--mute`, the game is silent.

The match requests sounds (`game/events.rs`) wherever the original plays one
in a match, with the same sound ID and priority: kick-off, the ball
launcher, throws, catches, interceptions, jumps, slides, tackles, injuries,
wall bounces, stars, ramps, warps, domes, electrobounces, pickups, power-ups,
the power-up countdown, goals, half time and the final whistle. Several crowd reactions depend on
which team started the match attacking up, as in the original.

Not included: sounds from the menus and manager screens, which this engine
does not have.

`--export-sounds <DIR>` writes the raw instrument samples and, in
`<DIR>/rendered`, every one of the 56 game sounds played through the driver.

## Goal replay

As in the original, every goal is followed by an action replay
(`game/replay.rs`). While the
clock runs, each frame's camera position and on-screen sprites are recorded
into a ring buffer the size of the original's (765 entries: one per frame
plus one per sprite), so the replay covers the last 3 or so seconds, fewer
when the screen is busy. It plays at half speed with "SCORED" or "OWN GOAL"
on the panel, and the score in the status bar updates when it ends. Either
team's fire button, or Escape, ends it.

Entries are packed as the original packed them: the sprite's
Amiga bank and index (`game/sprites.rs` maps the port's ST numbering to the
Amiga's four banks), its screen position and its colours. So whatever
platform's graphics are showing, the replay is the Amiga's, and keys 1-9
save it byte for byte as the original did: the 3K buffer,
the serial number the loader left in memory, the start and end, and
"goal", six sectors at sector 44 plus a track per slot. The original
allowed this only in one-player games outside manager mode. "REPLAY GOALS"
on the main menu plays slots 1 to 9 in turn, skipping empty ones, with
"GOAL n" on the panel for its first twenty frames, then says "REPLAYS
OVER".

## Tests

```bash
cargo test
```

Unit tests cover the decoders, the RNG and the
geometry. `simulation_tests.rs` plays whole matches headlessly against the
real disk images on both platforms, rendering every frame, and checks that a
seed always replays identically. `front_end_tests.rs` drives the menus with
a scripted player: the demo after 30 idle seconds, Escape leaving a match,
two-player legs, the league, cup and manager screens, saving and loading,
a goal replay saved in a match and watched from the menu, a sped-up match
and the intro. These are skipped when the images are missing.
`cargo test dump_ -- --ignored` with `SPEEDBALL_FRAMES_DIR` set writes sample
match, menu and result frames as BMPs.

## Layout

| Module | Role |
| --- | --- |
| `data/` | Disk loading, big-endian reads, named RAM addresses and lookup tables |
| `game/state.rs`, `state/` | `Match`: state and the per-frame update; the clock, camera, scoreboard, kick-off, ball, goals and message panel |
| `game/actions.rs` | Running, tackling, jumping, catching, throwing |
| `game/ai.rs` | Computer decisions |
| `game/items.rs`, `token.rs` | Coins, equipment, power-up tokens |
| `game/arena.rs` | Stars, multipliers, domes, electrobounces |
| `game/medic.rs` | Injuries and substitutions |
| `game/scene.rs` | The sprites on screen each frame, in drawing order |
| `game/replay.rs` | Recording, playing back and saving the goal replay |
| `game/sprites.rs` | The Amiga's sprite banks |
| `game/setup.rs` | Building teams per game mode |
| `game/career.rs`, `league.rs`, `cup.rs`, `squad.rs`, `statistics.rs` | Money, records, the league and cup rules, the squad, gym and market, match statistics |
| `graphics/` | Palettes, bitplane decoding, sprite sheets, pitch assembly |
| `render.rs` | Draws a `Match` onto the screen |
| `menu/` | Intro, main menu, game modes, result screens, replay viewer |
| `menu/manager/` | Manager screens: a session's view and drawing, the keypad and its keys, the squad table, transfers and the gym |
| `app.rs` | Window, keyboard, timing |

Design choices:

- **The simulation doesn't draw.** `render.rs` builds each frame from the
  match state, so `game/` has no graphics dependency and can run headless.
- **No shared mutable references.** `Match` owns everything, and
  cross-references are `PlayerId` / `EntityRef` handles.
- **Faithful logic, readable names.** The AI keeps the original branch
  structure and integer arithmetic so matches play the same, but routines
  have descriptive names, and code pointers became the `Think` enum.

## Differences from the original

- Player 2 has keyboard controls; the original only wired player 1.
- The menus differ from the Amiga's in a few ways: Escape on the main menu
  quits (the original had nowhere to go); each match gets a seed from
  `--seed` instead of random numbers stirred while the menus wait for
  input; and the "insert disk" prompts are gone, except before saving and
  loading.
- In a match, the original paused on any key without a job until the next
  key; the port has P instead, and F is its fast forward wherever the
  original's F does nothing.
- A sped-up match ran as fast as the Amiga could compute it; the port runs
  eight steps a frame.
- A saved replay's buffer holds zeros where the replay doesn't reach; the
  original's held whatever earlier matches had left there.

## Oddities kept on purpose

These look like mistakes but are how the game behaves, so the port keeps them.

Gameplay:

- Score multipliers are never reset during a match, so one-sided games can
  produce big scores.
- Hitting an opponent's lit wall star takes 2 points straight off their
  score, ignoring multipliers, though lighting it earned them 2 to 4 with a
  multiplier. (So a score never goes below zero.)
- When a jump lands, player 1's fire button is always cleared, whichever
  team jumped; player 2's never is.

Crowd and sound:

- Several crowd sounds depend on which team *started the match* attacking up,
  not which way they attack now, so after half time they follow the other
  end.
- After a completed star row, a multiplier change and an electrobounce, the
  crowd cheers when the last thrower's team started attacking *up*, even
  when the other team completed the star row.
- An injury makes the crowd cheer only when the injured player's team started
  attacking down.
- The "knocked down" grunt is picked by the *victim's* AI random byte, and the
  "ice cream" coin flip reads the random seed without advancing it.
- The goal replay leaves the camera where the replay ended; play resumes
  from there and the camera scrolls back to the ball. The replay is not
  consumed, so a goal scored soon after another replays the end of the
  previous one too.
- During a replay, the crowd shouts "ice cream" when the panel counter
  reaches 64 if bit 5 of the random seed is set. The seed doesn't change
  during a replay, so it is the same for the whole replay.
- Sound 43 is silence: it just stops channel 0. It plays when a new panel
  message appears, when a player gets up from a tackle and when someone is
  injured.

Menus and results:

- In a two-player series, the deciding match is not added to the records;
  only matches that leave the series open are.
- A new league numbers the teams 16 (Brutal Deluxe) down to 1, then sorts
  the tables before the first match. With everyone on no points nothing
  moves, so Brutal Deluxe start the season top of division 2.
- The league ends unless Brutal Deluxe are promoted: finishing a season in
  division 1 ends the game whatever the placing, with the trophy for first.
- Play-off matches are not added to the records.
- A bought market player's value keeps the market premium (a quarter, or
  half in the cup or for a team manager), so selling him back into a
  transfer counts for more than the same skills on a squad player.
- The gym's group and team purchases check the cash for the whole group
  first, then buy for each player in turn; players already at their limit
  are skipped without a message, and the money for them is not spent.
- The save key shows as a blank key except every fourth management round,
  the only rounds the original let you save.
- A save doesn't keep the round's results so far, the player-2 squad or the
  random numbers. So after loading part way
  through a league round, the results screen shows zeros for the matches
  played before the save, and the tables are sorted again from the saved
  fixture order (at the start of a season, with every team level, that
  reorders the placings).
- A loaded cup round always starts again from its first tie, which is
  Brutal Deluxe's (they are always drawn first), so nothing is replayed.
- The statistics are bytes per player, so a player's possession count wraps
  after 255 frames. A substitute takes over the injured player's slot with
  its counters reset, so the injured player's goals and tackles drop out of
  the team's statistics screen (the score is unaffected); only the
  substitution count carries over.

Replays and the intro:

- The domes are pitch tiles on the Amiga except for the frame when one is
  hit, so only that frame is recorded, and on ST graphics the replayed domes
  are drawn idle under it.
- A 32x32 sprite records whether it came from the launcher's bank, but
  not which of the others, so the big ball (its own bank) replays as the
  player frame with the same index.
- The replay viewer shows the status bar as the last match left it, since
  the Amiga only draws it once at boot; before any match, it is blank.
- Keys 1-9 work in the viewer when the last game was a one-player game
  outside manager mode: the flags that allow it outlast the game.
- The intro's typing sound was meant to differ for spaces, but the
  character is compared before it is converted, so every letter gets the
  same sound (0x36); the cursor's blinks get the other (0x37). Only the
  "presents" screens make these sounds, and they cannot be skipped.
- Fire skips the intro only when it is held as the intro checks it: a press
  and release within a fade goes unnoticed.

Sound driver:

- Sound priorities are only checked on channels 0-2; anything can interrupt
  channel 3.
- `repeat N ... next` runs its body N + 1 times.
- A channel's transposition survives into the next sound: starting a sound
  clears the rest of the channel's state, but not that.
- At each note, a sample with no loop point is cut by pointing the channel at
  the default instrument, where the zero sample may have been meant. Both
  are silent in practice.
- Vibrato does not reset its pitch adjustment when it ends; tremolo does reset
  its volume adjustment.
- An envelope release time of 0xFF is compared as a signed byte, so it means
  "never release".
- Refused sounds are recorded in a table nothing ever reads.
