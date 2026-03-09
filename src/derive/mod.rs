pub mod feature_driven;
pub mod stance;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StanceDerivationMode {
    Heuristic,
    FeatureDriven,
}
