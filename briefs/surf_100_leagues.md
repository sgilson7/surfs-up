# Brief: surf_100_leagues (the undersea scuba map)

Given by the person on 16 October 2026, filled in on the design log's "Your first map" tab. Their words are kept exactly below; the answers and notes after them are the builder's. Read `.claude/skills/themed-map/SKILL.md` first.

## The person's description (verbatim)

> # Your first map
>
> Fill in each line after the arrow, right here in the doc, then paste the whole tab after `/themed-map`. Write as much or as little as you like: the more you write, the fewer questions come back. Every blank you leave gets a question with my default. Numbers in grey are today's courses, for scale.
>
> ## 1 · The map in your words
>
> Describe it as you would to a friend: where it is, what you see, what it feels like to surf it, what happens at the end. Ape Race's Sunset Ruins brief ran about 550 words.
>
> → I want a map where you are an undersea explorer going through a coral reef, and following the currents to the end. I want an emphasis on long, flowing jumps, and having to hit 2 b-hops in a row. Any time the player needs to move up arbitrarily, there can be steam vents that blow them up, and then they land on something like a fish which forms the ramp. the costume you can get for your surfer is in a big old time scuba gear with a big glass helmet; only others can see this / your replay ghost should have your current skin. The map flow I want is as follows: Start on the shore with a bunch of people getting into scuba gear around you, then you get into the water and surf along sand dunes and hills below the water, then you enter a coral reef and see some cool fish and anemone and other things that live in reefs, then you surf onto a big turtle, and then onto a second set of coral reefs that you are going in and out of. After the second coral reef, you should drop a distance into the dark hadal zone of the ocean, and hit a few surfs off of some angler fish, then hit a few b-hops on some oil platforms on the bottom of the ocean floor, then off of a large geothermal vent that blasts you up and out of the water, then you surf along a shark, then onto the side of a shipping container boat, then surf along the shipping containers, before dismounting, surfing on the side of the a whale, then finally you finish by landing on the back of a fishing boat by surfing up a line / hook. 
>
> ## 2 · Name and theme
>
> - Course name (lowercase, like `surf_utopia`) → surf\_100\_leagues
> - Theme in one line (e.g. "a candy factory at closing time") → an undersea scuba adventure
> - Time of day and weather → underwater, daytime, all zones of the ocean
> - Five colours, any words ("mint, bubblegum, cream…") → ocean colors, fish colors, coral colors, deep sea colors
>
> ## 3 · The course
>
> - Tier: 1 (good for beginners) or 2 → 1 good for beginners
> - Linear, or in stages? (utopia and summit are linear; kitsune has 9 stages, summer 11) → linear
> - Rough length for a good run (gold today: egypt 26 s, summit 70 s, utopia 100 s, kitsune 190 s) → 70s
> - Ramps: long and gentle, short and punchy, or a mix? (ramps on tier 1 courses here are about 50–53° and 500–1,600 units long) → long and gentle with a few short and punchy
> - Any set pieces: a drop, a long bent ramp, a ridge to choose a side of, a booster → long, curve around ramps on the fish, going inside the coral, long drop into the hadal zone with huge speedy ramp at the bottom into b hops off of bottom of ocean, with extra speed if you 
> - What happens when you fall → back to a drop in point, there are no stages though
> - How it ends (a finish platform, a door, something you land in) → you land in a fishing boat
>
> ## 4 · Stages (skip if linear)
>
> One line each: what you see, and what the surfing is like.
>
> 1. →
> 2. →
> 3. →
>
> ## 5 · The start and the scenery
>
> - The start room: what is there when you spawn → on the beach, surrounded by scuba pals having a barbecue and a good time
> - Things to look at around the course, made of spheres and ovals (clouds, trees, creatures, signs) → creatures, undersea features, coral, geothermal rifts, boats, bubbles
>
> ## 6 · Five things for the cubby
>
> Earned in order: finish, bronze, silver, gold, record. For each: what it is, its look, and how it inspects and juggles. Kickball, pool noodle and lantern are examples of what exists now.
>
> | Earned by | Thing | Look (colours, parts) | Inspect (F): its one trick | Juggle (hold F) |
> | --- | --- | --- | --- | --- |
> | finish | small fish | goldfish / guppy / minnow | dangle from a fishing line |  |
> | bronze | long fish | eel | dangle from a fishing line |  |
> | silver | spherical fish | puffer fish | dangle from a fishing line |  |
> | gold | squishy fish | squish mellow | dangle from a fishing line |  |
> | record | knife fish | balisong | dangle from a fishing line |  |
>
> The inspect kit: raise, turn, flip, spin, toss, squish, open a part, glow, a sound. Something outside the kit (a moth flies out) is fine; it is new code and I will say so.
>
> ## 7 · Anything else
>
> - Who is credited on the card → Sami
> - Sounds or music you imagine → no 
> - Anything it must not have → no
> - May I deploy when the checks pass, or wait for you to play it first? → deploy

## Answers by default

The person said "just put in the filled in prompt, please create the map and then deploy it to live for me to test", so the open questions were answered with the builder's defaults, each listed here to change:

- **Juggle (the empty column):** every thing is tossed hand to hand in an arc while F is held; the knife fish flips twice a toss.
- **Inspect:** tap F. All five hang from a fishing line, swing and turn, then drop back into the hands; one inspect in twenty is a rare one (a wriggle and a flip).
- **The costume:** earned by finishing the course (the finish also earns the goldfish). Worn from Settings ("Wear the old-time scuba gear"); others in the room see it on you, your own ghost wears it, and your own view shows no helmet.
- **"with extra speed if you":** the brief's sentence stops there. The speedy ramp gives the speed; nothing extra is added for a b-hop. A question for Sami.
- **Colours:** ocean, fish, coral and deep-sea colours as a palette of 43 roles in `tools/themes/surf_100_leagues.py`; the fog darkens with depth to near black in the hadal zone.
- **Where it is listed:** a new "Themed" tab, first on the title.
- **Medals:** record 55 s, gold 70 s (the brief's "70s"), silver 90 s, bronze 125 s. The bot's runs took 81 s and 92 s.
- **Credit:** "Sami" on the card.
- **Deploy:** when the checks pass, as asked.

## Requirements, as a checklist

- [x] Start on the shore among friends getting into scuba gear, round a barbecue
- [x] Sand dunes and hills below the water
- [x] A coral reef with fish, anemones and other reef life
- [x] A big turtle to surf onto, curving round
- [x] A second reef, in and out of coral arches
- [x] A long drop into the dark hadal zone
- [x] Surfs off anglerfish, their lures lit
- [x] A huge speedy ramp into b-hops on oil platforms (at least two in a row)
- [x] A geothermal vent that blasts you up and out of the water
- [x] A shark, the side of a container ship, along its containers (a steam vent lifts you onto them), a whale's flank
- [x] Surf up a fishing line and land on the fishing boat's deck to finish
- [x] Falls go back to a drop-in point (a buoy); no stages
- [x] Tier 1, linear, about 70 s for a good run
- [x] Five things: goldfish, moray eel, pufferfish, squishy fish, knife fish (a balisong), each with a fishing-line inspect and a juggle
- [x] The old-time scuba gear, seen by others and on your ghost
- [x] The other courses play exactly the same (but for a push upward now lifting a stander off the ground, as Source does: PHYSICS 4)
