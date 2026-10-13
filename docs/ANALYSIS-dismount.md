# Dismounting: this game against Source and CS2

Written 13 October 2026, after Sam reported that surf_kitsune's second stage felt much harder than the original, "as if a wedge is missing", and that leaving a surface felt different from CS2 surf.

"Dismounting" here is everything from the moment a surfer leaves a ramp to the next contact: the ramp's end or top edge, the flight, the stage door, and the landing. Each behaviour is listed with what Source 1 does (from Source SDK 2013), what CS2 does (from Valve's patch notes and cs2kz-metamod's reimplementation of CS2 movement; CS2's own code is not public), what this game did before this change, and what it does now.

## What was wrong, in order of how much it mattered

### 1. The stage door stopped you dead (the big one)

Source's `trigger_teleport` keeps the player's velocity. In `src/game/server/triggers.cpp` (Source SDK 2013, `CTriggerTeleport::Touch`), only Half-Life 1 zeroes it; every other game passes a null velocity, and Valve's own comment marks it: `pVelocity = NULL;	//BUGBUG - This does not set the player's velocity to zero!!!`. The view is turned to the destination's angles, but the speed comes through. Surf maps are built around this: surf_kitsune's stage doors are holes in the end wall of each stage, and you arrive at the next stage with the speed you hit the door at.

This game zeroed the velocity on every teleport. On kitsune that meant every stage started from a standstill.

Stage 2 shows what that costs. It is one ridge, about 1,040 units long, whose front is a wall 15 units higher than the start platform, and whose end is about 500 units short of a platform 20 units lower than the ridge's last top. To clear that gap the surfer has to leave the ridge high, at roughly 800 u/s. Mechanical energy (height plus speed squared over twice gravity) does not appear from nowhere: strafing adds at most 900 (u/s)² a tick, which over a one-second ridge is far too little. From a standstill the stage is all but impossible. Stage 1 drops about 350 units from its start to its door, which alone gives about 750 u/s, more with strafing, and that speed is meant to come along.

`crates/core/tests/stage2.rs` measures it: a beam search over yaw and keys reaches the far platform when it arrives with the speed stage 1 gives (1,100 u/s) and never does from a standstill. Nothing in the geometry is missing: every brush of the stage is present and solid as in the BSP (`SECOND-ORDER.md` row 23).

**Now:** a teleport that keeps the run going (a stage door, a fall back to a stage's start) keeps velocity, as Source does. A timer restart or a finish still puts you at the start standing still, as a timer plugin does. Test: `a_stage_door_keeps_your_speed_as_source_does`, and `stage_two_with_the_speed_stage_one_carries`.

### 2. A quarter of the air control when rising slowly

Source 1's `CategorizePosition` sets surface friction to 0.25 when the player is off the ground and rising at no more than 140 u/s, and `AirAccelerate` multiplies by it: right after a slow launch off a ramp's top, air acceleration is a quarter. With airaccelerate 150 the cap of 30 u/s usually binds first, so this bites only when you counter-strafe or turn hard, which is what steering a dismount is. CS2's November 2025 update says it "Fixed a case where slowly surfing up a ramp was handled incorrectly due to misidentification as a jump apex" and "Fixed a case where air acceleration was clamped incorrectly while surfing".

**Now:** no slow-rise cut (`TUNING.up_surface_friction` is 1.0), following CS2.

### 3. Fewer acceleration steps per second than CS2

Air acceleration adds at most the 30 u/s cap per step, so the speed you can gain per second grows with the number of steps. Source 1 makes one step a tick. CS2 runs 64 ticks but splits a tick at the sub-tick times of your inputs; cs2kz's competitive mode rounds those split points to half ticks. At one step a tick and 64 ticks, this game gained less per second than CS2 (and less than CS:S servers at 100 tick, where kitsune was surfed).

**Now:** two air-acceleration steps a tick, the view turning from last tick's yaw to this tick's between them (`TUNING.air_substeps = 2`). This uses only the commands already recorded, so runs still replay to the bit. It is an approximation of CS2's sub-tick at cs2kz's resolution, not a copy of CS2 (whose code is not public). Test: `turning_in_the_air_gains_more_with_two_sub_steps`.

### 4. Snagging on geometry

CS2's July 2025 update "fixed several cases where a player would get stuck on map geometry while surfing"; cs2kz's reimplementation aims each later bump of a move 1/32 unit off the plane already hit. This game already had the rampbug fixes surf servers use (`docs/RESEARCH-surf-movement.md` section 6); it now also aims 1/32 unit off the last plane (`TUNING.snag_offset`), as CS2 does.

## Behaviours checked and left as they were

| Behaviour | Source 1 | CS2 | This game |
| --- | --- | --- | --- |
| Ground is a surface with normal z ≥ 0.7 | yes | yes (`sv_standable_normal` 0.7) | yes |
| Never grounded while rising faster than 140 u/s | yes | yes as far as known | yes |
| Air wish-speed cap | 30 | 30 (`sv_air_max_wishspeed`) | 30 |
| Overbounce on ramps | 1 (`sv_bounce` 0) | 1 | 1 |
| Ramp clipping (projection, one re-adjust) | yes | yes | yes |
| Teleport turns your view to the destination | yes | yes | yes |
| Landing on a walkable slope: better of clip and snap | random by sub-tick position | not known for vanilla CS2; cs2kz's own mode ports a slopefix | better of the two (RNGFix) |
| Tick | 64 to 128 by server | 64 plus sub-tick | 64, two air steps |

## Stage 2 itself

After the fix kitsune's stage 2 is as hard as the original, which is what Sam knows. It has not been made easier: no wedge is added, because none is missing. If it still feels too hard after playing with the speed carried over, a small entry wedge or a gentler gap is a one-file change in the extractor's output and should be a decision, not a default (`SECOND-ORDER.md` row 26).

Also changed: kitsune's see-through grid floors are drawn as netting again, so the ridge and the pit can be seen from the stage's start, as in the original.

## Sources

- Source SDK 2013, `src/game/server/triggers.cpp`, `CTriggerTeleport::Touch`: https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/game/server/triggers.cpp
- Source SDK 2013, `src/game/shared/gamemovement.cpp`, `CategorizePosition`, `AirAccelerate`: https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/game/shared/gamemovement.cpp
- CS2 November 2025 update notes (surf fixes): https://www.hotspawn.com/?p=142508
- CS2 release notes, 29 October 2024 (`sv_jump_precision_enable` for surf): https://devtrackers.gg/cs2/p/d88c2e96-release-notes-for-10-29-2024
- cs2kz-metamod, CS2 movement reimplementation (vanilla and CKZ modes): https://github.com/KZGlobalTeam/cs2kz-metamod
- CS2 surf server configs: SharpTimer `surf_.cfg`, CS2Surf/Timer `server_settings.cfg`
