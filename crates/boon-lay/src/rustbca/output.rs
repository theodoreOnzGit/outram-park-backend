// SPDX-License-Identifier: GPL-3.0
//
// RustBCA copy — provenance
// -------------------------
// Upstream project : RustBCA — https://github.com/lcpp-org/RustBCA
// Upstream version : v3.0.0, commit a356280d564bbdfbe74b97f32db0776444da58a5
// Upstream source  : src/output.rs
// Authors          : Jon Drobny <drobny2@illinois.edu>, Jon Drobny
//                    <jdrobny@tae.com> (upstream Cargo.toml); no copyright line upstream
// Original license : GNU GPL v3 (upstream LICENSE copied beside this file)
// Local changes    : libm routing (Real::); `distributions` feature code removed; file
//                    output types and functions gated on rustbca-std
//                    (full list: module docs in mod.rs)

use super::*;
#[cfg(feature = "rustbca-std")]
use std::fs::File;

#[derive(Clone, Debug)]
pub struct OutputUnits {
    pub length_unit: f64,
    pub energy_unit: f64,
    pub mass_unit: f64
}

/// Converts from 6D particle coordinates to energy, angle coordinates w.r.t. negative x-axis
pub fn energy_angle_from_particle(particle: &particle::Particle, units: &OutputUnits) -> (f64, f64) {
    let energy = particle.E/units.energy_unit;
    let ux = particle.dir.x;
    let uy = particle.dir.y;
    let uz = particle.dir.z;

    let vyz = (Real::powi(uy, 2) + Real::powi(uz, 2)).sqrt();
    let angle = Real::atan2(vyz, -ux) * 180.0 / PI;

    (energy, angle)
}

#[cfg(feature = "rustbca-std")]
/// File streams for list output
pub struct OutputListStreams {
    reflected_file_stream: BufWriter<File>,
    sputtered_file_stream: BufWriter<File>,
    deposited_file_stream: BufWriter<File>,
    trajectory_file_stream: BufWriter<File>,
    trajectory_data_stream: BufWriter<File>,
    displacements_file_stream: BufWriter<File>,
    energy_loss_file_stream: BufWriter<File>,
}

/// Simulation-wide summary output tracker.
pub struct Summary {
    pub num_incident: u64,
    pub num_sputtered: u64,
    pub num_reflected: u64,
}

/// Summary tracker of sputtering and reflection
impl Summary {
    pub fn new(num_incident: u64) -> Summary {
        Summary {
            num_incident,
            num_sputtered: 0,
            num_reflected: 0,
        }
    }

    pub fn add(&mut self, particle: &particle::Particle) {
        if particle.incident & particle.left {
            self.num_reflected += 1;
        }
        if !particle.incident & particle.left {
            self.num_sputtered += 1;
        }
    }
}

#[cfg(feature = "rustbca-std")]
pub struct SummaryPerSpecies {
    pub m: Vec<f64>,
    pub sputtered: Vec<usize>,
    pub reflected: Vec<usize>,
    pub deposited: Vec<usize>,
    pub summary_stream_file: BufWriter<File>,
}

#[cfg(feature = "rustbca-std")]
impl SummaryPerSpecies {
    pub fn new(options: &Options) -> SummaryPerSpecies {

        let summary_output_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(format!("{}{}", options.name, "summary.output"))
            .context("Could not open output file.")
            .unwrap();
        let writer = BufWriter::with_capacity(8000, summary_output_file);

        SummaryPerSpecies {
            m: vec![],
            sputtered: vec![],
            reflected: vec![],
            deposited: vec![],
            summary_stream_file: writer,
        }
    }

