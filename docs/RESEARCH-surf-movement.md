# Surf movement in Counter-Strike (Source engine): research notes for a browser reimplementation

Compiled 2026-10-13. Purpose: give an engineer enough exact detail to reimplement CS:S / CS:GO surf movement so that it feels right.

**How to read the verification tags**

- **[SRC]**: read directly in the source code listed (line-level).
- **[DOC]**: stated in documentation or a guide I fetched.
- **[DERIVED]**: my own algebra, or a numerical experiment on a port of the source code (Section 4.4). It is not a published claim.
- **[UNVERIFIED]**: community knowledge or memory that I could not confirm from a fetched source. Treat it as a hypothesis.

Valve's CS:S and CS:GO game DLL movement code (`cs_gamemovement.cpp`) is **not** in the public Source SDK 2013. The SDK ships the shared base class `CGameMovement`, which both games derive from. Every CS-specific override below is therefore inferred from cvars, wiki tables, or third-party reimplementations. I did not use leaked CS:GO source.

---

## 1. Sources

| # | Source | URL | Used for |
|---|---|---|---|
| S1 | Source SDK 2013, `gamemovement.cpp` at commit 0d8dcee (2015, `mp/` tree). This is the canonical "SDK 2013" movement code. | https://github.com/ValveSoftware/source-sdk-2013/blob/0d8dceea4310fde5706b3ce1c70609d72a38efdf/mp/src/game/shared/gamemovement.cpp | All core movement code |
| S2 | Source SDK 2013, current `master` (`src/game/shared/gamemovement.cpp`, after the 2025 TF2 SDK update) | https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/game/shared/gamemovement.cpp | Diffed against S1. The only movement changes are the new optional args `flSlideMultiplier` / `flRedirectCoeff`, which default to 0 and then do nothing. |
| S3 | SDK 2013 `gamemovement.h` | https://github.com/ValveSoftware/source-sdk-2013/blob/0d8dceea4310fde5706b3ce1c70609d72a38efdf/mp/src/game/shared/gamemovement.h | `GetAirSpeedCap() { return 30.f; }`, jump constants |
| S4 | SDK 2013 `movevars_shared.cpp` | https://github.com/ValveSoftware/source-sdk-2013/blob/0d8dceea4310fde5706b3ce1c70609d72a38efdf/mp/src/game/shared/movevars_shared.cpp | Base cvar defaults |
| S5 | SDK 2013 `gamerules.cpp` (`g_DefaultViewVectors`) | https://github.com/ValveSoftware/source-sdk-2013/blob/0d8dceea4310fde5706b3ce1c70609d72a38efdf/mp/src/game/shared/gamerules.cpp | Default hull and eye heights |
| S6 | SDK 2013 `public/coordsize.h` | https://github.com/ValveSoftware/source-sdk-2013/blob/0d8dceea4310fde5706b3ce1c70609d72a38efdf/mp/src/public/coordsize.h | `DIST_EPSILON`, `COORD_RESOLUTION` |
| S7 | SDK 2013 `mathlib_base.cpp` (`_VectorNormalize`) | https://github.com/ValveSoftware/source-sdk-2013/blob/0d8dceea4310fde5706b3ce1c70609d72a38efdf/mp/src/mathlib/mathlib_base.cpp | Normalising a zero vector gives a zero vector (`1/(r+FLT_EPSILON)`) |
| S8 | SDK 2013 `shareddefs.h` | https://github.com/ValveSoftware/source-sdk-2013/blob/0d8dceea4310fde5706b3ce1c70609d72a38efdf/mp/src/game/shared/shareddefs.h | `PLAYER_MAX_SAFE_FALL_SPEED` |
| S9 | QuakeWorld `pmove.c` (id Software) | https://github.com/id-Software/Quake/blob/master/QW/client/pmove.c | `PM_AirAccelerate` with `if (wishspd > 30) wishspd = 30;`, ClipVelocity with STOP_EPSILON |
| S10 | WinQuake `sv_user.c` | https://github.com/id-Software/Quake/blob/master/WinQuake/sv_user.c | Original Quake 1 `SV_AirAccelerate` 30 cap |
| S11 | Quake III `bg_pmove.c` / `bg_local.h` | https://github.com/id-Software/Quake-III-Arena/blob/master/code/game/bg_pmove.c , https://github.com/id-Software/Quake-III-Arena/blob/master/code/game/bg_local.h | Q3 contrasts: `OVERCLIP 1.001`, `MIN_WALK_NORMAL 0.7`, `pm_airaccelerate 1.0` |
| S12 | Adrian Biagioli, "Bunnyhopping from the Programmer's Perspective" (2015-02-14) | https://adrianb.io/2015/02/14/bunnyhop.html (old URL flafla2.github.io now redirects here) | Projection-limited acceleration explanation |
| S13 | Valve Developer Community, "Counter-Strike: Global Offensive/Mapper's Reference" (archived copy) | https://developer.valvesoftware.com/wiki/Counter-Strike:_Global_Offensive/Mapper%27s_Reference (fetched via web.archive.org because VDC blocks bots) | CS:GO hull and eye heights, 45.573° walkable slope, `sv_standable_normal` |
| S14 | VDC, "List of CS:GO Cvars" (archived 2023) | https://developer.valvesoftware.com/wiki/List_of_CS:GO_Cvars | CS:GO cvar names and defaults. **This table truncates fractional defaults to integers** (sv_friction 5.2 shows as "5"), which matters below. |
| S15 | VDC, "Sv_friction" (archived) | https://developer.valvesoftware.com/wiki/Sv_friction | friction 4 (CS/CS:S), 5.2 (CS:GO) |
| S16 | Total CS, sv_airaccelerate | https://totalcsgo.com/commands/svairaccelerate | CS:GO/CS2 default 12 |
| S17 | SurfTimer (CS:GO surf timer), `cfg/sourcemod/surftimer/main.cfg` | https://github.com/surftimer/SurfTimer/blob/master/cfg/sourcemod/surftimer/main.cfg | Surf server cvars as actually shipped |
| S18 | SurfTimer `SurfTimer.sp` (restores defaults on unload) and `surftimer/{convars,misc,hooks,surfzones,globals}.sp` | https://github.com/surftimer/SurfTimer/tree/master/addons/sourcemod/scripting | CS:GO defaults it restores, prespeed zone caps, zone types, sync attribution |
| S19 | shavit's bhoptimer, `shavit-core.sp` / `shavit-misc.sp` | https://github.com/shavitush/bhoptimer/tree/master/addons/sourcemod/scripting | Strafe sync and strafe count algorithm, prestrafe limit |
| S20 | shavit-ssj (Nairdaa), `shavit-ssj.sp` | https://github.com/Nairdaa/shavit-ssj/blob/main/shavit-ssj.sp | "Gain %", SSJ sync, efficiency formulas |
| S21 | Momentum Mod, `mom_gamemovement.cpp` (develop) | https://github.com/momentum-mod/game/blob/develop/mp/src/game/shared/momentum/mom_gamemovement.cpp | Rampbug analysis and fix (comments plus code) |
| S22 | Momentum Mod, `mom_system_gamemode.cpp/.h` | https://github.com/momentum-mod/game/blob/develop/mp/src/game/shared/momentum/mom_system_gamemode.cpp | Surf-mode cvars and tick interval (0.015) |
| S23 | GAMMACASE/MomSurfFix README | https://github.com/GAMMACASE/MomSurfFix | Port of the Momentum rampbug fix to CS:GO/CS:S servers |
| S24 | jason-e/rngfix README and `tech.md` | https://github.com/jason-e/rngfix , https://github.com/jason-e/rngfix/blob/master/tech.md | Landing, slope, edgebug, telehop and trigger quirks, and their tickrate dependence |
| S25 | Dignitas, "Beginner's guide to surfing in CS:GO" | https://dignitas.gg/articles/beginner-s-guide-to-surfing-in-csgo | Which key to hold on which side of a ramp |
| S26 | FreakHosting, "Setting up a surf server" (CS:S) | https://help.freakhosting.com/games/counter-strike-source/setting-up-a-surf-server.md | sv_airaccelerate 150 as the standard, 100 tick recommended for skill surf |
| S27 | Momentum Mod docs, "Fixing Entities" | https://docs.momentum-mod.org/guide/map_submission/fixing_entities/ | Teleport velocity modes ("Keep negative Z … generally only relevant to Surf") |
| S28 | CS2Fixes-RampbugFix (search result only, **not read**) | https://github.com/rcnoob/CS2Fixes-RampbugFix | Says the CS2 rampbug cause is unconfirmed |
| S29 | Knife speed 250: search summaries of cs.money / bo3.gg / csdb (**weak**) | https://cs.money/blog/esports/based-movement-in-csgo/ | CS:GO knife max speed 250 |

