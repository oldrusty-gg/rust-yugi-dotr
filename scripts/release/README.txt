ROSES - a fan recreation of Yu-Gi-Oh! The Duelists of the Roses (PAL)
Release {RELEASE}  (beta)
======================================================================

This is an unfinished, unofficial fan project. It contains NO game data:
everything the game shows and plays comes from YOUR OWN copy of the PAL
(European) PlayStation 2 disc, which the launcher imports once.


WHAT YOU NEED
-------------
- Windows 10 or Windows 11, 64-bit.
- A graphics card/driver with DirectX 12 or Vulkan support (any GPU from
  roughly 2014 onwards with current drivers; integrated Intel/AMD is fine).
- About 2.5 GB of free disk space for the imported game.
- An ISO image of your own PAL disc:
  "Yu-Gi-Oh! The Duelists of the Roses" (Europe), SLES-51579, as a plain
  .iso file (an exact 1:1 dump; extract it first if it is in a .zip/.7z).
  US/Japanese versions and modified ISOs are not accepted - the importer
  checks the ISO's fingerprint and tells you if it doesn't match.
- Nothing else to install: no emulator, BIOS, ffmpeg or runtime packages.


FIRST START
-----------
1. Unzip this folder somewhere you can write to, e.g. Documents or the
   Desktop (NOT "Program Files"). The game stores its data next to the
   launcher.
2. Run roses-launcher.exe.
   Windows SmartScreen may warn about an unrecognised app because these
   files are not signed: click "More info" -> "Run anyway".
3. Click "Import ISO" and pick your .iso. The button shows the progress;
   this takes a few minutes and only reads the ISO (it is never changed).
4. Click "Play". The game starts with the intro movie and the title
   screen. Next time, just run the launcher and click Play.
   The gear button (Settings) has Re-import, the data, saves and logs
   folders, and the controller layout.


CONTROLS
--------
Gamepad: Xbox (XInput) controllers and Sony DualSense / DualShock 4
(USB or Bluetooth) work directly. Buttons follow the PS2 layout by
position: A/Cross = Cross, B/Circle = Circle, X/Square = Square,
Y/Triangle = Triangle, bumpers L1/R1, triggers L2/R2, Back/Share = Select.
The game window must have focus. If you use Steam Input or DS4Windows
and buttons register twice, turn those off for this game.

Keyboard (player 1):
  Arrows = D-pad      X = Cross              Z = Circle
  A = Square          D = Triangle           S = Start
  Tab = Select        Q / E = L1 / R1        1 / W = L2 / R2
  Start skips the intro movie.


SAVING
------
Like on the PS2, nothing is saved automatically. On the campaign map
open SYSTEM -> SAVE to save to the virtual memory card; choose
"Continue" on the title screen to load it. Save often.

The memory card is stored in:
  roses-data\memory-card\   (next to roses-launcher.exe)
To move to a newer version of this package, copy your roses-data
folder (or at least roses-data\memory-card) into the new folder.


KNOWN LIMITATIONS
-----------------
(Also in KNOWN-ISSUES.txt, which the launcher's "Known issues" link opens.)
- Attack effects (the per-monster battle animations) are not drawn yet;
  battles resolve correctly but show simpler stand-ins.
- A few rare card effects or rule situations are not supported yet.
  When one comes up, the game stops: the window title then reads
  "Roses - stopped: ..." with the reason. Close the game, start it
  again from the launcher and Continue from your last save. Please
  report it (see below).
- Some screens are still incomplete: the in-duel card details view,
  parts of the deck editor's catalogue, NOW LOADING screens, and some
  fades/timings differ slightly from the original.
- The intro/ending movies' sound sync has not been checked by ear.
- Two-player Custom Duel and Trade need a second controller (or the
  second keyboard layout) and have had little testing.
- Online Play is a beta (see ONLINE PLAY).


ONLINE PLAY (beta)
------------------
"Online Play" (the last entry on the title screen) duels other players
over the internet through an online server. When this download includes
an online.json beside the launcher, the server's address and certificate
come with it and there is nothing to set up; otherwise put the online.json
of the server you want to use in roses-data\ (next to the launcher).
Make your account once on the launcher's Profile page (the person icon
on the left): Sign up with a name of 3-12 letters or digits and a
password of at least 8 characters (typed twice). The game is then
signed in already: Title -> Online Play goes straight to the lobby.
Log in, Log out and Change password are on the same page. (You can also
sign up inside the game: Online Play -> CONNECT -> SIGN UP.)
In the lobby choose a player to challenge, or CREATE ROOM and give your
friend the 5-character room code (they choose JOIN ROOM and type it).
Your deck is the current deck of your MEMORY CARD save (slot 1); online
duels change no save. If the game closes during a duel, start it again
within a minute and choose Online Play: you rejoin the duel.
Both players need the same version of this build and the same PAL disc.


REPORTING A PROBLEM
-------------------
Click "Report a problem" in the launcher. It opens the logs folder:
  roses-data\logs\
Send the files in it (launcher.log, game.log and the "-previous" copies)
together with a short description of what you were doing. The game log
of the last session is kept until you press Play twice more, so send it
soon after the problem. The logs contain no game data, only messages
(and folder paths on your PC).


FILES
-----
roses-launcher.exe    the launcher: import your ISO, then Play
KNOWN-ISSUES.txt      what is known not to work yet
roses.exe             the importer the launcher runs (command line)
roses-bevy-scene.exe  the game
assets\               the game's own shader programs (no game data)
online.json           the Online Play server's address and certificate pin
