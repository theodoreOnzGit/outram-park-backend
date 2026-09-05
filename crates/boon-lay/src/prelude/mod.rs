pub use crate::decay_xml_info_serde;
pub use crate::decay_xml_info_serde::*;

// --- compute-backend resource switcher (CPU vs wgpu) ---
pub use crate::compute::{ComputeType, ThreadCount};
pub use crate::lagrangian_decay_simulator::stochastic_decay_chain;
pub use crate::nuclide_reaction_and_decay_data::decay_library;
pub use crate::nuclide_reaction_and_decay_data::DecayType;
pub use crate::nuclide_reaction_and_decay_data::HalfLifeAndDecayEnergyInfo;
pub use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;

pub use crate::lagrangian_decay_simulator::monte_carlo_single_radionuclide_decay_simulator::SingleNuclideSimulatorMC;

// --- triso_atops_fork: Eulerian / continuum TRISO release (fork of INL TRISO-ATOPS) ---
pub use crate::triso_atops_fork::activities::{
    activity_from_atom_count, atom_count_from_activity, base_activities, becquerels_from_curies,
    circulating, circulating_steadystate, clean_up, clean_up_steadystate, curies_from_becquerels,
    plate_out, plate_out_steadystate, release_rate, FailureFractions, SourceAndGraphite, BQ_PER_CI,
};
pub use crate::triso_atops_fork::diffusion::{
    diffusion_coefficient, diffusion_coefficient_sic_ag, integrate_diffusion_over_time,
    DiffusionMaterial, KernelGraphiteDiffusion,
};
pub use crate::triso_atops_fork::normal_operation::{
    normal_operation_node, NodalActivities, NodalActivitiesCurie, NodeState, ParentPools,
    PlantConstants,
};
pub use crate::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
pub use crate::triso_atops_fork::nuclide_model::{
    supported_nuclides, ElementGroup, TrisoAtopsNuclide, TRISO_ATOPS_NUCLIDE_COUNT,
};
pub use crate::triso_atops_fork::release_models::{
    rb_fail, release_fraction_transient, ReleaseMaterial,
};
pub use crate::triso_atops_fork::{Activity, DecayConstant, ReleaseFraction};