---

## 2. Constants and what says so

### 2.1 Engine and movement-code constants (shared Source base)

| Constant | Value | Source / note |
|---|---|---|
| Air wish-speed cap (`GetAirSpeedCap`) | **30** u/s | [SRC] S3 `virtual float GetAirSpeedCap( void ) { return 30.f; }`. Same as Quake 1/QW `if (wishspd > 30) wishspd = 30;` [SRC] S9, S10. |
| CS:GO `sv_air_max_wishspeed` | **30** | [DOC] S14 lists it with default 30 (replicated). That CS:GO's `GetAirSpeedCap()` returns this cvar is **[UNVERIFIED]**, but strongly implied by the name and value. |
| `MAX_CLIP_PLANES` | 5 | [SRC] S1 `#define MAX_CLIP_PLANES 5` |
| Bumps per TryPlayerMove | 4 | [SRC] S1 `numbumps = 4;`. Momentum raises this to 8 (`sv_ramp_bumpcount`) [SRC] S21. |
| `STOP_EPSILON` | 0.1 | [SRC] S1 defines it but Source's `ClipVelocity` **does not use it**. QW does, zeroing small components [SRC] S9. |
| `DIST_EPSILON` | 0.03125 (1/32) | [SRC] S6 `#define DIST_EPSILON (0.03125)`. This is the trace back-off from surfaces. |
| `COORD_RESOLUTION` | 1/32 | [SRC] S6 (used in `StayOnGround` snapping) |
| Ground / standable normal | `normal.z >= 0.7` is ground | [SRC] S1 `CategorizePosition`, `TryTouchGroundInQuadrants`, `StayOnGround`. In TryPlayerMove the "floor" test is `> 0.7`. acos(0.7) = **45.573°**: anything steeper is a surf ramp. [DOC] S13 says "Walkable if at most 45.573° steep", and that `sv_standable_normal` (cheat) can change it. Q3 uses the same `MIN_WALK_NORMAL 0.7f` [SRC] S11. |
| `NON_JUMP_VELOCITY` | 140 u/s | [SRC] S1 `CategorizePosition`: if `vel.z > 140` you are forced airborne even above walkable ground. [DOC] S24 explains why this matters for surf and bhop. |
| Ground probe distance | 2 units down | [SRC] S1 `flOffset = 2.0f` in CategorizePosition |
| `sv_optimizedmovement` shortcut | if `vel.z > 250` set ground = NULL at the start of PlayerMove (instead of a full CategorizePosition) | [SRC] S1 `PlayerMove` |
| Overbounce | `1.0 + sv_bounce*(1 - surfaceFriction)`, with sv_bounce default 0, so **1.0** | [SRC] S1, S4 (`sv_bounce "0"`); [DOC] S14 (CS:GO sv_bounce 0) |
| Gravity integration | half step before the move, half after | [SRC] S1 `StartGravity` / `FinishGravity` ("yes, this 0.5 looks wrong, but it's not") |
| `sv_maxvelocity` clamp | **per axis**, ±3500 | [SRC] S1 `CheckVelocity` clamps each of x, y, z separately. [SRC] S4 default 3500. [DOC] S14 CS:GO 3500 ("per axis"). |
| `sv_stepsize` | 18 | [SRC] S4 |
| Surface friction in air while moving up | `m_surfaceFriction = 0.25` when not grounded and `vel.z > 0` | [SRC] S1 `CategorizePosition`. It scales `accelspeed` in AirAccelerate, but only when the player is airborne **and** rising after the last categorise. This is a subtle point; see 3.4. |
| Jump (SDK base, non-HL2) | 268.3281573 = sqrt(2·800·45) | [SRC] S1 `CheckJumpButton` (`flMul = 268.3281572999747f` with `Assert(gravity == 800)`). CS games override this. |

