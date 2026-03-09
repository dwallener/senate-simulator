use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SenateEvent {
    NoMeaningfulMovement,
    LeadershipSignalsAction,
    MotionToProceedAttempted,
    DebateBegins,
    AmendmentFightBegins,
    ClotureFiled,
    ClotureVoteScheduled,
    ClotureInvoked,
    ClotureFails,
    FinalPassageScheduled,
    FinalPassageSucceeds,
    FinalPassageFails,
    NegotiationIntensifies,
    ProceduralBlock,
    ReturnedToCalendar,
    Other(String),
}

impl fmt::Display for SenateEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::NoMeaningfulMovement => "NoMeaningfulMovement",
            Self::LeadershipSignalsAction => "LeadershipSignalsAction",
            Self::MotionToProceedAttempted => "MotionToProceedAttempted",
            Self::DebateBegins => "DebateBegins",
            Self::AmendmentFightBegins => "AmendmentFightBegins",
            Self::ClotureFiled => "ClotureFiled",
            Self::ClotureVoteScheduled => "ClotureVoteScheduled",
            Self::ClotureInvoked => "ClotureInvoked",
            Self::ClotureFails => "ClotureFails",
            Self::FinalPassageScheduled => "FinalPassageScheduled",
            Self::FinalPassageSucceeds => "FinalPassageSucceeds",
            Self::FinalPassageFails => "FinalPassageFails",
            Self::NegotiationIntensifies => "NegotiationIntensifies",
            Self::ProceduralBlock => "ProceduralBlock",
            Self::ReturnedToCalendar => "ReturnedToCalendar",
            Self::Other(value) => value.as_str(),
        };
        f.write_str(value)
    }
}
