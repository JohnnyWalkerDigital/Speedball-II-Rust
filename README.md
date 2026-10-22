# Speedball 2 (Rust)

A Rust port of the 2011 C# reimplementation of the Speedball 2 match engine
in `../src`, extended from the Amiga disassembly with sound, goal replays
and the front-end menus. Like the original, it reads graphics and the game's
lookup tables straight from the Atari ST / Amiga disk images in
`../src/Resources`.

## Running

```bash
cargo run --release
```

starts at the main menu. `--mode` skips the menus and plays one match:

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
| Esc | Leave a match or menu for the main menu; on the main menu, quit |

With `--mode knockout`, both teams are computer controlled until you touch a
team's controls, as in the C# port.

## Menus

`menu/` is a port of the Amiga front end (`main_menu` and the game-mode
routines). It runs on the Amiga data whichever graphics the match uses:
`data/amiga.rs` reads the disk's overlay directory and rebuilds the menus'
memory from `Amiga.dmp` the way `load_mgmt_data` does, loading the menu
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
  round, a defeat ends it; the knockout trophy screen after the last) and
  practice.
- Two players: one match or the best of three or five, each match two legs
  with ends swapped and extra legs while the aggregate is level.
- The demo game.
- Result screens: score and winner, the aggregate after two legs, the
  victory and defeat pictures for Brutal Deluxe, "PLAYER n WINS" and
  "GAME OVER".
- After each match (`game/career.rs`): prize money, the teams' records
  (`update_match_stats`) and the match statistics for the statistics
  screen (`game/statistics.rs`). These persist between games, as the
  original's variables did: cash resets on the main menu, but each team's
  earnings cap does not.

Prize money: a team keeps what it picked up in coins (100 each), at least
the game's starting amount (0, or 300 in league games) and at most its cap
(1100 in a knockout, 2000 for two players, 10000 in practice). Whatever it
fell short of the cap is added to its next cap. Coins stop appearing once
team 1 reaches its cap (in two-player games, once both teams have).

Not yet: league and cup (selecting them goes back to the main menu), the
manager screens (the gym before knockout and two-player matches, the
knockout fixture screen, the statistics screen, the manager screens after
the demo, and anything to spend the money on), replay saving and the
"REPLAY GOALS" viewer (selecting it does nothing), and the intro. There is no team selection
in the original: one-player games are always Brutal Deluxe.

## Sound

The C# port had no sound. `sound/` is a port of the original Amiga sound
driver (`sound_play` and its sequencer, recovered from the disassembly) plus a
small emulation of the Amiga's sound chip. It plays the original sequences,
instruments, envelopes and effects straight from `Amiga.dmp`, ticking at
50 Hz like the original's vertical-blank interrupt. Channels 0 and 3 play on
the left and 1 and 2 on the right, as on the Amiga. It needs `Amiga.dmp` even
with `--atari` graphics; without it, or with `--mute`, the game is silent.

The match requests sounds (`game/events.rs`) at every in-match `sound_play`
call in the original, with the same sound ID and priority: kick-off, the ball
launcher, throws, catches, interceptions, jumps, slides, tackles, injuries,
wall bounces, stars, ramps, warps, domes, electrobounces, pickups, power-ups,
the power-up countdown, goals, half time and the final whistle. Each constant
names the original routine it comes from. Several crowd reactions depend on
which team started the match attacking up, as in the original.

Not included: sounds from the menus and manager screens, which this engine
does not have.

`--export-sounds <DIR>` writes the raw instrument samples and, in
`<DIR>/rendered`, every one of the 56 game sounds played through the driver.

## Goal replay