### 2.2 Cvar defaults by game

| Cvar | SDK 2013 base [SRC] S4 | CS:S | CS:GO | Typical CS:GO surf server |
|---|---|---|---|---|
| sv_gravity | 800 (non-HL2) | 800 [UNVERIFIED but universal] | 800 [DOC] S14 | 800 [DOC] S17 |
| sv_airaccelerate | 10 | 10 [DOC] S26 ("vanilla default of 10") | **12** [DOC] S14, S16 | **150** [DOC] S17. S26 calls 150 "the standard starting point". Momentum surf also uses 150 [SRC] S22. |
| sv_accelerate | 10 (non-HL2) | 5 per Momentum's CS:S-style base [SRC] S22, otherwise [UNVERIFIED] | **5.5**. S14 shows "5" (truncated) with the note "old value is 5.6"; SurfTimer restores `sv_accelerate 5.5` on unload [SRC] S18. | **10** [DOC] S17 |
| sv_friction | 4 | 4 [DOC] S15 | **5.2** [DOC] S15 | **5.2** [DOC] S17 |
| sv_stopspeed | 100 | 75 in Momentum base [SRC] S22, otherwise [UNVERIFIED] | 80 [DOC] S14 | (not set, so 80) |
| sv_maxspeed | 320 | 320 [UNVERIFIED] | 320 [DOC] S14 | **350** [DOC] S17. The effective air wishspeed is still the player's weapon speed (knife 250); see below. |
| player max speed (knife) | n/a | 250 [UNVERIFIED]; Momentum uses 260 [SRC] S22 | **250** [DOC, weak] S29 | 250 |
| sv_maxvelocity | 3500 | 3500 | 3500 [DOC] S14 | 3500 (SurfTimer's per-map menu offers 3500/4000/5000/10000) [SRC] S18 |
| sv_bounce | 0 | 0 | 0 [DOC] S14 | 0 |
| sv_enablebunnyhopping | n/a | 0 | 0 ("Allow player speed to exceed maximum running speed") [DOC] S14 | **1** [DOC] S17 |
| sv_autobunnyhopping | n/a | n/a | 0 [DOC] S14 | SurfTimer: `ck_auto_bhop 1` [DOC] S17 |
| sv_staminamax / jumpcost / landcost | n/a | n/a | 80 / 0.08 / 0.05 [DOC] S14, S18 | **0 / 0 / 0** [DOC] S17 |
| sv_clamp_unsafe_velocities | n/a | n/a | 1 [DOC] S14 | **0** [DOC] S17 |
| sv_jump_impulse | n/a | [UNVERIFIED] | 301.993377 = sqrt(2·800·57). S14 shows "301" with the description "sqrt(2*gravity*height)"; the 57-unit figure is [DERIVED]. | same |
| sv_standable_normal / sv_walkable_normal | n/a | n/a | 0.7 / 0.7. S14 shows "0" (truncated); S13 says "under 0.7" makes steeper slopes walkable. | same |
| sv_ladder_scale_speed | n/a | n/a | 0.78 [UNVERIFIED]; S14 shows "0" (truncated) | 1 [DOC] S17 |
| sv_wateraccelerate | 10 | 10 | [UNVERIFIED] | (not set) |

### 2.3 Player hull and eyes

| | SDK 2013 default [SRC] S5 | CS:GO [DOC] S13 |
|---|---|---|
| Standing hull | (-16,-16,0)–(16,16,72) | 32 × 32 × **72** |
| Ducked hull | (-16,-16,0)–(16,16,36) | 32 × 32 × **54** |
| Eye (view offset) standing | 64 | 64.093811 (S13 lists "feet level 0.031250", which is roughly the DIST_EPSILON hover) |
| Eye ducked | 28 | 46.076218 |

The origin is at the **feet** (mins.z = 0). CS:S's ducked hull height is [UNVERIFIED]: I believe it is 45 in CS:S versus 54 in CS:GO, but I did not confirm this.

### 2.4 Derived per-tick numbers [DERIVED]

`accelspeed = airaccel · wishspeed · dt` (wishspeed is the **uncapped** 250, not 30; see 3.3):

| tick | dt | aa=12, ws=250 | aa=150, ws=250 |
|---|---|---|---|
| 64 | 0.015625 | 46.9 | 585.9 |
| 66.67 | 0.015 | 45.0 | 562.5 |
| 100 | 0.01 | 30.0 | 375.0 |
| 128 | 0.0078125 | 23.4 | 293.0 |

Whenever `accelspeed >= 30 - v·wishdir`, AirAccelerate simply **sets the velocity component along wishdir to 30**. At aa=150 that holds for any `v·wishdir > -556` at 64 tick, which covers essentially always. This is why surf air control feels "snappy" and forgiving.

---

## 3. Per-tick movement, faithful to Source

All code is paraphrased from S1 with names kept. `dt = gpGlobals->frametime = 1/tickrate` (times `m_flLaggedMovementValue`, which is 1 normally). Units are Hammer units, seconds, and degrees.

### 3.1 Input → wish (CheckParameters + AirMove front half)

```
// usercmd: forwardmove (W=+cl_forwardspeed, S=-), sidemove (D=+cl_sidespeed, A=-), both 450 in CS:GO [UNVERIFIED value; sign convention SRC]
// CheckParameters():
maxspeed = min(client_maxspeed /*weapon, knife 250*/, sv_maxspeed) * surfaceMaxSpeedFactor
spd = |(fmove, smove, umove)|
if spd > maxspeed: scale fmove, smove, umove by maxspeed/spd

// AirMove():
forward, right = AngleVectors(viewangles)      // pitch is included here, then...
forward.z = 0; right.z = 0; normalize both       // ...thrown away: ONLY YAW MATTERS in air
wishvel = forward*fmove + right*smove            // wishvel.z = 0
wishdir = normalize(wishvel); wishspeed = |wishvel|
if wishspeed != 0 and wishspeed > maxspeed: wishvel *= maxspeed/wishspeed; wishspeed = maxspeed
AirAccelerate(wishdir, wishspeed, sv_airaccelerate)
velocity += baseVelocity                         // trigger_push / conveyors
TryPlayerMove()
velocity -= baseVelocity
```

Edge case [SRC]: at pitch exactly ±90°, `forward` with z zeroed is the zero vector, and normalising it gives zero (S7), so W/S do nothing. This does **not** explain "looking up kills speed". That effect comes from yaw (see 4.3).

### 3.2 FullWalkMove (the per-tick driver, not in water)

```
PlayerMove():
    CheckParameters()
    if movetype != WALK or gameCodeMovedPlayer or !sv_optimizedmovement: CategorizePosition()
    else if velocity.z > 250: SetGroundEntity(NULL)
    if ground == NULL: fallVelocity = -velocity.z
    Duck(); LadderMove() ...
    FullWalkMove()

FullWalkMove():
    StartGravity()                       // vz -= 0.5*g*dt; vz += baseVel.z*dt; baseVel.z = 0; CheckVelocity()
    if IN_JUMP: CheckJumpButton()        // happens BEFORE friction: jumping on the landing tick skips friction entirely
    else oldbuttons &= ~IN_JUMP
    if ground != NULL:
        velocity.z = 0
        Friction()
    CheckVelocity()                      // per-axis clamp to ±sv_maxvelocity, NaN -> 0
    if ground != NULL: WalkMove()        // Accelerate + StepMove + StayOnGround
    else:              AirMove()         // AirAccelerate + TryPlayerMove
    CategorizePosition()                 // decide grounded or airborne for NEXT tick
    CheckVelocity()
    FinishGravity()                      // vz -= 0.5*g*dt; CheckVelocity()
    if ground != NULL: velocity.z = 0
    CheckFalling()                       // landing sounds and damage only
```

On a surf ramp (`normal.z < 0.7`) the player is **never** grounded. Every tick takes the AirMove path, friction never applies, and gravity is the full `g·dt` per tick, split half and half around the move.

### 3.3 AirAccelerate [SRC] S1 (identical in structure to QW `PM_AirAccelerate` S9)

```
AirAccelerate(wishdir, wishspeed, accel):
    if dead or waterjumping: return
    wishspd = min(wishspeed, AIR_CAP)              // AIR_CAP = 30 (GetAirSpeedCap / CS:GO sv_air_max_wishspeed)
    currentspeed = dot(velocity, wishdir)          // 3D dot, but wishdir.z = 0, so this is horizontal
    addspeed = wishspd - currentspeed
    if addspeed <= 0: return
    accelspeed = accel * wishspeed * dt * surfaceFriction   // NOTE: UNCAPPED wishspeed (e.g. 250) here
    accelspeed = min(accelspeed, addspeed)
    velocity += accelspeed * wishdir
```

The asymmetry matters: the **cap** uses 30 but the **rate** uses 250. Use the uncapped value for the rate or air control will feel about 8× too weak.

`surfaceFriction` is reset to 1.0 in every CategorizePosition. It is set to 0.25 when airborne with `vel.z > 0` (after the ground test fails). Rising off a ramp therefore cuts the air accel *rate* to a quarter, but with aa=150 the rate rarely binds (585×0.25 = 146 > 30). [SRC] S1; consequence [DERIVED].

### 3.4 Ground Accelerate and Friction (for bhop and start zones)

```
Accelerate(wishdir, wishspeed, accel):        // same as above but no 30 cap
    addspeed = wishspeed - dot(velocity, wishdir); if addspeed <= 0: return
    accelspeed = min(accel*dt*wishspeed*surfaceFriction, addspeed)
    velocity += accelspeed*wishdir

Friction():                                   // only if grounded
    speed = |velocity|; if speed < 0.1: return
    control = max(speed, sv_stopspeed)
    drop = control * sv_friction * surfaceFriction * dt
    velocity *= max(speed - drop, 0) / speed

WalkMove(): velocity.z = 0; Accelerate(wishdir, wishspeed, sv_accelerate); velocity.z = 0;
            if |v| < 1: v = 0; return
            try a flat move to origin + v*dt; if it works: move, StayOnGround(); else StepMove() (up 18, over, down)
```

### 3.5 ClipVelocity [SRC] S1

```
ClipVelocity(in, normal, out, overbounce):     // returns blocked flags (1 = floor if normal.z > 0, 2 = wall if normal.z == 0)
    backoff = dot(in, normal) * overbounce
    out = in - normal * backoff
    adjust = dot(out, normal)                  // "iterate once to make sure we aren't still moving through the plane"
    if adjust < 0: out -= normal * adjust
```

With overbounce = 1 this is a pure projection onto the plane: the normal component is removed and nothing is added. Quake 3 instead uses `OVERCLIP = 1.001`, which pushes slightly away from the plane [SRC] S11. QW zeroes components with |x| < 0.1 [SRC] S9. Source does neither. For a Source feel, use overbounce exactly 1 plus the single re-adjust.

The 2025 SDK adds `flRedirectCoeff` (default 0). When it is > 0 it rescales `out` to keep part of the blocked speed. CS games do not use it; ignore it [SRC] S2.

### 3.6 TryPlayerMove (the slide loop) [SRC] S1

```
TryPlayerMove():
    numbumps = 4; numplanes = 0; blocked = 0
    original_velocity = primal_velocity = velocity
    allFraction = 0; time_left = dt
    for bump in 0..numbumps-1:
        if |velocity| == 0: break
        end = origin + time_left * velocity
        pm = TraceHull(origin, end, playerMins, playerMaxs)       // swept AABB vs world, ends DIST_EPSILON short of surfaces
        allFraction += pm.fraction
        if pm.allsolid: velocity = 0; return 4                   // trapped
        if pm.fraction > 0:
            if pm.fraction == 1:                                 // full move: re-check an UNSWEPT box at the end point
                stuck = TraceHull(pm.endpos, pm.endpos)
                if stuck.startsolid or stuck.fraction != 1:      // "precision issue with terrain tracing"
                    velocity = 0; break                          // <-- one source of rampbugs
            origin = pm.endpos
            original_velocity = velocity
            numplanes = 0
        if pm.fraction == 1: break                               // done
        AddToTouched(pm)
        if pm.normal.z > 0.7: blocked |= 1
        if pm.normal.z == 0:  blocked |= 2
        time_left -= time_left * pm.fraction
        if numplanes >= MAX_CLIP_PLANES: velocity = 0; break
        planes[numplanes++] = pm.normal

        if numplanes == 1 and movetype == WALK and ground == NULL:
            // FIRST plane while airborne (the normal surfing case):
            for each plane i (just one):
                if planes[i].z > 0.7: ClipVelocity(original_velocity, planes[i], new_velocity, 1)
                                      original_velocity = new_velocity
                else:                 ClipVelocity(original_velocity, planes[i], new_velocity,
                                                   1 + sv_bounce*(1 - surfaceFriction))   // = 1
            velocity = original_velocity = new_velocity
        else:
            // multi-plane (or grounded) case: find a plane whose clip doesn't push into the others
            for i in 0..numplanes-1:
                ClipVelocity(original_velocity, planes[i], velocity, 1)
                if for all j != i: dot(velocity, planes[j]) >= 0: break
            if i == numplanes:                                   // no single plane works: slide along the crease
                if numplanes != 2: velocity = 0; break
                dir = normalize(cross(planes[0], planes[1]))     // zero if the planes are (nearly) identical!
                velocity = dir * dot(dir, velocity)
            if dot(velocity, primal_velocity) <= 0:              // reversed: stop dead
                velocity = 0; break
    if allFraction == 0: velocity = 0                            // never moved at all this tick
    (slam-sound bookkeeping)
    return blocked
```

### 3.7 CategorizePosition [SRC] S1

```
CategorizePosition():
    surfaceFriction = 1.0
    CheckWater()
    point = origin - (0,0,2)
    movingUpRapidly = velocity.z > 140 (minus ground entity's vz if on a mover)
    if movingUpRapidly (or moving up on a ladder): SetGroundEntity(NULL)
    else:
        pm = TraceHull(origin, point)
        if !pm.hit or pm.normal.z < 0.7:
            pm = TryTouchGroundInQuadrants(origin, point)        // four half-size boxes, each looking for normal.z >= 0.7
            if !pm.hit or pm.normal.z < 0.7:
                SetGroundEntity(NULL)
                if velocity.z > 0 and movetype != NOCLIP: surfaceFriction = 0.25
            else SetGroundEntity(pm)
        else SetGroundEntity(pm)

SetGroundEntity(pm):     // landing: velocity.z = 0 immediately (NO clip against the slope)
```

The `velocity.z = 0` on landing, together with the 2-unit snap distance, is the root of the "random" slope-landing behaviour that RNGFix and slopefix address (Section 6).

### 3.8 CS:GO-specific differences (what is known)

- The air cap is a cvar, `sv_air_max_wishspeed` = 30 [DOC] S14. Implement it as a constant of 30 unless you want to tune it.
- Default `sv_airaccelerate` is 12 and `sv_accelerate` 5.5. Friction is 5.2 and stopspeed 80 [DOC] S14, S15, S18. **Surf servers override airaccelerate to 150**, so the 12 never matters for surf.
- Ducked hull is 54 (not 36), with eyes at 64.09 / 46.08 [DOC] S13.
- Stamina (`sv_staminamax 80`, jump and land costs) slows you after jumps in stock CS:GO. Surf servers zero it [DOC] S17.
- `sv_enablebunnyhopping 0` caps speed on jumps in stock CS:GO; surf servers set it to 1 [DOC] S14, S17. The exact CS:GO cap formula is [UNVERIFIED]; I believe it is about 1.1× max speed, but did not confirm.
- Weapon-dependent max speed (knife 250) feeds `m_flMaxSpeed`, and therefore `wishspeed` [DOC, weak] S29.
- Whether CS:GO's TryPlayerMove or CategorizePosition differ from SDK 2013 is **[UNVERIFIED]**. The S13 table shows tick-dependent jump heights (54 vs 55 units at 64 vs 128 tick), which is consistent with the same discrete integrator.

---

## 4. Why surfing works and where speed comes from

### 4.1 Energy bookkeeping [DERIVED from the code above]

Let `e = |v|²/2 + g·z` (specific mechanical energy). Each tick, per step:

- **Gravity** (half step, move, half step) conserves `e` up to integrator error. Falling converts height into speed one-for-one: `|v|² = |v0|² + 2·g·Δh`.
- **ClipVelocity** with overbounce 1 removes the normal component: `Δ|v|² = -(v·n)²` (always ≤ 0). Grazing contact costs almost nothing. Hitting a surface head-on costs a lot.
- **AirAccelerate**: let `u = v·wishdir` (before) and `a = min(accelspeed, 30 - u)`. Then `Δ|v|² = 2ua + a²`. When `a = 30 - u` (always true on surf servers, per Section 2.4), `Δ|v|² = 900 - u²`. The **maximum is 900 (u/s)² per tick, reached when wishdir is exactly perpendicular to velocity (u = 0)**. This matches Biagioli's point that "only the projection of the current velocity onto acceleration is limited" [DOC] S12.
- **Friction** applies only on ground ticks. Ramps are never ground.

So: **surf speed = height lost + (strafe gain ≤ 900 per tick) − (clip losses)**. The 900-per-tick budget is worth `900·tickrate / (2g)` units of "free height" per second: **36 u/s at 64 tick, 37.5 at 66.67, 56 at 100, 72 at 128** [DERIVED]. Most surf speed comes from **descending ramps**. Air strafing adds a modest, tick-dependent bonus and steers. Skill means losing as little as possible to clips while trading height for speed efficiently.

### 4.2 What the ramp does geometrically

Take a ramp with slope θ > 45.57° (normal.z < 0.7). Put its fall line in the y–z plane, with the ramp rising toward +y: `n = (0, -sin θ, cos θ)`. The player travels along +x.

1. **The ramp is a frictionless inclined plane.** Gravity pulls you into the ramp. ClipVelocity removes the into-ramp part and keeps the tangential part, so you accelerate *down the fall line* at `g·sin θ`. That down-slope velocity is perpendicular to your +x travel. On its own it slides you off the bottom of the ramp.
2. **Holding the strafe key toward the top of the ramp** (A when the ramp face rises to your left, i.e. "if you are on the right side of the ramp, you will continuously hold the A key" [DOC] S25) gives a horizontal wishdir `w = (0,1,0)` into the ramp. Sliding down-slope makes `u = v·w` negative, so AirAccelerate adds `30 - u` along `+y`: it **cancels the horizontal outward drift and sets that component to +30**. That push points into the ramp, so ClipVelocity strips its normal part. What survives is `w - (w·n)n = cos θ · (0, cos θ, sin θ)`, which points **up the fall line**. Net effect: the key holds you at a chosen height on the ramp instead of sliding off.
3. **The mouse (yaw) decides how much height to trade for speed.** The wishdir for A is the view's left vector. Rotating the view toward the bottom of the ramp tilts wishdir forward, `u` rises above 30, AirAccelerate stops adding, and gravity takes you down the ramp, gaining speed. Rotating the view toward the top makes wishdir point backward-into-ramp, and velocity gets turned up the ramp. You climb and lose speed. Riding a ramp well means continuously steering this balance. [DERIVED, confirmed numerically in 4.4]
4. **Air between ramps** is ordinary air strafing. Alternate A/D in step with the mouse so that wishdir stays nearly perpendicular to velocity (u ≈ 0, maximum 900/tick gain) while yaw rotates with the velocity. The optimal turn per tick is about `atan(30/|v|)`: 2.9°/tick at 600 u/s, and 0.86°/tick at 2000 u/s [DERIVED].

### 4.3 Why not to press W

With W held and yaw roughly along velocity, wishdir ≈ velocity direction, so `u ≈ |v| ≫ 30` and AirAccelerate adds **nothing**. With W+A, the diagonal wishdir still has `u ≈ |v|·cos45° ≫ 30`, which again adds nothing. So W does not brake you directly in the air. It **cancels the into-ramp hold** that A or D was providing, and you slide off the ramp. My simulation shows "W only" and "W+A" are identical to "no keys" [DERIVED]. Guides describe this as W killing momentum or sliding you off the bottom. I saw this only in search-result summaries of csdb.gg guides, and S25 does not mention W, so the guide wording is [UNVERIFIED]. The mechanism is the 30-cap projection. Pitch is irrelevant in air (z is zeroed in AirMove), so the "don't aim up" advice is really about yaw and steering into ramps.

### 4.4 Numerical check [DERIVED]

I ported FullWalkMove's air path (half-step gravity, AirAccelerate with the uncapped rate, a 4-bump TryPlayerMove with first-plane overbounce-1 clip, and allFraction zeroing) against one infinite plane. Start speed is 1000 u/s along the ramp, sv_airaccelerate 150, wishspeed 250, run for 2 s:

| ramp | tick | input | speed after 2 s | Δz |
|---|---|---|---|---|
| 55° | 64 | no keys / W / W+A | 1648.6 | −1073.6 (energy exactly conserved) |
| 55° | 64 | A, yaw = velocity dir | 365.0 | +570.8 (turned up the ramp, climbed) |
| 55° | 64 | A, yaw 1° toward bottom | 1274.7 | −358.7 |
| 65° | 64 | A, yaw 1° toward bottom | 1497.5 | −745.7 |
| 65° | 128 | A, yaw 1° toward bottom | 1147.7 | −130.5 |

Free-air strafing (600 u/s start, 1 s, A held, constant turn rate):

| tick, aa | 1°/tick | 2°/tick | 3°/tick | 5°/tick | 8°/tick |
|---|---|---|---|---|---|
| 64, aa=12 | 628 | 642 | 644 | 495 | 193 |
| 64, aa=150 | 628 | 642 | 645 | 618 | 519 |
| 128, aa=12 | 655 | 678 | 372 | 202 | 157 |
| 128, aa=150 | 655 | 684 | 687 | 633 | 478 |

So the peak gain does **not** depend on airaccelerate, but **tolerance to over-turning** does. aa=150 makes mistakes cheap, which is what makes surf playable, especially at higher tick. Higher tick gives more total gain per second.

---

## 5. Sync, gain, strafes: definitions and how timers compute them

### 5.1 Strafe sync, shavit's bhoptimer [SRC] S19; SurfTimer copies it ("Strafe Sync taken from shavit's bhoptimer") [SRC] S18

This is evaluated per server tick in `OnPlayerRunCmd`, using `vel[]` = the usercmd's (forwardmove, sidemove) and `angles[1]` = yaw:

```
yawdelta = AngleDiff(yaw, lastYaw)              // > 0 means turning left
if airborne and not ladder and not in water and yawdelta != 0 and horizontalSpeed > 0:
    dirangle = |viewYaw(0..360) - velocityYaw|  // classifies the style (forward, sideways, half-sideways, backwards)
    // normal / forward style (dirangle < 22.5 or > 337.5):
    totalMeasures++
    if (yawdelta > 0 and sidemove <= -100) or (yawdelta < 0 and sidemove >= 100):
        goodGains++                              // turning left while holding A, or right while holding D
sync% = goodGains / totalMeasures * 100
```

Sync is the share of airborne ticks **with mouse movement** in which the held strafe key matches the mouse's turn direction. Ticks without yaw change are ignored. Pressing both keys, or neither, while turning counts as bad.

### 5.2 Strafe count [SRC] S19

In the air, each **new press** of a strafe key counts as one strafe: a transition of sidemove from ≥ 0 to < 0 for A, or ≤ 0 to > 0 for D (W and S are counted only for some styles). SSJ counts rising edges of IN_MOVELEFT / IN_MOVERIGHT / IN_FORWARD / IN_BACK button bits [SRC] S20.

### 5.3 "Gain" (SSJ plugin, used alongside shavit) [SRC] S20

```
each airborne tick (strafeTick++):
    wishdir, wishspeed from (forwardmove, sidemove) and yaw, exactly like AirMove; cap wishspeed at m_flMaxspeed
    if wishspeed > 0:
        wishspd = min(wishspeed, 30)
        currentgain = dot(horizontalVelocity, wishdir)       // = u
        if currentgain < 30:
            syncedTick++
            gaincoeff = (wishspd - |currentgain|) / wishspd  // 1.0 when wishdir ⟂ velocity
        (wall-touch correction)
        rawGain += gaincoeff
Gain%  = rawGain / strafeTicks * 100
SSJ Sync% = syncedTicks / strafeTicks * 100                   // ticks where AirAccelerate actually added speed
Efficiency% = Gain% * |net displacement| / path length
```

Gain% measures how close wishdir stayed to perpendicular, normalised against the 30 cap. It is linear in |u| (the true per-tick gain is `900 − u²`), so it is a proxy, not exact energy.

KZTimer's definitions were **not checked** [UNVERIFIED].

---

## 6. Known quirks: reproduce or avoid?

| Quirk | Cause (source) | Recommendation |
|---|---|---|
| **Rampbug** (sudden stop or large speed loss while riding a ramp; "vertical rampbug" when it happens going down) | Several zero-velocity exits in TryPlayerMove [SRC] S1: (a) the unswept "stuck" retrace after a full-fraction move; (b) `allFraction == 0`; (c) the crease branch: if the two clip planes are (nearly) identical, `cross()` ≈ 0, and normalising it gives 0 (S7), so velocity becomes 0. Momentum's comment says this happens when "you can 'clip' into the ramp … So the game thinks the surfer is clipping along two of the exact same planes" [SRC] S21. (d) The `dot(v, primal) <= 0` stop. (e) Running out of planes or bumps. These are triggered by trace precision at brush seams, adjoining ramp brushes, and displacements. That trigger is a community consensus, not a Valve statement. My own port reproduced (b) from float noise when the player sits exactly at the DIST_EPSILON offset [DERIVED]. CS2's cause is officially unconfirmed per S28 [DOC, not read]. | **Avoid.** It is the most-hated surf bug, and servers run MomSurfFix [DOC] S23. Use robust collision: dedupe near-parallel planes (if `dot(n0,n1) > 0.999`, treat them as one plane), keep a consistent skin distance, never zero velocity on allFraction==0 while airborne, and use 8 bumps. Momentum's fix pushes 20 u/s along the plane normal (x/y only) when the planes are identical [SRC] S21. |
| **Ramp landing / slope landing RNG**: landing on a walkable slope sometimes gives a boost and sometimes doesn't | CategorizePosition's 2-unit snap sets `vz = 0` without clipping against the slope [SRC] S1. Whether you collided first (clip, which keeps speed) or snapped (vz discarded) depends on sub-tick position. Downhill misses are more frequent at higher tick, uphill losses at lower tick [DOC] S24. | **Avoid the randomness.** Do what slopefix/RNGFix do: on landing, apply the ClipVelocity result if it gives more horizontal speed. |
| **Sliding on walkable slopes** (normal.z ≥ 0.7, but vz > 140 makes them act as a surf ramp) | NON_JUMP_VELOCITY [SRC] S1; [DOC] S24 | **Reproduce.** It is part of the feel, and mappers rely on it. |
| **Bhop on flat ground / "bhop onto ramp"** | CheckJumpButton runs **before** Friction in FullWalkMove. Jumping on the landing tick skips friction, so there is no speed loss [SRC] S1. Surf ramps themselves cannot be jumped from, because you are never grounded. Surf servers use autobhop [DOC] S17. | **Reproduce** (autobhop on). Optionally add RNGFix's uphill/downhill determinism. |
| **Edgebug** (land on the trailing edge, lose vz, no jump) | Collision with the walkable top during the tick, ending off the edge [DOC] S24 | Mostly irrelevant to surf. Avoid, or make it deterministic. |
| **Telehop** (hit the wall behind a thin teleport before the trigger fires) | Triggers fire in ProcessImpacts **after** the whole move [DOC] S24 | **Avoid.** Fire triggers along the swept path, or restore pre-collision velocity like RNGFix. |
| **Trigger jumping** (thin floor triggers missed) | Grounded up to 2 units above the floor [DOC] S24 | **Avoid** (extend the touch test down to the floor). |
| **Stair sliding** (surf only) | StepMove lets fast players slide up stairs; hitting a riser face first kills speed [DOC] S24 | Reproduce the slide, avoid the riser loss (RNGFix behaviour). |
| **"Wall-bug" / ramp–wall pinch** | Hitting a wall (normal.z == 0) clips away `(v·n)²`. In V-shaped ramp/wall creases the crease branch or the reverse check zeroes velocity [SRC] S1. The exact community meaning of "wallbug" is [UNVERIFIED]. | Keep the loss from hitting walls (it is fair). Avoid the zero-velocity crease cases. |
| **Per-axis sv_maxvelocity** | CheckVelocity clamps x, y, z separately [SRC] S1, so diagonal horizontal speed can reach 3500·√2 ≈ 4950 | Reproduce for fidelity, or clamp the vector length if you prefer predictability. Note: speedometers show "3500" caps along axes. |
| **Gravity half-steps** | StartGravity/FinishGravity [SRC] S1 | **Reproduce.** It makes jump arcs and ramp exits tick-consistent. |
| **surfaceFriction 0.25 while rising** | CategorizePosition [SRC] S1 | Reproduce (cheap). It slightly reduces air control rate right after launching off something. |
| **Pitch ±90 null forward** | AirMove zeroes z, then normalises [SRC] S1 | Harmless, but clamp pitch to ±89 like CS. |
| **Ducking in air** | Changes the hull 72 → 54 (CS:GO) | Reproduce if you support duck. The surf-specific details (duck-jump hull shift) are [UNVERIFIED]. |

### Surf map conventions (for content, not physics)

- **Fail triggers:** a `trigger_teleport` volume under each section returns the player to the current stage start or checkpoint. Speed handling on teleport varies: Momentum offers keep / reset / "keep negative Z", and calls the last "generally only relevant to Surf" [DOC] S27. Mappers also build speed-killing "teleport hubs" (teleport into a small box, then teleport again) [DOC] S24.
- **Staged vs linear:** SurfTimer zone types are Start, End, **Stage**, **Checkpoint**, SpeedStart, TeleToStart, Validator, Checker, AntiJump, AntiDuck, MaxSpeed, Stop [SRC] S18. Staged maps use Stage zones (each stage timed and restartable). Linear maps use Checkpoints only. Bonuses are separate zone groups.
- **Start-zone speed caps:** SurfTimer caps horizontal speed inside a start zone to a per-zone `PreSpeed`. The default is `ck_default_prespeed 260`, and the admin menu offers 250 / 260 / 285 / 300 / **350** / 500 / custom [SRC] S18. shavit limits prestrafe to `runspeed + shavit_misc_prestrafelimit` = 260 + 30 = **290** by default, and divides the limit by 3 if you jump in the zone (prespeed modes 1–6) [SRC] S19.

---

## 7. Recommended tick rate and constants for a CS:GO surf-server feel

**Tick rate.** Run physics at a **fixed** step with render interpolation. Never use variable dt: every rule above is per tick, and strafe gain per second scales with tick rate (Section 4.1).

- **Default: 64 tick (dt = 1/64)** for CS:GO feel. Offer 66.67 (dt = 0.015, the CS:S default, which Momentum's Surf mode uses [SRC] S22) and 100 or 128 as options.
- Many CS:GO surf servers ran above 64 (85.3 / 102.4 / 128 appear in server names; a host guide for SurfTimer says to use 102.4; S26 recommends 100 for CS:S skill surf "for more precise ramp collisions"). I could not verify which was most common, so treat it as [UNVERIFIED]. At higher tick, strafing gains more and ramp collisions are smoother, but RNGFix notes that some landing bugs become *more* frequent.

**Constants (CS:GO surf server)**

```
TICK                 = 64          (configurable 64 / 66.67 / 100 / 128)
GRAVITY              = 800         sv_gravity
AIR_ACCELERATE       = 150         sv_airaccelerate (surf)        [stock CS:GO 12]
AIR_WISHSPEED_CAP    = 30          sv_air_max_wishspeed / GetAirSpeedCap
ACCELERATE (ground)  = 10          sv_accelerate (SurfTimer)      [stock CS:GO 5.5]
FRICTION             = 5.2         sv_friction (CS:GO and SurfTimer)
STOPSPEED            = 80          sv_stopspeed (CS:GO)
PLAYER_MAXSPEED      = 250         knife; this is the wishspeed used in Accelerate/AirAccelerate
SV_MAXSPEED          = 350         (only an upper bound; the 250 knife speed binds)
MAX_VELOCITY         = 3500        per axis
FORWARD/SIDE_SPEED   = 450         input magnitude before clamping to 250 [UNVERIFIED CS:GO value; clamped anyway]
JUMP_IMPULSE         = 301.993377  sqrt(2*800*57), CS:GO
AUTOBHOP             = on ; STAMINA = off ; BUNNYHOP CAP = off
STANDABLE_NORMAL_Z   = 0.7         (ramps steeper than 45.573° are surfable)
NON_JUMP_VELOCITY    = 140
GROUND_PROBE         = 2 units
STEP_SIZE            = 18
OVERBOUNCE           = 1.0         (sv_bounce 0)
DIST_EPSILON         = 0.03125     collision skin
MAX_CLIP_PLANES      = 5 ; BUMPS = 4 (use 8 with the anti-rampbug fixes)
HULL                 = 32x32x72 standing, 32x32x54 ducked, origin at feet
EYE                  = 64 standing (64.09), 46 ducked (46.08)
START ZONE CAP       = 260–350 u/s horizontal (map-configurable; shavit default 290)
```

**Implementation checklist for the right feel**

1. Order the tick exactly as in FullWalkMove (3.2): half gravity → jump → friction (if grounded) → move → categorize → half gravity.
2. In AirAccelerate, cap the *target* at 30 but use the *uncapped* wishspeed for the rate.
3. Only yaw drives wishdir. sidemove sign: A is negative, along the right vector.
4. Use a swept-AABB vs. convex-brush or triangle collision with a 1/32 skin. Do projection-only clipping with overbounce 1 and one re-adjust. Do not use Q3's 1.001 overclip; it adds a slight push-off that feels "bouncy".
5. Ground requires normal.z ≥ 0.7 **and** vz ≤ 140.
6. Ship the rampbug mitigations (Section 6) from day one. Players will forgive many things, but not rampbugs.
7. Make slope landings deterministic (take the better of clip vs. snap).
8. Per-axis velocity clamp at 3500 if you want CS:GO-exact top speeds.

---

### Open questions / not verified

- The exact CS:GO `CCSGameMovement` overrides: the TryPlayerMove variant, the duck-jump hull handling, and the bunnyhop cap formula.
- The exact CS:S defaults for sv_accelerate, sv_stopspeed, the ducked hull and the jump impulse. Momentum's CS:S-style base (accelerate 5, stopspeed 75, maxspeed 260) is the best public proxy.
- Which tick rate the big CS:GO surf networks used, and KZTimer's sync definition.
- The cl_forwardspeed / cl_sidespeed values in CS:GO. They are irrelevant in practice because input is clamped to the 250 max speed.
