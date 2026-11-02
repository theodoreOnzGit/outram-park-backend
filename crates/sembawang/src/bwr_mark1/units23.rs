//! Fukushima Daiichi Units 2 and 3: which unit a plant is, the operators'
//! recorded sequences for Units 2 and 3, and the recorded plant readings
//! they are compared against (gh:#1109).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! **The source.** Every time below is from one open document held in the
//! repository: USNRC HRTD, *GE BWR/4 Advanced Technology*, ch. 7.1
//! "Fukushima Dai-ichi Event" (Rev 0114), **Table 7.1-2 "Event
//! Chronology"**, pp. 7.1-18 to 7.1-23
//! (`crates/kovan-literature/reactor-literature/theodore-open-corpus/nrc/nrc-hrtd-ge-bwr4-atm-ch7.1-fukushima.pdf`,
//! PDF pages 22 to 27; read locally with `pdftotext`). Its Figures
//! 7.1-9a to 7.1-9j draw the same chronology and add nothing for Units 2
//! and 3. The table's "TIME ZERO PLUS" column (h:mm after the 14:46 JST
//! scram) is the time used; each row's Japan time is kept in its label so
//! that the two can be checked against each other. Each event carries the
//! page it is on ([`SequenceEvent::source`]).
//!
//! **A disagreement inside the source.** Three rows on pp. 7.1-19 and
//! 7.1-20 give an offset 2 min short of their Japan time: 20:48 against
//! 11:36 (14:46 + 20:48 is 11:34), 21:22 against 12:10, 21:47 against
//! 12:35. Every other Unit 2 and 3 row agrees to the minute. The offset
//! column is used, as for all rows; the 2 min is far below anything the
//! comparison resolves, and is recorded, not resolved.
//!
//! **Inputs and checks are kept apart** (root `CLAUDE.md`, "PROCESS right
//! first"). The operators' actions (a start, a loss, an SRV opened, a
//! spray, a vent order, an injection) are inputs. What the plant did on its
//! own (RCIC's high-level trips, the explosions) is recorded with
//! [`EventKind::Recorded`] and is never acted on: the model must produce
//! it. The pressures, levels and temperatures the table reports are
//! [`OBSERVATIONS`], the check, never an input.
//!
//! **What had to be assumed (\[A\], said at each row).** Unit 3's RCIC from
//! 01:17 run in the operators' throttled mode (the table records no high-
//! level trip in the next 19.5 h, so level control is inferred); the fire
//! pumper's spray stopping when it began injecting (42:39); Unit 3's vent
//! valves closing at 59:14 (the table says they "had failed closed" by
//! then) and venting resumed at 64:14 (the row says "just prior to
//! commencing venting"); the run lengths. **Not found** in the table:
//! when Unit 2's RCIC suction moved from the storage tank to the torus (the
//! row at 26:44 says it was then taking suction from the pool; the model
//! has no torus suction, `turbine_pump`'s docs); the count of SRVs the
//! operators opened beyond the one each row names; when Unit 2's SRV
//! closed again (the row at 80:04 implies it did, about 79:54, but states
//! no closure, so none is applied).
//!
//! **Not in the model (stated):** Units 1 and 2's torus cooling and spray
//! (00:39 to the tsunami at 00:55, on AC: 16 min, before the decay heat had
//! heated the pool); Unit 3's MOX assemblies and the Unit 3 to Unit 4
//! hydrogen path (gh:#1122, #1123); Unit 2's blow-out panel
//! (opened by Unit 1's explosion at 24:50, row on p. 7.1-20).

use super::params::Params;
use super::containment_network::NetworkConfig;
use super::EventKind;

/// Which Fukushima Daiichi unit a plant is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Unit {
    /// Unit 1 (BWR/3, isolation condensers): the model before gh:#1109.
    #[default]
    One,
    /// Unit 2 (BWR/4, RCIC and HPCI).
    Two,
    /// Unit 3 (BWR/4, RCIC and HPCI; DC power survived the tsunami).
    Three,
}

impl Unit {
    /// The unit's inputs ([`Params::default`], [`Params::unit2`],
    /// [`Params::unit3`]).
    pub fn params(self) -> Params {
        match self {
            Unit::One => Params::default(),
            Unit::Two => Params::unit2(),
            Unit::Three => Params::unit3(),
        }
    }

