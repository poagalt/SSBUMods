# Rat of Doom — Pikachu Custom Moveset

Status: **in design (grilling)**. Nothing below is settled until the open questions in
`docs/decisions.md` are answered.

## Original brief (verbatim)

> Make a SSBU mod that puts a custom moveset over the pikachu character model. the described
> moves should use the exact frame data, damage, and range as the character that it is normally
> on. Animations should use the pikachu character model as best as possible - if the move is on a
> character that reaches farther than pikachu, port in the appendage/tool for the period of time
> where the hitbox is active, then remove it again when the move ends. For example, for little mac
> fmsmash display little mac's arm extending from pikachu's body while the move is active. That
> appendage's hurtbox should exist and be active for that time too (if it's a sword or something
> disjointed, then don't have a hurtbox appropriately). For moves which need a different size
> character than pikachu, for example, inciniroar back air, stretch pikachu's character size
> appropriately for the animation's duration. For any moves that need a sword, use the sprite of
> chrom's sword (move's range should still be as long as it needs to be). Ignore items in
> parenthesis, those are for my own reference as other options for that move. The mod should NOT
> require the csk collection - see this mod https://gamebanana.com/mods/399292 as an example of how
> to do this. It is downloaded and extracted in the poaga/Downloads directory, and contains several
> of the same moves and implementations requested here. Use online research to help you whenever
> you get stuck - you should be able to find information on everything you need to do to get this
> working online.

## Moveset

| Slot | Source fighter | Notes |
|---|---|---|
| Jab | Chrom | sword |
| Dash attack | Mythra | sword |
| F-tilt | Mythra | sword; replaced Terry after ADR 0001 excluded special-cancel |
| U-tilt | Falco | two-hit, hits both sides |
| D-tilt | R.O.B. | f3, 14 total, -5 |
| F-smash | Little Mac | **super armor f8-15 (9-15 down-angled)** |
| U-smash | Fox | |
| D-smash | Mr. Game & Watch | buries / 2-frames / semi-spike |
| Nair | Palutena | |
| F-air | Kazuya | |
| B-air | Cloud | sword |
| U-air | Corrin | sword |
| D-air | King K. Rool | |
| Neutral B | Piranha Plant | Ptooie |
| Side B | Incineroar | Alolan Whip (command grab, 3 follow-ups) |
| Up B | Mr. Game & Watch | Fire + parachute |
| Down B | Incineroar | Revenge (counter, stores x3) |
| Grab | Palutena | |
| Pummel | Kazuya | |
| F-throw | Donkey Kong | **cargo carry + all four cargo throws** |
| B-throw | Incineroar | |
| U-throw | Mewtwo | |
| D-throw | Pikachu | unchanged |
| Get-up attack | Bowser | face-up, face-down and trip attack; chosen after research |
| Ledge attack | Ganondorf | |
| Spot dodge / roll / air dodge | Fox | |

## Attributes

| Attribute | Source fighter |
|---|---|
| Weight | Bowser |
| Initial dash | Zero Suit Samus |
| Movement (run) speed | Little Mac |
| Walk speed | Marth |
| Air speed | Jigglypuff |
| Air acceleration | Jigglypuff |
| Jump height | Falco |
| Fall speed | Joker |

## Explicitly out of scope / unassigned

Taunts, victory animations, voice/SFX, portraits. The CSS name and the ledge attack are done.
Not yet decided whether these stay vanilla Pikachu.

## Hard constraint

Must **not** depend on the CSK Collection. Reference implementation:
"George" by LilyLambda / TCNick3 (GameBanana 399292), extracted at
`C:\Users\poaga\Downloads\george_v30_by_lilylambda`.
