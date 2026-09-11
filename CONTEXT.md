# Combat Ductus

An internal-only 2D versus fighting game, built as a demo/showcase of AI-assisted development. Playable characters are stylized depictions of the company's own employees (managers and up), not licensed or fictional fighters.

## Language

**Versus Fighter**:
The genre: 1v1 duels between two characters on a fixed arena, structured in rounds, won by depleting the opponent's health before time runs out. Distinct from a *beat 'em up* (a side-scrolling brawler against waves of enemies), which this project explicitly is not.
_Avoid_: Beat 'em up, brawler

**Character**:
A playable fighter composited from one employee's face photograph, a hand-picked Body Type, and a moveset. Sourced only from managers and above.
_Avoid_: Fighter (ambiguous with the genre), Avatar

**Body Type**:
One of three purely cosmetic sprite proportions (Small, Medium, Large) chosen per Character to match that employee's real-world build for recognizability. Carries no gameplay effect — stats, speed, and damage are identical across Body Types.
_Avoid_: Weight class, archetype

**Roster**:
The full set of playable Characters shipped in a given version. Planned at 5-6 for v1, drawn from managers and above.

**Health**:
A Character's remaining damage capacity, starting a Match at 5 points. A Match ends immediately the instant either Character's Health reaches 0 — there are no rounds.
_Avoid_: Hit points, energy

**Punch**:
A Character's light attack, dealing 1 point of damage to the opponent's Health.
_Avoid_: Hit

**Kick**:
A Character's heavy attack, dealing 2 points of damage to the opponent's Health.

**Jump**:
Vertical repositioning available to either Character. Purely movement — a Character cannot Punch or Kick while airborne in v1.

**Facing**:
Each Character has a facing direction fixed for the whole Match — Player 1 always faces right, Player 2 always faces left — regardless of either Character's position. Characters never turn. There is no block or dedicated dodge mechanic; avoiding an attack is purely a matter of moving out of its range.

**Match**:
A single, unrepeated contest between two Characters, decided the moment one Character's Health reaches 0. Not divided into rounds. In v1, a Match's end is terminal — a winner is declared and there is no rematch/restart flow.
_Avoid_: Round, game (when "Match" is meant)

**Stage**:
The single fixed environment a Match is fought in — a cleared business office landscape with workstations pushed to the side. Purely a backdrop; v1 has exactly one Stage, with no selection.
_Avoid_: Arena, level, map

**Idle Opponent**:
The state of the Player 2 Character before a second network Client has connected: present on the Stage, but stationary and unresponsive to input. Resolves into a normal controlled Character the moment a second Client joins.
_Avoid_: CPU, bot, AI opponent (none of these apply — it is not being controlled by any logic, just absent)