    /// The unit's containment network.
    pub fn network_config(self) -> NetworkConfig {
        match self {
            Unit::One => NetworkConfig::unit1(),
            Unit::Two => NetworkConfig::unit2(),
            Unit::Three => NetworkConfig::unit3(),
        }
    }

    /// Unit 1 alone has isolation condensers (HRTD Table 7.1-1, p. 7.1-3:
    /// "IC" for Unit 1, "RCIC" for Units 2 to 6).
    pub fn has_isolation_condenser(self) -> bool {
        self == Unit::One
    }

    /// Its recorded sequence ([`super::EVENTS`] is Unit 1's, in its own
    /// form; this returns the Units 2 and 3 tables, empty for Unit 1).
    pub fn sequence(self) -> &'static [SequenceEvent] {
        match self {
            Unit::One => &[],
            Unit::Two => &UNIT2_SEQUENCE,
            Unit::Three => &UNIT3_SEQUENCE,
        }
    }

    /// How long its historical run goes, s: Unit 1's 30 h; for Units 2
    /// and 3 \[A\] the run lengths, past the last containment reading this
    /// file compares against before each unit's later instrument failures
    /// (Unit 2: 81:16; Unit 3: 69:14).
    pub fn scenario_end_s(self) -> f64 {
        match self {
            Unit::One => super::SCENARIO_END_S,
            Unit::Two => 82.0 * 3600.0,
            Unit::Three => 70.0 * 3600.0,
        }
    }

    /// A number for the worker wire.
    pub fn code(self) -> f64 {
        match self {
            Unit::One => 1.0,
            Unit::Two => 2.0,
            Unit::Three => 3.0,
        }
    }

    /// The unit for a wire code; Unit 1 for anything else.
    pub fn from_code(c: f64) -> Self {
        if c == 2.0 {
            Unit::Two
        } else if c == 3.0 {
            Unit::Three
        } else {
            Unit::One
        }
    }
}

/// One row of a unit's sequence: when, what the table says, what the
/// model does, and where in the source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SequenceEvent {
    /// Seconds after the scram (the table's "TIME ZERO PLUS").
    pub t_s: f64,
    pub label: &'static str,
    pub kind: EventKind,
    /// The table and page, and any \[A\] on the row.
    pub source: &'static str,
}

/// `h:mm` after the scram, in seconds.
pub const fn hm(h: u32, m: u32) -> f64 {
    ((h * 60 + m) * 60) as f64
}

const P18: &str = "HRTD Table 7.1-2, p. 7.1-18";
const P19: &str = "HRTD Table 7.1-2, p. 7.1-19";
const P20: &str = "HRTD Table 7.1-2, p. 7.1-20";
const P21: &str = "HRTD Table 7.1-2, p. 7.1-21";
const P22: &str = "HRTD Table 7.1-2, p. 7.1-22";

/// Unit 2's sequence (HRTD Table 7.1-2).
pub const UNIT2_SEQUENCE: [SequenceEvent; 10] = [
    SequenceEvent { t_s: hm(0, 6), label: "14:52 Unit 2 RCIC in service", kind: EventKind::RcicStart, source: P18 },
    SequenceEvent { t_s: hm(0, 19), label: "15:05 Units 2 and 3 RCIC in service, cycled for level control (recorded)", kind: EventKind::Recorded, source: P18 },
    SequenceEvent { t_s: hm(0, 42), label: "15:28 Unit 2 RCIC trip on high RPV water level (recorded: the model trips at L8 itself)", kind: EventKind::Recorded, source: P18 },
    SequenceEvent { t_s: hm(0, 53), label: "15:39 Unit 2 RCIC manually restarted", kind: EventKind::RcicStart, source: P18 },
    SequenceEvent { t_s: hm(0, 55), label: "15:41 Tsunami: loss of all AC and DC power (Unit 2)", kind: EventKind::DcLost, source: P18 },
    SequenceEvent { t_s: hm(44, 14), label: "11:00 (13 Mar) Unit 2 aligned for containment venting (the disk must still burst)", kind: EventKind::VentOpened, source: P21 },
    SequenceEvent { t_s: hm(70, 39), label: "13:25 (14 Mar) Unit 2 RCIC lost due to pump failure", kind: EventKind::RcicLost, source: P22 },
    SequenceEvent { t_s: hm(75, 14), label: "18:00 Car batteries used to open 1 SRV", kind: EventKind::ReliefHeld(1), source: P22 },
    SequenceEvent { t_s: hm(77, 8), label: "19:54 Unit 2 start of seawater injection via fire pumper", kind: EventKind::InjectionOn, source: P22 },
    SequenceEvent { t_s: hm(82, 0), label: "End of the Unit 2 run", kind: EventKind::Recorded, source: "[A] run length" },
];