    pub fn print(&mut self, options: &Options, output_units: &OutputUnits) {
        //Write to summary file
        writeln!(self.summary_stream_file, "mass, reflected, sputtered, deposited")
            .unwrap_or_else(|_| panic!("Output error: could not write to {}summary.output.", options.name));

        for (mass, reflected, sputtered, deposited) in izip!(&self.m, &self.reflected, &self.sputtered, &self.deposited) {
            writeln!(self.summary_stream_file, "{}, {}, {}, {},", mass/output_units.mass_unit, reflected, sputtered, deposited)
                .unwrap_or_else(|_| panic!("Output error: could not write to {}summary.output.", options.name));
        }
        self.summary_stream_file.flush().unwrap();
    }

    pub fn update(&mut self, particle: &particle::Particle) {

        if self.m.contains(&(particle.m)) {

            let index = self.m.iter().position(|m| *m == particle.m).unwrap();

            match (particle.incident, particle.left) {
                (true, true) => self.reflected[index] += 1,
                (true, false) => self.deposited[index] += 1,
                (false, true) => self.sputtered[index] += 1,
                _ => (),
            }
        } else {
            self.m.push(particle.m);
            match (particle.incident, particle.left) {
                (true, true) => {self.reflected.push(1); self.deposited.push(0); self.sputtered.push(0);},
                (true, false) => {self.reflected.push(0); self.deposited.push(1); self.sputtered.push(0);},
                (false, true) => {self.reflected.push(0); self.deposited.push(0); self.sputtered.push(1);},
                _ => {self.reflected.push(0); self.deposited.push(0); self.sputtered.push(0);},
            }
        }
    }
}

#[cfg(feature = "rustbca-std")]
/// Open list output files for streaming write
pub fn open_output_lists(options: &Options) -> OutputListStreams {
    //Open output files for streaming output
    let reflected_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(format!("{}{}", options.name, "reflected.output"))
        .context("Could not open output file.")
        .unwrap();
    let reflected_file_stream = BufWriter::with_capacity(options.write_buffer_size, reflected_file);

    let sputtered_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(format!("{}{}", options.name, "sputtered.output"))
        .context("Could not open output file.")
        .unwrap();
    let sputtered_file_stream = BufWriter::with_capacity(options.write_buffer_size, sputtered_file);

    let deposited_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(format!("{}{}", options.name, "deposited.output"))
        .context("Could not open output file.")
        .unwrap();
    let deposited_file_stream = BufWriter::with_capacity(options.write_buffer_size, deposited_file);

    let trajectory_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(format!("{}{}", options.name, "trajectories.output"))
        .context("Could not open output file.")
        .unwrap();
    let trajectory_file_stream = BufWriter::with_capacity(options.write_buffer_size, trajectory_file);

    let trajectory_data = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(format!("{}{}", options.name, "trajectory_data.output"))
        .context("Could not open output file.")
        .unwrap();
    let trajectory_data_stream = BufWriter::with_capacity(options.write_buffer_size, trajectory_data);

    let displacements_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(format!("{}{}", options.name, "displacements.output"))
        .context("Could not open output file.")
        .unwrap();
    let displacements_file_stream = BufWriter::with_capacity(options.write_buffer_size, displacements_file);

    let energy_loss_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(format!("{}{}", options.name, "energy_loss.output"))
        .context("Could not open output file.")
        .unwrap();
    let energy_loss_file_stream = BufWriter::with_capacity(options.write_buffer_size, energy_loss_file);

    OutputListStreams {
        reflected_file_stream,
        sputtered_file_stream,
        deposited_file_stream,
        trajectory_file_stream,
        trajectory_data_stream,
        displacements_file_stream,
        energy_loss_file_stream,
    }
}

