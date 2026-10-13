# Brief

Stage 1 of the Builder procedure. The request below is the person's, as given on 13 October 2026, and it is the authority on what this game is. `PLAN.md` records the decisions taken from it.

> please use the Builder repo as a base in the directory above, and use the work and repos of Sam gilson at sgilson7.github.io,which can all be found one directory up, and ptonkle.github.io, which is my friends tools he created with a clean usage/install of builder. please build out the MVP and deploy it to my website. you have approval for creating a public github repo, github pages, and approval to deploy to my website when its ready. any time you need to make 2D art and any kind of asset for the game, use the tikz latex prompt you can find in other repos, and try and follow the style guide set in ptonkle's work. now build the mvp for the tool and deploy it, dont stop till its deployed, good luck: I would like a game that is as similar to the alternative, community driven game mode called "Surfing" found in the counter strike games, specifically i remember playing cs surf maps in counter strike global offensive. I want in the MVP for surf_utopia to be perfectly recreated as the only map for now. I want you to spend a lot of time tuning how it feels to surf; first do a literature review / research on sources describing and showing how cs surfing works, stuff like sync rates and the bizarre details of the source engine that make surfing feel so good. for the player character, in their hands instead of guns, they should be able to have different fun things they get to hold by getting certain times on the map, like a rubber chicken if they get the best time, and they start with nothing in their hands at first exceot their hands are clasped in a buddhist mantra / prayer. the surf map should be theoretically accessible by up to 8 people at a time, and should run as smoothly as possible, with some combination of rollback netcode and the netcode found in ape race on ptonkle's github. you should be able to see other people surfing alongside you, but you can use some clever ways to reduce lag if all 8 people is too much, like connected peers only send their race data once at the end of the race, and then next time you race, you render all peers who have sent race times, so then watching them race is all done client side rendering, and not waiting for their position data to be sent every tick. alright, now start and dont stop till the mvp is ready and its deployed to the website. the name of game should be "Surfs Up!" or some very slight, incidental counter strike reference that does not include any mention to terrorism. in general, the game should not have a military styling, it should instead have the vibe of kids at their local gymnasium and in this universe, surfing is just a well known thing to do, like kids going to gyms with trapeezes over soft blocks of fabric so when you jump off the trapeeze, the kids dont hurt themselves, that kind of vibe. ok good luck start working

## What done means

- surf_utopia is playable start to finish in a browser, from its real geometry.
- Movement follows Source's: air acceleration, ramp clipping, the 0.7 ground rule, gravity, the tick.
- A run is timed from leaving the start to an ending; sync, strafes and speed are shown.
- Hands start clasped; better times unlock things to hold, and the best time holds the rubber chicken.
- Up to eight browsers meet online. Live surfers are drawn from their inputs with rollback, and each finished run is sent once and replayed as a ghost.
- The look is a kids' gymnasium, not a military one. 2D art is TikZ.
- Deployed to GitHub Pages and listed on sgilson7.github.io.

## Decisions that stay with the person

- Whether the geometry taken from the original map may be published (see `LICENSES.md`).
- Medal times, once the course has been played by people.