/// Unit 3's sequence (HRTD Table 7.1-2).
pub const UNIT3_SEQUENCE: [SequenceEvent; 18] = [
    SequenceEvent { t_s: hm(0, 19), label: "15:05 Unit 3 RCIC in service", kind: EventKind::RcicStart, source: P18 },
    SequenceEvent { t_s: hm(0, 39), label: "15:25 Unit 3 RCIC trips on high RPV level (recorded: the model trips at L8 itself)", kind: EventKind::Recorded, source: P18 },
    SequenceEvent { t_s: hm(0, 55), label: "15:41 Tsunami: station blackout, Unit 3 keeps DC", kind: EventKind::AcLost, source: P18 },
    SequenceEvent {
        t_s: hm(1, 17),
        label: "16:03 Unit 3 RCIC started up for injection (level 157 in > TAF)",
        kind: EventKind::RcicStartThrottled,
        source: "HRTD Table 7.1-2, p. 7.1-19; throttled mode [A] (no high-level trip recorded in the next 19.5 h)",
    },
    SequenceEvent { t_s: hm(20, 48), label: "11:36 (12 Mar) Unit 3 RCIC lost and could not be restarted", kind: EventKind::RcicLost, source: P19 },
    SequenceEvent {
        t_s: hm(21, 47),
        label: "12:35 Unit 3 HPCI auto start at 120 in > TAF; operators controlled injection",
        kind: EventKind::HpciStartThrottled,
        source: P20,
    },
    SequenceEvent { t_s: hm(35, 56), label: "02:42 (13 Mar) Unit 3 HPCI tripped, battery depletion", kind: EventKind::HpciLost, source: P20 },
    SequenceEvent { t_s: hm(35, 56), label: "02:42 (13 Mar) Unit 3 batteries depleted: SRVs cannot be operated", kind: EventKind::DcLost, source: P20 },
    SequenceEvent { t_s: hm(40, 53), label: "07:39 Unit 3 containment sprays using a fire pumper", kind: EventKind::SprayOn, source: P20 },
    SequenceEvent { t_s: hm(41, 55), label: "08:41 Unit 3 containment venting commenced (the disk must still burst)", kind: EventKind::VentOpened, source: P20 },
    SequenceEvent { t_s: hm(42, 22), label: "09:08 Unit 3 SRV opened on automobile batteries", kind: EventKind::ReliefHeld(1), source: P21 },
    SequenceEvent { t_s: hm(42, 39), label: "09:25 Unit 3 fire pumper moves from spray to injection", kind: EventKind::SprayOff, source: "HRTD Table 7.1-2, p. 7.1-21; [A] the spray's end, taken at the injection's start" },
    SequenceEvent { t_s: hm(42, 39), label: "09:25 Unit 3 start of freshwater injection", kind: EventKind::InjectionOn, source: P21 },
    SequenceEvent { t_s: hm(46, 26), label: "13:12 Unit 3 start of seawater injection", kind: EventKind::InjectionOn, source: P21 },
    SequenceEvent {
        t_s: hm(59, 14),
        label: "02:00 (14 Mar) Unit 3 containment vent valve(s) had failed closed",
        kind: EventKind::VentClosed,
        source: "HRTD Table 7.1-2, p. 7.1-21; [A] closed by then, taken at this time",
    },
    SequenceEvent {
        t_s: hm(64, 14),
        label: "07:00 Unit 3 venting recommenced",
        kind: EventKind::VentOpened,
        source: "HRTD Table 7.1-2, p. 7.1-21 (\"just prior to commencing venting\"); [A] the time",
    },
    SequenceEvent { t_s: hm(68, 15), label: "11:01 Unit 3 reactor building explodes (recorded)", kind: EventKind::Recorded, source: P21 },
    SequenceEvent { t_s: hm(70, 0), label: "End of the Unit 3 run", kind: EventKind::Recorded, source: "[A] run length" },
];