As in the original, every goal is followed by an action replay
(`game/replay.rs`, ported from `run_replay` and `replay_frame`). While the
clock runs, each frame's camera position and on-screen sprites are recorded
into a ring buffer the size of the original's (765 entries: one per frame
plus one per sprite), so the replay covers the last 3 or so seconds, fewer
when the screen is busy. It plays at half speed with "SCORED" or "OWN GOAL"
on the panel, and the score in the status bar updates when it ends. Either
team's fire button skips it. The frames drawn live and replayed come from
the same list (`game/scene.rs`, `draw_game`'s order), which the renderer
draws.

Not included: saving replays to disk (keys 1-9 during a replay) and the
replay viewer in the main menu.

## Tests

```bash
cargo test
```

Unit tests cover the decoders, the RNG (bit-exact against the C#) and the
geometry. `simulation_tests.rs` plays whole matches headlessly against the
real disk images on both platforms, rendering every frame, and checks that a
seed always replays identically. `front_end_tests.rs` drives the menus with
a scripted player: the demo after 30 idle seconds, Escape leaving a match,
two-player legs. These are skipped when the images are missing.
`cargo test dump_ -- --ignored` with `SPEEDBALL_FRAMES_DIR` set writes sample
match, menu and result frames as BMPs.

## Layout

| Module | Role | C# origin |
| --- | --- | --- |
| `data/` | Disk loading, big-endian reads, named RAM addresses and lookup tables | `ExtensionByte`, scattered `Game.AtariDisk.Read*` calls |
| `game/state.rs` | `Match`: state and the per-frame update | `Match.cs` |
| `game/actions.rs` | Running, tackling, jumping, catching, throwing | `Player.cs` (action routines) |
| `game/ai.rs` | Computer decisions | `Player.cs` (`sub_D742`, `sub_E076`, ...) |
| `game/items.rs`, `token.rs` | Coins, equipment, power-up tokens | `Entity.cs`, `Token.cs` |
| `game/arena.rs` | Stars, multipliers, domes, electrobounces | `Stars.cs`, `ScoreMultipliers.cs`, ... |
| `game/medic.rs` | Injuries and substitutions | `Match.cs` (`AnimMedic*`), `Player.sub_F6DC` |
| `game/scene.rs` | The sprites on screen each frame, in drawing order | drawing code in `Match.cs` |
| `game/replay.rs` | Recording and playing back the goal replay | none (new, from the disassembly) |
| `game/setup.rs` | Building teams per game mode | `Game.cs` |
| `graphics/` | Palettes, bitplane decoding, sprite sheets, pitch assembly | `Palette`, `Sprite`, `SpriteManager`, `Field` |
| `render.rs` | Draws a `Match` onto the screen | drawing code spread through `Match`/`Entity` |
| `menu/` | Main menu, game modes, result screens | none (new, from the disassembly) |
| `app.rs` | Window, keyboard, timing | `Main.cs`, `Program.cs` |

Design choices:

- **The simulation doesn't draw.** The C# drew the status bar from inside
  game logic and only when values changed. `render.rs` now rebuilds each
  frame from the match state, so `game/` has no graphics dependency and can
  run headless.
- **No shared mutable references.** C# objects pointed at each other freely
  (`EntityHoldingBall`, `PlayerThrower`, token holders). Here `Match` owns
  everything, and cross-references are `PlayerId` / `EntityRef` handles.
- **Faithful logic, readable names.** The AI keeps the original branch
  structure and integer arithmetic so matches play the same, but routines
  have descriptive names with the original label in their docs (`sub_DBE8`
  is `pass_from_defence`), and code pointers became the `Think` enum.

## Differences from the C# version

- Player 2 has keyboard controls; the original only wired player 1.
- The mouse step/fast-forward controls are now P/N/F.
- The thrower starts as team 1's kick-off player instead of `null`, the
  status bar clock shows from the first frame, and negative scores display as
  zero. Cases where the C# would have thrown an exception (unknown code
  pointers, out-of-range sprites) are ignored instead.
- The first-run export of every sprite to PNG and the debug dumps were dropped,
  as were menu and intro assets the match never uses.
- Matches are seeded (`--seed`) rather than using `System.Random`.
- Goal replays, the "SCORED"/"OWN GOAL" captions and the status bar holding
  the old score until the replay ends are new; the C# had no replay. Wall
  stars are drawn under every sprite, as the original's pitch tiles are.
- C# bugs fixed, checked against the Amiga disassembly: the ball
  keeps its sideways drift through a warp tunnel when moving straight up or
  down (`step_warp`), and the ball launcher shows during a launch and hides
  once the ball is in play (`step_prepare_ball_launch`, `step_ball_launch`).
- The status bar's team 2 midfield light tests team 2's own player, as the
  original does (`green_light_active_player`); the C# tested team 1's.
- Each team has its own earnings cap (`match_cash_max_p1`, `_p2`); the C#
  gave team 2 team 1's. They only differ once caps carry over between
  matches.
- The menus are new (see Menus). Where they differ from the Amiga: Escape on
  the main menu quits (the original had nowhere to go); each match gets a
  seed from `--seed` instead of random numbers stirred while the menus wait
  for input; and the "insert disk" prompts are gone.

## Oddities kept on purpose

These look like mistakes but are how the game behaves, so the port keeps them.
Each was checked in the Amiga disassembly unless marked otherwise.

Gameplay:

- Score multipliers are never reset during a match, so one-sided games can
  produce big scores.
- Hitting an opponent's lit wall star takes 2 points straight off their score,
  ignoring multipliers, which can push it below zero (shown as 000).
- `jumping_action_fn` always clears player 1's fire button when a jump lands,
  whichever team jumped; the line that would clear player 2's is never reached.

Crowd and sound:

- Several crowd sounds depend on which team *started the match* attacking up
  (`initial_playing_direction`), not which way they attack now, so after half
  time they follow the other end.
- `play_sound_downwards` cheers when the last thrower's team started attacking
  *up*, despite its name. It runs after a completed star row, a multiplier
  change and an electrobounce, always checking the last thrower's team, even
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
- The code's comments around the interception sounds (`get_ball`) contradict
  each other; the port follows the branches, not the comments.

Menus and results:

- A negative score on a result screen shows as "ZZZ": `format_decimal_digit`
  divides with `divu`, which gives up on a negative number and leaves the
  remainder as -1, one before the digit 0 in the font.
- Scores are compared as unsigned 16-bit numbers, so a negative score
  (possible through the wall stars) beats any positive one: on the result
  screen, in the knockout, in a two-player series and in the records. In
  the records it also counts in full rather than a tenth, for the same
  `divu` reason.
- In a two-player series, the deciding match is not added to the records;
  only matches that leave the series open are (`game_two_player`).
- The statistics are bytes per player, so a player's possession count wraps
  after 255 frames. A substitute takes over the injured player's slot with
  its counters reset, so the injured player's goals and tackles vanish
  from the team's statistics; only the substitution count carries over.
- `clear_match_stats` clears one team record too many: its eighteenth
  pointer is the program's first instruction, which points into unused
  memory.

Sound driver:

- Sound priorities are only checked on channels 0-2; anything can interrupt
  channel 3.
- `for N ... next` runs its body N + 1 times.
- A channel's transposition survives into the next sound, because
  `sound_play_channel` clears the channel one field short of it.
- At each note, a sample with no loop point is cut by pointing the channel at
  the default instrument; the disassembly's comments suspect the zero sample
  was intended. Both are silent in practice.
- Vibrato does not reset its pitch adjustment when it ends; tremolo does reset
  its volume adjustment.
- An envelope release time of 0xFF is compared as a signed byte, so it means
  "never release".
- `sound_play` records refused sounds in a table nothing ever reads.

From the C# port (not checked against the original):

- The computer goalkeeper's positioning (`sub_E61A`) has bounds that look
  inverted; the C# marks them `// bug?`.
- Team 2's credit limit is copied from team 1's.