#[cfg(feature = "rustbca-std")]
/// Write output lists
pub fn output_lists(output_list_streams: &mut OutputListStreams, particle: particle::Particle, options: &Options, output_units: &OutputUnits) {

    let length_unit = output_units.length_unit;
    let energy_unit = output_units.energy_unit;
    let mass_unit = output_units.mass_unit;

    if !particle.incident & options.track_displacements & (particle.energy_origin > particle.Ed) {
        writeln!(
            output_list_streams.displacements_file_stream, "{},{},{},{},{},{},{},{},{}",
            particle.m/mass_unit, particle.Z, particle.energy_origin/energy_unit,
            particle.pos_origin.x/length_unit, particle.pos_origin.y/length_unit, particle.pos_origin.z/length_unit,
            particle.pos.x/length_unit, particle.pos.y/length_unit, particle.pos.z/length_unit
        ).unwrap_or_else(|_| panic!("Output error: could not write to {}displacements.output.", options.name));
    }

    //Incident particle, left simulation: reflected
    if particle.incident & particle.left {
        writeln!(
            output_list_streams.reflected_file_stream, "{},{},{},{},{},{},{},{},{},{}",
            particle.m/mass_unit, particle.Z, particle.E/energy_unit,
            particle.pos.x/length_unit, particle.pos.y/length_unit, particle.pos.z/length_unit,
            particle.dir.x, particle.dir.y, particle.dir.z,
            particle.number_collision_events
        ).unwrap_or_else(|_| panic!("Output error: could not write to {}reflected.output.", options.name));
    }

    //Incident particle, stopped in material: deposited
    if particle.incident & particle.stopped {
        writeln!(
            output_list_streams.deposited_file_stream, "{},{},{},{},{},{}",
            particle.m/mass_unit, particle.Z,
            particle.pos.x/length_unit, particle.pos.y/length_unit, particle.pos.z/length_unit,
            particle.number_collision_events
        ).unwrap_or_else(|_| panic!("Output error: could not write to {}deposited.output.", options.name));
    }

    //Not an incident particle, left material: sputtered
    if !particle.incident & particle.left {
        writeln!(
            output_list_streams.sputtered_file_stream, "{},{},{},{},{},{},{},{},{},{},{},{},{}",
            particle.m/mass_unit, particle.Z, particle.E/energy_unit,
            particle.pos.x/length_unit, particle.pos.y/length_unit, particle.pos.z/length_unit,
            particle.dir.x, particle.dir.y, particle.dir.z,
            particle.number_collision_events,
            particle.pos_origin.x/length_unit, particle.pos_origin.y/length_unit, particle.pos_origin.z/length_unit
        ).unwrap_or_else(|_| panic!("Output error: could not write to {}sputtered.output.", options.name));
    }

    //Trajectory output
    if particle.track_trajectories {
        writeln!(output_list_streams.trajectory_data_stream, "{}", particle.trajectory.len())
            .unwrap_or_else(|_| panic!("Output error: could not write to {}trajectory_data.output.", options.name));

        for pos in particle.trajectory {
            writeln!(
                output_list_streams.trajectory_file_stream, "{},{},{},{},{},{}",
                particle.m/mass_unit, particle.Z, pos.E/energy_unit,
                pos.x/length_unit, pos.y/length_unit, pos.z/length_unit,
            ).unwrap_or_else(|_| panic!("Output error: could not write to {}trajectories.output.", options.name));
        }
    }

    if particle.incident & options.track_energy_losses {
        for energy_loss in particle.energies {
            writeln!(
                output_list_streams.energy_loss_file_stream, "{},{},{},{},{},{},{}",
                particle.m/mass_unit, particle.Z,
                energy_loss.En/energy_unit, energy_loss.Ee/energy_unit,
                energy_loss.x/length_unit, energy_loss.y/length_unit, energy_loss.z/length_unit,
            ).unwrap_or_else(|_| panic!("Output error: could not write to {}energy_loss.output.", options.name));
        }
    }
}

#[cfg(feature = "rustbca-std")]
/// Flush output list streams
pub fn output_list_flush(output_list_streams: &mut OutputListStreams) {
    output_list_streams.displacements_file_stream.flush().unwrap();
    output_list_streams.reflected_file_stream.flush().unwrap();
    output_list_streams.deposited_file_stream.flush().unwrap();
    output_list_streams.sputtered_file_stream.flush().unwrap();
    output_list_streams.trajectory_data_stream.flush().unwrap();
    output_list_streams.trajectory_file_stream.flush().unwrap();
}