/// What a reading is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observed {
    /// "Containment pressure" (the table's usual phrase; the drywell's,
    /// as the comparison reads it), psig.
    ContainmentPsig,
    /// The drywell's, named so, psig.
    DrywellPsig,
    /// The torus's, psig.
    TorusPsig,
    /// The vessel's, psig.
    RpvPsig,
    /// The vessel's level, inches above the top of active fuel (negative
    /// below).
    LevelInAboveTaf,
}

/// One reading from HRTD Table 7.1-2: the check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    pub unit: Unit,
    pub t_s: f64,
    pub what: Observed,
    /// As the table gives it (psig, or inches).
    pub value: f64,
    /// The page, and the table's own caveat when it gives one.
    pub note: &'static str,
}

impl Observation {
    /// The value in SI: Pa absolute for a pressure, m for a level.
    pub fn si(&self) -> f64 {
        match self.what {
            Observed::LevelInAboveTaf => self.value * 0.0254,
            _ => self.value * PSI_PA + super::params::P_ATM,
        }
    }
}

/// One psi, Pa.
pub const PSI_PA: f64 = 6894.757;

const fn ob(unit: Unit, t_s: f64, what: Observed, value: f64, note: &'static str) -> Observation {
    Observation { unit, t_s, what, value, note }
}

/// Every Unit 2 and 3 pressure and level the table reports within the two
/// runs, in table order (the Unit 2 rows at 84:14 and later and Unit 3's at
/// 69:14 and later fall outside them, or the table calls them questionable).
pub const OBSERVATIONS: [Observation; 33] = [
    ob(Unit::Three, hm(1, 17), Observed::LevelInAboveTaf, 157.0, P19),
    ob(Unit::Two, hm(7, 4), Observed::LevelInAboveTaf, 134.0, P19),
    ob(Unit::Two, hm(8, 39), Observed::RpvPsig, 914.0, P19),
    ob(Unit::Two, hm(8, 39), Observed::ContainmentPsig, 5.0, P19),
    ob(Unit::Three, hm(21, 22), Observed::ContainmentPsig, 42.0, P19),
    ob(Unit::Three, hm(21, 47), Observed::LevelInAboveTaf, 120.0, P20),
    ob(Unit::Three, hm(26, 14), Observed::RpvPsig, 421.0, P20),
    ob(Unit::Two, hm(26, 44), Observed::ContainmentPsig, 30.0, P20),
    ob(Unit::Three, hm(35, 56), Observed::RpvPsig, 580.0, P20),
    ob(Unit::Three, hm(37, 29), Observed::LevelInAboveTaf, 0.0, P20),
    ob(Unit::Three, hm(38, 14), Observed::RpvPsig, 1070.0, P20),
    ob(Unit::Three, hm(38, 14), Observed::LevelInAboveTaf, -79.0, P20),
    ob(Unit::Three, hm(38, 14), Observed::ContainmentPsig, 38.0, P20),
    ob(Unit::Three, hm(40, 59), Observed::LevelInAboveTaf, -118.0, P20),
    ob(Unit::Three, hm(40, 59), Observed::RpvPsig, 1060.0, P20),
    ob(Unit::Three, hm(40, 59), Observed::ContainmentPsig, 52.0, P20),
    ob(Unit::Three, hm(40, 59), Observed::TorusPsig, 49.0, P20),
    ob(Unit::Three, hm(41, 55), Observed::ContainmentPsig, 78.0, "HRTD Table 7.1-2, p. 7.1-20: the maximum"),
    ob(Unit::Two, hm(42, 9), Observed::ContainmentPsig, 39.0, P20),
    ob(Unit::Three, hm(42, 24), Observed::ContainmentPsig, 77.0, P21),
    ob(Unit::Three, hm(42, 38), Observed::ContainmentPsig, 63.0, P21),
    ob(Unit::Three, hm(42, 39), Observed::RpvPsig, 51.0, P21),
    ob(Unit::Three, hm(46, 14), Observed::RpvPsig, 28.0, P21),
    ob(Unit::Three, hm(59, 14), Observed::ContainmentPsig, 23.0, P21),
    ob(Unit::Three, hm(60, 54), Observed::ContainmentPsig, 33.0, P21),
    ob(Unit::Three, hm(62, 34), Observed::ContainmentPsig, 51.0, P21),
    ob(Unit::Three, hm(68, 7), Observed::ContainmentPsig, 60.0, P21),
    ob(Unit::Two, hm(68, 15), Observed::ContainmentPsig, 52.0, P21),
    ob(Unit::Two, hm(69, 44), Observed::ContainmentPsig, 56.0, "HRTD Table 7.1-2, p. 7.1-22: with 301 degF water (saturated)"),
    ob(Unit::Two, hm(70, 39), Observed::DrywellPsig, 52.0, P22),
    ob(Unit::Two, hm(73, 48), Observed::RpvPsig, 1015.0, P22),
    ob(Unit::Two, hm(80, 4), Observed::ContainmentPsig, 63.0, P22),
    ob(Unit::Two, hm(81, 16), Observed::ContainmentPsig, 95.0, "HRTD Table 7.1-2, p. 7.1-22: \"stable\" (p. 7.1-22 at 80:58 calls the torus reading questionable)"),
];

