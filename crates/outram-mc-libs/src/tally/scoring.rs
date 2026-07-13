// Tally scoring — accumulate scores at collision/track events.
//
// C++ source: `src/tallies/tally_scoring.cpp`.
//
// After each transport event (collision, surface crossing, track end), this
// module evaluates which tallies/bins are hit and accumulates the score.
//
// TODO: implement score_event once Particle, Tally, and geometry are ported.