/// The readings for `unit`, in time order.
pub fn observations(unit: Unit) -> impl Iterator<Item = &'static Observation> {
    OBSERVATIONS.iter().filter(move |o| o.unit == unit)
}

/// The events of `unit`'s sequence whose time falls in the step starting at
/// `t_s` of length `dt_s` (`t_s <= t < t_s + dt_s`, Unit 1's rule).
pub fn due(unit: Unit, t_s: f64, dt_s: f64) -> impl Iterator<Item = &'static SequenceEvent> {
    unit.sequence().iter().filter(move |e| t_s >= e.t_s && t_s < e.t_s + dt_s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tables are in time order, every input row cites a page of Table
    /// 7.1-2 (an \[A\] row says so), and the times reproduce the table's
    /// Japan times (14:46 plus the offset, mod 24 h) written in each label.
    #[test]
    fn the_sequences_are_ordered_cited_and_consistent_with_their_clock_times() {
        for unit in [Unit::Two, Unit::Three] {
            let seq = unit.sequence();
            assert!(seq.windows(2).all(|w| w[0].t_s <= w[1].t_s), "{unit:?}");
            for e in seq {
                assert!(e.source.contains("HRTD Table 7.1-2") || e.source.contains("[A]"), "{}", e.label);
                if let Some(clock) = e.label.get(0..5).filter(|c| c.as_bytes()[2] == b':') {
                    let (h, m): (u32, u32) = (clock[0..2].parse().unwrap(), clock[3..5].parse().unwrap());
                    let jst = (14 * 60 + 46 + (e.t_s as u32) / 60) % (24 * 60);
                    // The source's own 2 min disagreement (module docs).
                    let slip = if e.t_s == hm(20, 48) || e.t_s == hm(21, 47) { 2 } else { 0 };
                    assert_eq!(jst + slip, h * 60 + m, "{}", e.label);
                }
            }
            assert!((seq.last().unwrap().t_s - unit.scenario_end_s()).abs() < 1.0);
        }
        assert!(Unit::One.sequence().is_empty() && Unit::One.has_isolation_condenser() && !Unit::Two.has_isolation_condenser());
        for u in [Unit::One, Unit::Two, Unit::Three] {
            assert_eq!(Unit::from_code(u.code()), u);
        }
        assert_eq!(Unit::Three.params(), Params::unit3());
        assert_eq!(Unit::Two.network_config(), NetworkConfig::unit2());
        let o: Vec<_> = observations(Unit::Two).collect();
        assert!(o.windows(2).all(|w| w[0].t_s <= w[1].t_s));
        let first = o.iter().find(|x| x.what == Observed::ContainmentPsig).unwrap();
        assert_eq!(first.t_s, hm(8, 39));
        assert!((first.si() - (5.0 * PSI_PA + super::super::params::P_ATM)).abs() < 1e-9);
        assert!((o[0].si() - 134.0 * 0.0254).abs() < 1e-12);
    }

    /// `due` returns an event exactly in the step that contains its time.
    #[test]
    fn an_event_is_due_in_one_step_only() {
        let t = hm(70, 39);
        assert_eq!(due(Unit::Two, t - 1.0, 1.0).count(), 0);
        assert_eq!(due(Unit::Two, t, 1.0).map(|e| e.kind).collect::<Vec<_>>(), vec![EventKind::RcicLost]);
        assert_eq!(due(Unit::Two, t + 1.0, 1.0).count(), 0);
        // Two rows at 35:56 for Unit 3.
        assert_eq!(due(Unit::Three, hm(35, 56), 1.0).count(), 2);
    }
}
